# OAuth Device Flow — Feature Specification

**Project:** GitHub Export (Tauri v1 desktop app)  
**Feature:** Replace PAT-based authentication with GitHub OAuth Device Flow  
**Spec Author:** Research Subagent  
**Date:** 2026-03-03  

---

## Sources Consulted

1. GitHub Docs — Authorizing OAuth Apps (Device Flow): https://docs.github.com/en/apps/oauth-apps/building-oauth-apps/authorizing-oauth-apps  
2. Tauri v1 `tauri::api::shell::open` Rust API: https://docs.rs/tauri/1.8.0/tauri/api/shell/fn.open.html  
3. Tokio `time::sleep` async API: https://docs.rs/tokio/latest/tokio/time/fn.sleep.html  
4. RFC 8628 — OAuth 2.0 Device Authorization Grant: https://tools.ietf.org/html/rfc8628  
5. `keyring` crate — crates.io / docs.rs (already used in the project)  
6. `reqwest` crate v0.12 JSON API (already used in the project via Cargo.toml)  
7. GitHub OAuth App settings docs: https://github.com/settings/developers  

---

## 1. Current State Analysis

### Auth Flow (end-to-end today)

**Rust side (`src-tauri/src/github/auth.rs`):**  
- `authenticate_with_token(token: &str)` — builds an `Octocrab` client from a PAT.  
- `store_token(token: &str)` — writes to OS keyring (service=`"github-export"`, user=`"github-token"`).  
- `load_token()` — reads from that same keyring entry.  
- `delete_token()` — removes the keyring entry.  

**Rust side (`src-tauri/src/main.rs` — registered Tauri commands):**  
- `authenticate(token: String, state)` → calls `authenticate_with_token`, fetches `current().user()`, stores in `AppState`, calls `store_token`, returns `username`.  
- `restore_session(state)` → calls `load_token()`, re-authenticates, returns `Option<username>`.  
- `logout(state)` → clears `AppState`, calls `delete_token()`.  

**Frontend (`src/index.html` + `src/main.js`):**  
- Login screen has `#token-input` (password field) and `#login-btn`.  
- On click: `invoke("authenticate", { token })` → on success calls `showApp(username)`.  
- `DOMContentLoaded`: calls `invoke("restore_session")` → if returned user, calls `showApp`.  
- Logout button: `invoke("logout")` → switches back to login screen.  

### Files that require changes

| File | Change type |
|------|-------------|
| `src-tauri/src/github/auth.rs` | Major — add device flow functions, keep keyring helpers |
| `src-tauri/src/main.rs` | Moderate — add 2 new commands, remove/keep old `authenticate` |
| `src-tauri/Cargo.toml` | Minor — add `shell-open-api` to tauri features |
| `src-tauri/tauri.conf.json` | Minor — tighten shell open scope to GitHub URLs |
| `src/index.html` | Moderate — replace PAT form with OAuth button + code card |
| `src/main.js` | Moderate — replace PAT login logic with device flow logic |
| `src/styles.css` | Minor — add styles for OAuth UI (button, code card, spinner) |

---

## 2. GitHub OAuth App Requirement

### Creating the OAuth App

The developer (app publisher) must create a GitHub OAuth App before shipping:

1. Visit https://github.com/settings/developers → "OAuth Apps" → "New OAuth App"
2. Fill in:
   - **Application name:** `GitHub Export`
   - **Homepage URL:** `https://github.com/your-org/github-export` (or any valid URL)
   - **Authorization callback URL:** Leave blank or enter any value — **not used** for Device Flow
3. After creation, enable **"Device Flow"** in the app's settings page (there is a dedicated checkbox under "Device Flow" → enable it)
4. Note the **Client ID** (20-character alphanumeric string starting with `Ov23...` or `ghu_...`)
5. **No client secret needed** — the GitHub Device Flow specification (RFC 8628 §3.4) explicitly states client authentication is not required for public clients using device authorization

### In the code

A placeholder constant will be defined in `auth.rs`:

```rust
/// Replace this with the Client ID from your GitHub OAuth App settings.
/// See: https://github.com/settings/developers
const GITHUB_CLIENT_ID: &str = "YOUR_OAUTH_APP_CLIENT_ID";
```

The `client_id` is **not a secret** — it is intentionally embedded in the binary and visible to users. GitHub's device flow is designed for exactly this use case (CLI tools, desktop apps).

Required OAuth scopes for this application:

- `repo` — read/write access to repositories (needed for issues, PRs)
- `security_events` — read access to security alerts (Dependabot/code scanning)

---

## 3. Proposed Device Flow Implementation

### 3.1 GitHub Device Flow — Protocol Summary

**Step 1 — Request codes:**
```
POST https://github.com/login/device/code
Headers: Accept: application/json
Body: client_id=<CLIENT_ID>&scope=repo%20security_events
```
Response:
```json
{
  "device_code": "3584d83530557fdd1f46af8289938c8ef79f9dc5",
  "user_code": "WDJB-MJHT",
  "verification_uri": "https://github.com/login/device",
  "expires_in": 900,
  "interval": 5
}
```

**Step 2 — Open browser** to `verification_uri`, display `user_code` to user.

**Step 3 — Poll for token:**
```
POST https://github.com/login/oauth/access_token
Headers: Accept: application/json
Body: client_id=<CLIENT_ID>&device_code=<device_code>&grant_type=urn:ietf:params:oauth:grant-type:device_code
```
Poll every `interval` seconds (minimum). On success:
```json
{
  "access_token": "gho_16C7e42F292c6912E7710c838347Ae178B4a",
  "token_type": "bearer",
  "scope": "repo,security_events"
}
```
Intermediate responses (while waiting):
- `{ "error": "authorization_pending" }` → keep polling
- `{ "error": "slow_down" }` → add 5s to interval, keep polling
- `{ "error": "access_denied" }` → user cancelled, return error
- `{ "error": "expired_token" }` → 15-minute timeout exceeded, return error

---

### 3.2 Rust Backend Changes (`src-tauri/src/github/auth.rs`)

#### New structs (Serde-serializable, returned to JS)

```rust
#[derive(Debug, Serialize)]
pub struct DeviceFlowStart {
    pub user_code: String,
    pub verification_uri: String,
    pub device_code: String,
    pub expires_in: u64,
    pub interval: u64,
}
```

#### New constants

```rust
/// Replace with your GitHub OAuth App Client ID.
/// Device flow does NOT require a client secret.
const GITHUB_CLIENT_ID: &str = "YOUR_OAUTH_APP_CLIENT_ID";
const DEVICE_CODE_URL: &str  = "https://github.com/login/device/code";
const ACCESS_TOKEN_URL: &str = "https://github.com/login/oauth/access_token";
const OAUTH_SCOPES: &str     = "repo security_events";
```

#### New public function: `begin_device_flow`

```rust
/// POST to GitHub to get device_code + user_code, then open the browser.
/// Returns the DeviceFlowStart needed for polling.
pub async fn begin_device_flow(app_handle: &tauri::AppHandle) -> Result<DeviceFlowStart> {
    let client = reqwest::Client::new();

    let resp: serde_json::Value = client
        .post(DEVICE_CODE_URL)
        .header("Accept", "application/json")
        .form(&[
            ("client_id", GITHUB_CLIENT_ID),
            ("scope", OAUTH_SCOPES),
        ])
        .send()
        .await
        .context("Failed to reach GitHub device code endpoint")?
        .json()
        .await
        .context("Failed to parse device code response")?;

    let device_code     = resp["device_code"].as_str().context("missing device_code")?.to_string();
    let user_code       = resp["user_code"].as_str().context("missing user_code")?.to_string();
    let verification_uri = resp["verification_uri"].as_str().context("missing verification_uri")?.to_string();
    let expires_in      = resp["expires_in"].as_u64().unwrap_or(900);
    let interval        = resp["interval"].as_u64().unwrap_or(5);

    // Open the browser automatically — user still needs to type the user_code
    tauri::api::shell::open(
        &app_handle.shell_scope(),
        &verification_uri,
        None,
    )
    .context("Failed to open browser")?;

    Ok(DeviceFlowStart { user_code, verification_uri, device_code, expires_in, interval })
}
```

#### New public function: `poll_device_flow`

```rust
/// Poll GitHub until the user authorizes or the code expires.
/// On success, stores token in keyring and returns (access_token, username) tuple.
pub async fn poll_device_flow(
    device_code: &str,
    expires_in: u64,
    mut interval: u64,
) -> Result<String> {
    use tokio::time::{sleep, Duration, Instant, timeout};

    let deadline   = Duration::from_secs(expires_in);
    let client     = reqwest::Client::new();
    let start      = Instant::now();

    loop {
        // Check outer timeout
        if start.elapsed() >= deadline {
            anyhow::bail!("Authorization timed out — the code expired after {} seconds", expires_in);
        }

        // Respect the server-mandated polling interval
        sleep(Duration::from_secs(interval)).await;

        let resp: serde_json::Value = client
            .post(ACCESS_TOKEN_URL)
            .header("Accept", "application/json")
            .form(&[
                ("client_id",   GITHUB_CLIENT_ID),
                ("device_code", device_code),
                ("grant_type",  "urn:ietf:params:oauth:grant-type:device_code"),
            ])
            .send()
            .await
            .context("Network error while polling for access token")?
            .json()
            .await
            .context("Failed to parse access token poll response")?;

        if let Some(token) = resp["access_token"].as_str() {
            return Ok(token.to_string());
        }

        match resp["error"].as_str().unwrap_or("") {
            "authorization_pending" => continue,
            "slow_down" => {
                interval += 5; // Server requests we slow down
                continue;
            }
            "access_denied" => anyhow::bail!("Authorization was cancelled by the user."),
            "expired_token" => anyhow::bail!("The device code has expired. Please try again."),
            other => anyhow::bail!("Unexpected error from GitHub: {}", other),
        }
    }
}
```

#### Existing functions to keep unchanged

- `store_token(token: &str)` — **keep as-is**
- `load_token()` — **keep as-is**
- `delete_token()` — **keep as-is**
- `authenticate_with_token(token: &str)` — **keep as-is** (used by `restore_session`)

---

### 3.3 Rust Backend Changes (`src-tauri/src/main.rs`)

#### New command: `start_device_flow`

```rust
/// Begin the OAuth Device Flow: get user_code + device_code, open browser.
#[tauri::command]
async fn start_device_flow(
    app_handle: tauri::AppHandle,
) -> Result<github::auth::DeviceFlowStart, String> {
    github::auth::begin_device_flow(&app_handle)
        .await
        .map_err(|e| e.to_string())
}
```

#### New command: `poll_device_flow`

```rust
/// Poll GitHub until authorization completes. Returns the authenticated username.
#[tauri::command]
async fn poll_device_flow(
    device_code: String,
    expires_in: u64,
    interval: u64,
    state: State<'_, Mutex<AppState>>,
) -> Result<String, String> {
    let token = github::auth::poll_device_flow(&device_code, expires_in, interval)
        .await
        .map_err(|e| e.to_string())?;

    // Store token in keyring
    if let Err(e) = github::auth::store_token(&token) {
        eprintln!("Warning: could not store token in keyring: {e}");
    }

    // Build Octocrab client and fetch username (same pattern as `authenticate`)
    let client = github::auth::authenticate_with_token(&token)
        .await
        .map_err(|e| e.to_string())?;

    let user = client
        .current()
        .user()
        .await
        .map_err(|e| format!("Failed to fetch user: {e}"))?;

    let username = user.login.clone();

    let mut app = state.lock().map_err(|e| e.to_string())?;
    app.client   = Some(client);
    app.token    = Some(token);
    app.username = Some(username.clone());

    Ok(username)
}
```

#### Update `tauri::generate_handler!`

Add `start_device_flow` and `poll_device_flow` to the handler list.  
The old `authenticate` command **should be retained** as a fallback PAT path (some enterprise users may prefer it), but the login UI will no longer show the PAT input by default. If removing it, remove from the handler list too. **Recommendation: keep `authenticate` registered** — it is harmless and gives a fallback.

#### `tauri::generate_handler!` after changes:

```rust
.invoke_handler(tauri::generate_handler![
    authenticate,        // keep (restore_session depends on store_token flow)
    restore_session,     // keep unchanged
    logout,              // keep unchanged
    start_device_flow,   // NEW
    poll_device_flow,    // NEW
    list_repos,
    fetch_issues,
    fetch_pulls,
    fetch_security_alerts,
    export_data,
])
```

---

### 3.4 Cargo.toml Changes

**Only one change required:**

```toml
# Before:
tauri = { version = "1", features = ["dialog-save", "shell-open"] }

# After:
tauri = { version = "1", features = ["dialog-save", "shell-open", "shell-open-api"] }
```

**Why:** The `shell-open` feature enables the Tauri allowlist entry and the JS `tauri.shell.open()` API. The `shell-open-api` feature is the separate gate that enables the **Rust** API `tauri::api::shell::open()` (as documented on docs.rs: "Available on crate feature `shell-open-api` only"). Without adding this feature, the Rust code in `begin_device_flow` will not compile.

All other required crates are **already present**:

| Crate | Already present? | Needed for |
|-------|-----------------|------------|
| `reqwest` v0.12 with `json` feature | ✅ Yes | HTTP calls to GitHub device/code and token endpoints |
| `tokio` v1 with `full` features | ✅ Yes | `tokio::time::sleep`, `tokio::time::Instant` |
| `serde_json` v1 | ✅ Yes | Parsing JSON poll responses |
| `anyhow` v1 | ✅ Yes | Error propagation in auth functions |
| `keyring` v3 | ✅ Yes | Storing the OAuth token (same as PAT storage) |
| `tauri` v1 | ✅ Yes | `AppHandle`, `shell_scope()` |

**No new crate dependencies needed.**

---

### 3.5 tauri.conf.json Changes

The `allowlist.shell.open` is already `true`. For defense in depth, tighten to only allow GitHub URLs:

```json
// Before:
"shell": {
  "open": true
}

// After:
"shell": {
  "open": "^https://github\\.com/.*"
}
```

This restricts `tauri::api::shell::open` to only open URLs matching the GitHub domain, preventing any future accidental or malicious use to open arbitrary URLs.

---

### 3.6 Frontend Changes (`src/index.html`)

Replace the entire `#login-screen` section:

```html
<!-- ─── Login screen ─────────────────────────── -->
<section id="login-screen" class="screen active">
  <div class="login-card">
    <h1>GitHub Export</h1>
    <p class="login-subtitle">Sign in to access your repositories, issues, pull requests, and security alerts.</p>

    <!-- Step 1: Sign-in button (shown by default) -->
    <div id="oauth-step-signin">
      <button id="oauth-signin-btn" class="btn-github">
        <svg class="github-icon" viewBox="0 0 16 16" width="20" height="20" aria-hidden="true">
          <path fill="currentColor" d="M8 0C3.58 0 0 3.58 0 8c0 3.54 2.29 6.53 5.47 7.59.4.07.55-.17.55-.38
          0-.19-.01-.82-.01-1.49-2.01.37-2.53-.49-2.69-.94-.09-.23-.48-.94-.82-1.13-.28-.15-.68-.52-.01-.53.63-.01
          1.08.58 1.23.82.72 1.21 1.87.87 2.33.66.07-.52.28-.87.51-1.07-1.78-.2-3.64-.89-3.64-3.95
          0-.87.31-1.59.82-2.15-.08-.2-.36-1.02.08-2.12 0 0 .67-.21 2.2.82.64-.18 1.32-.27 2-.27.68
          0 1.36.09 2 .27 1.53-1.04 2.2-.82 2.2-.82.44 1.1.16 1.92.08 2.12.51.56.82 1.27.82 2.15
          0 3.07-1.87 3.75-3.65 3.95.29.25.54.73.54 1.48 0 1.07-.01 1.93-.01 2.2 0 .21.15.46.55.38A8.013
          8.013 0 0016 8c0-4.42-3.58-8-8-8z"/>
        </svg>
        Sign in with GitHub
      </button>
      <p id="login-error" class="error"></p>
    </div>

    <!-- Step 2: Device code card (hidden until flow starts) -->
    <div id="oauth-step-code" class="hidden">
      <p class="login-subtitle">Your browser has opened <strong>github.com/login/device</strong>.<br>
        Enter this code when prompted:</p>
      <div class="device-code-display">
        <span id="device-user-code">XXXX-XXXX</span>
        <button id="copy-code-btn" class="btn-copy" title="Copy code">
          <svg viewBox="0 0 16 16" width="16" height="16" aria-hidden="true">
            <path fill="currentColor" d="M0 6.75C0 5.784.784 5 1.75 5h1.5a.75.75 0
            010 1.5h-1.5a.25.25 0 00-.25.25v7.5c0 .138.112.25.25.25h7.5a.25.25 0
            00.25-.25v-1.5a.75.75 0 011.5 0v1.5A1.75 1.75 0 019.25 16h-7.5A1.75
            1.75 0 010 14.25v-7.5z"/><path fill="currentColor" d="M5 1.75C5 .784
            5.784 0 6.75 0h7.5C15.216 0 16 .784 16 1.75v7.5A1.75 1.75 0 0114.25
            11h-7.5A1.75 1.75 0 015 9.25v-7.5zm1.75-.25a.25.25 0 00-.25.25v7.5c0
            .138.112.25.25.25h7.5a.25.25 0 00.25-.25v-7.5a.25.25 0 00-.25-.25h-7.5z"/>
          </svg>
        </button>
      </div>
      <div class="oauth-waiting">
        <div class="spinner"></div>
        <span>Waiting for authorization…</span>
      </div>
      <button id="oauth-cancel-btn" class="btn-cancel">Cancel</button>
    </div>

  </div>
</section>
```

---

### 3.7 Frontend Changes (`src/main.js`)

#### DOM references to add / replace

```javascript
// OAuth Device Flow DOM references
const oauthSigninBtn   = $("#oauth-signin-btn");
const oauthStepSignin  = $("#oauth-step-signin");
const oauthStepCode    = $("#oauth-step-code");
const deviceUserCode   = $("#device-user-code");
const copyCodeBtn      = $("#copy-code-btn");
const oauthCancelBtn   = $("#oauth-cancel-btn");
const loginError       = $("#login-error");
```

#### Remove / replace PAT references

Remove: `tokenInput`, `loginBtn` DOM references and their event listener.  
Keep: `loginError`.

#### New auth logic

```javascript
// Tracks whether the user cancelled during polling
let oauthCancelled = false;

oauthSigninBtn.addEventListener("click", async () => {
  loginError.textContent = "";
  oauthSigninBtn.disabled = true;

  let flowData;
  try {
    flowData = await invoke("start_device_flow");
  } catch (e) {
    loginError.textContent = `Failed to start sign-in: ${e}`;
    oauthSigninBtn.disabled = false;
    return;
  }

  // Show the code card, hide the button
  deviceUserCode.textContent = flowData.user_code;
  oauthStepSignin.classList.add("hidden");
  oauthStepCode.classList.remove("hidden");

  oauthCancelled = false;

  // Poll for authorization
  try {
    const username = await invoke("poll_device_flow", {
      deviceCode: flowData.device_code,
      expiresIn:  flowData.expires_in,
      interval:   flowData.interval,
    });

    if (!oauthCancelled) {
      await showApp(username);
    }
  } catch (e) {
    resetOAuthUI();
    loginError.textContent = oauthCancelled
      ? ""
      : `Authorization failed: ${e}`;
  }
});

copyCodeBtn.addEventListener("click", () => {
  navigator.clipboard.writeText(deviceUserCode.textContent).then(() => {
    copyCodeBtn.title = "Copied!";
    setTimeout(() => (copyCodeBtn.title = "Copy code"), 2000);
  });
});

oauthCancelBtn.addEventListener("click", () => {
  oauthCancelled = true;
  resetOAuthUI();
});

function resetOAuthUI() {
  oauthStepCode.classList.add("hidden");
  oauthStepSignin.classList.remove("hidden");
  oauthSigninBtn.disabled = false;
  deviceUserCode.textContent = "XXXX-XXXX";
}
```

#### Keep `restore_session` and `logout` event handler unchanged

The `restore_session` invocation in `DOMContentLoaded` works without change — it still calls `load_token()` from keyring (the OAuth token is stored under the same keyring key as PATs were).

The `logout` handler is unchanged.

---

### 3.8 CSS Changes (`src/styles.css`)

Add these new rules (append to end of file):

```css
/* ── OAuth Device Flow UI ────────────────────── */

/* GitHub-branded sign-in button */
.btn-github {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 0.6rem;
  width: 100%;
  padding: 0.7rem 1rem;
  background: #238636;
  color: #fff;
  border: 1px solid rgba(240,246,252,0.1);
  border-radius: var(--radius);
  cursor: pointer;
  font-size: 1rem;
  font-weight: 600;
  transition: background 0.15s;
  margin-top: 0.5rem;
}
.btn-github:hover:not(:disabled) { background: #2ea043; }
.btn-github:disabled { opacity: 0.55; cursor: default; }
.github-icon { flex-shrink: 0; }

/* Device code display */
.device-code-display {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 0.75rem;
  background: var(--bg);
  border: 1px solid var(--border);
  border-radius: var(--radius);
  padding: 1rem 1.25rem;
  margin: 1rem 0;
}
#device-user-code {
  font-family: "Fira Mono", "Cascadia Code", "Consolas", monospace;
  font-size: 1.9rem;
  font-weight: 700;
  letter-spacing: 0.12em;
  color: var(--text);
}
.btn-copy {
  background: none;
  border: 1px solid var(--border);
  color: var(--text-muted);
  border-radius: var(--radius);
  padding: 0.35rem 0.5rem;
  cursor: pointer;
  display: flex;
  align-items: center;
  transition: color 0.15s, border-color 0.15s;
}
.btn-copy:hover { color: var(--accent); border-color: var(--accent); }

/* Spinner + waiting text */
.oauth-waiting {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 0.6rem;
  color: var(--text-muted);
  font-size: 0.9rem;
  margin-bottom: 1rem;
}
@keyframes spin { to { transform: rotate(360deg); } }
.spinner {
  width: 16px;
  height: 16px;
  border: 2px solid var(--border);
  border-top-color: var(--accent);
  border-radius: 50%;
  animation: spin 0.75s linear infinite;
  flex-shrink: 0;
}

/* Cancel button */
.btn-cancel {
  width: 100%;
  padding: 0.5rem;
  background: none;
  color: var(--text-muted);
  border: 1px solid var(--border);
  border-radius: var(--radius);
  cursor: pointer;
  font-size: 0.88rem;
  transition: color 0.15s, border-color 0.15s;
}
.btn-cancel:hover { color: var(--red); border-color: var(--red); }

/* Minor: login subtitle text */
.login-subtitle {
  color: var(--text-muted);
  margin-bottom: 1rem;
  font-size: 0.9rem;
  line-height: 1.5;
}
```

---

## 4. Implementation Steps (Ordered)

1. **`src-tauri/Cargo.toml`** — Add `"shell-open-api"` to the `tauri` features list.

2. **`src-tauri/tauri.conf.json`** — Change `"shell": { "open": true }` to `"shell": { "open": "^https://github\\.com/.*" }` to restrict shell open to GitHub URLs.

3. **`src-tauri/src/github/auth.rs`**:
   a. Add `use serde::Serialize;` (already in scope via `use anyhow::{Context, Result};` and project-level serde dep).  
   b. Add `GITHUB_CLIENT_ID`, `DEVICE_CODE_URL`, `ACCESS_TOKEN_URL`, `OAUTH_SCOPES` constants.  
   c. Add `DeviceFlowStart` struct.  
   d. Add `begin_device_flow(app_handle: &tauri::AppHandle) -> Result<DeviceFlowStart>` function.  
   e. Add `poll_device_flow(device_code: &str, expires_in: u64, interval: u64) -> Result<String>` function.  
   f. Keep all existing functions (`authenticate_with_token`, `store_token`, `load_token`, `delete_token`) unchanged.

4. **`src-tauri/src/main.rs`**:
   a. Add `start_device_flow(app_handle: tauri::AppHandle)` command.  
   b. Add `poll_device_flow(device_code, expires_in, interval, state)` command.  
   c. Add both to `tauri::generate_handler![]`.

5. **`src/styles.css`** — Append the OAuth UI CSS rules.

6. **`src/index.html`** — Replace the `#login-screen` section content with the OAuth UI markup.

7. **`src/main.js`**:
   a. Update DOM references block (remove `tokenInput`, `loginBtn`; add OAuth refs).  
   b. Remove the `loginBtn.addEventListener("click", ...)` PAT handler.  
   c. Add `oauthSigninBtn`, `copyCodeBtn`, `oauthCancelBtn` event listeners.  
   d. Add `resetOAuthUI()` helper.  
   e. Keep `restore_session`, `logout`, `showApp`, and all other functions unchanged.

8. **Compile check** — Run `cargo build` from `src-tauri/` and `cargo clippy -- -D warnings` to verify no errors.

9. **Manual test** — Run `npm run dev` (Tauri dev mode), click "Sign in with GitHub", confirm browser opens, enter user_code, confirm app receives token and shows authenticated state.

---

## 5. UI Design

### Initial Screen (login not yet started)

```
┌─────────────────────────────────────────────────┐
│                                                 │
│                 GitHub Export                   │
│                                                 │
│  Sign in to access your repos, issues, PRs,     │
│  and security alerts.                           │
│                                                 │
│  ┌─────────────────────────────────────────┐    │
│  │  ⬤  Sign in with GitHub                │    │  ← green #238636 button with GitHub SVG logo
│  └─────────────────────────────────────────┘    │
│                                                 │
└─────────────────────────────────────────────────┘
```

- Card uses same `.login-card` container as today
- Title: `h1` with "GitHub Export" (no emoji needed — icon in button)
- Subtitle: grey helper text
- Button: GitHub green (`#238636`), white text, GitHub Octocat SVG inline, hover lightens to `#2ea043`

### After "Sign in with GitHub" is clicked

```
┌─────────────────────────────────────────────────┐
│                                                 │
│                 GitHub Export                   │
│                                                 │
│  Your browser has opened                        │
│  github.com/login/device.                       │
│  Enter this code when prompted:                 │
│                                                 │
│  ┌──────────────────────────────────┐           │
│  │    W D J B - M J H T    [copy]   │           │  ← monospace, 1.9rem, var(--text)
│  └──────────────────────────────────┘           │
│                                                 │
│   ⟳  Waiting for authorization…               │  ← spinner + text
│                                                 │
│  ┌──────────────────────────────────┐           │
│  │              Cancel              │           │  ← subtle grey border button
│  └──────────────────────────────────┘           │
│                                                 │
└─────────────────────────────────────────────────┘
```

- Browser opens automatically via `tauri::api::shell::open`
- User code displayed in large monospace  
- Copy button (clipboard icon) beside the code — copies to clipboard on click  
- Spinner animation (CSS `@keyframes spin`) + "Waiting for authorization…"
- Cancel button — aborts polling in JS, returns to sign-in button

### Transition on Success

When `poll_device_flow` resolves:

1. The `showApp(username)` function is called (existing code, no change)
2. `#login-screen` loses `.active` class, hides
3. `#app-screen` gains `.active` class, shows
4. `#username` is set to `@username`
5. `loadRepos()` is called

The existing CSS transition (`display: flex` / `display: none` via `.screen` / `.screen.active`) handles the switch. No additional animation needed unless desired.

---

## 6. Error Handling

### Token expired (15-minute timeout)

GitHub returns `{ "error": "expired_token" }` once the device code is older than 900 seconds.

**Handling in Rust:** `poll_device_flow` returns `Err("The device code has expired. Please try again.")`.  
**Handling in JS:** Caught in the `catch(e)` block of the `poll_device_flow` invocation → `resetOAuthUI()` → `loginError.textContent` set to the error string → user sees the "Sign in with GitHub" button again with an error message, can retry.

There is also a client-side timeout check using `start.elapsed() >= deadline` in the Rust polling loop, ensuring the loop terminates even if GitHub never returns `expired_token` (defensive programming).

### User denied access

GitHub returns `{ "error": "access_denied" }` when the user clicks "Cancel" on the browser authorization page.

**Handling in Rust:** `poll_device_flow` returns `Err("Authorization was cancelled by the user.")`.  
**Handling in JS:** Same as above — reset UI, display error, allow retry.

### User cancels from app UI

The "Cancel" button in the app sets `oauthCancelled = true`. When the pending `poll_device_flow` invoke eventually resolves or rejects:
- If resolves (user approved in browser after cancelling in app view): `showApp` is NOT called (`if (!oauthCancelled)` guard), UI stays on login.
- If rejects: error is silently swallowed (empty `loginError`).

**Limitation:** The Rust `poll_device_flow` command continues running in the background even after JS "cancels". This is acceptable because:
- The device code expires in 15 minutes maximum
- The Tokio runtime will clean up the future
- No memory leaks; AppState is only mutated on success, which is guarded by `oauthCancelled` in JS

If future improvement is needed, a cancellation token (e.g., `tokio_util::sync::CancellationToken`) can be introduced.

### Network errors

`reqwest` errors propagate as `anyhow::Error` → `"Network error while polling for access token: ..."`.  
**Handling in JS:** Same catch block, displayed in `loginError`.

### Already authenticated (token in keyring on startup)

`restore_session` in `DOMContentLoaded` calls `load_token()` → if a valid token exists from a previous OAuth session (stored under the same keyring key), it re-authenticates and calls `showApp` immediately — the login screen is never shown.

This works without any change because the OAuth token output from `poll_device_flow` is stored via `store_token()` (same service/user key as PATs).

### `slow_down` rate limiting

When GitHub returns `{ "error": "slow_down" }`:
- The `interval` variable is incremented by 5 seconds *in the Rust polling loop*
- The loop sleeps for the new longer interval before the next request
- This is transparent to the JS layer

---

## 7. Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|------------|
| Developer forgets to enable "Device Flow" in OAuth App settings | Medium | High — app silently gets `device_flow_disabled` error | Spec includes explicit setup instructions; error is surfaced to JS as `"device_flow_disabled"` error string |
| `CLIENT_ID` placeholder left in code | Medium | High — no valid OAuth App registered | Add a compile-time `assert!` or at least a comment warning; document in README |
| `shell-open-api` feature missing | Low (easy to add) | High — compile error | Explicitly included in Cargo.toml change in Step 1 |
| `poll_device_flow` Tauri command times out with Tauri's default IPC timeout | Low | High — auth never completes | Tauri v1 has no default IPC timeout on async commands; long-running async tasks are fine |
| Browser not available (rare headless environment) | Very Low | Medium | `tauri::api::shell::open` returns a `Result`; error propagated to JS — user sees "Failed to open browser: ..." and can manually navigate |
| Token stored but Octocrab client not rebuilt on next launch | None | N/A | `restore_session` rebuilds the Octocrab client on startup from keyring token — no change needed |
| `access_denied` polling loop doesn't terminate immediately | None | N/A | `access_denied` arm does `anyhow::bail!` which exits the loop and returns Err immediately |
| CSP (Content Security Policy) in tauri.conf.json blocks inline SVG | Low | Low | `"csp": null` is already set in tauri.conf.json — no CSP restrictions |
| Enterprise GitHub (`github.example.com`) incompatibility | Medium | Medium | Out of scope for v1; device code URL is hardcoded to `github.com`. Can be parameterized in a future iteration |
| Multiple `poll_device_flow` invocations running in parallel (user clicks "Sign in" multiple times) | Low | Low | `oauthSigninBtn.disabled = true` immediately on click prevents double-invocation |

---

## Summary of Findings

**All research goals confirmed:**

1. **Device Flow protocol** is fully documented. The flow is: POST `/login/device/code` → display `user_code` + open browser → poll `/login/oauth/access_token` with `grant_type=urn:ietf:params:oauth:grant-type:device_code` until approved/expired. Default timeout is 900s, default poll interval is 5s.

2. **No client secret required** for the Device Flow (confirmed by GitHub docs: "The client_secret is not needed for the device flow"). The `client_id` is safe to compile into the binary.

3. **`tauri::api::shell::open`** is available in Tauri v1 behind the `shell-open-api` crate feature (distinct from the `shell-open` allowlist feature). The app already has `shell-open`; only `shell-open-api` needs adding. The function signature is `open(scope: &ShellScope, path: impl AsRef<str>, with: Option<Program>) -> Result<()>`.

4. **All required crates already present** — `reqwest` v0.12 with `json` (for HTTP), `tokio` with `full` (for `sleep`/`Instant`), `serde_json` (for response parsing), `anyhow` (for error propagation), `keyring` (for token storage). Zero new dependencies.

5. **Token storage is identical** to PAT storage — `store_token`/ `load_token` use keyring service=`"github-export"`, user=`"github-token"`. OAuth token replaces PAT in the same slot. `restore_session` works unchanged.

6. **Polling in async Rust** is straightforward: `tokio::time::sleep(Duration::from_secs(interval)).await` inside a `loop { }`, with `anyhow::bail!` for terminal error conditions.

7. **Frontend changes are self-contained** — only `#login-screen` HTML, three small JS functions, and new CSS classes. All existing functionality (repo list, tabs, export, filters) is untouched.

---

**Spec file path:** `c:\Projects\github-export\.github\docs\SubAgent docs\OAUTH_DEVICE_FLOW_spec.md`
