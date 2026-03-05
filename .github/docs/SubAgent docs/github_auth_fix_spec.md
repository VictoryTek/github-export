# GitHub Authentication Fix — Feature Specification

**Project:** GitHub Export (Tauri v1 desktop app)  
**Feature:** Fix "Missing device_code in response" authentication error  
**Spec Author:** Research Subagent  
**Date:** 2026-03-05  

---

## Sources Consulted

1. **GitHub Docs — Device Flow for OAuth Apps**: https://docs.github.com/en/apps/oauth-apps/building-oauth-apps/authorizing-oauth-apps#response-parameters-1  
2. **GitHub Docs — Enabling Device Flow on an OAuth App**: https://docs.github.com/en/apps/oauth-apps/building-oauth-apps/authorizing-oauth-apps#enabling-device-flow-for-an-oauth-app  
3. **RFC 8628 — OAuth 2.0 Device Authorization Grant §3.2**: https://datatracker.ietf.org/doc/html/rfc8628#section-3.2 (error responses from the device authorization endpoint)  
4. **GitHub Docs — OAuth App error responses**: The device code endpoint returns `{"error":"...","error_description":"..."}` for all failure cases  
5. **`reqwest` v0.12 docs**: `.json()` succeeds even on HTTP 4xx/5xx responses as long as the body is valid JSON; caller must check for application-level errors  
6. **Tauri v1 source — `tauri::command` injection**: `AppHandle` is a valid injectable parameter in `#[tauri::command]`; confirmed from Tauri 1.x command guide  
7. **GitHub OAuth App settings page**: https://github.com/settings/developers — "Device Flow" must be **explicitly enabled** via a dedicated checkbox; it is **disabled by default** on all new OAuth Apps  

---

## 1. Current State Analysis

### Error Message (verbatim as shown in UI)
```
Failed to start sign-in: Missing device_code in response
```

### Auth Flow (current end-to-end)

**Trigger**: User clicks "Sign in with GitHub" in `src/index.html`.

**Frontend** (`src/main.js`, lines ~72–110):
- Calls `invoke('start_device_flow')` (no arguments).
- On success: displays `user_code`, then calls `invoke('poll_device_flow', {...})`.
- On failure: shows `"Failed to start sign-in: " + String(err)` in `#login-error`.

**Rust command** (`src-tauri/src/github/auth.rs`, `start_device_flow`):
```rust
let resp: serde_json::Value = client
    .post(DEVICE_CODE_URL)                        // https://github.com/login/device/code
    .header("Accept", "application/json")
    .form(&[("client_id", GITHUB_CLIENT_ID), ("scope", OAUTH_SCOPES)])
    .send().await?
    .json().await?;

let device_code = resp["device_code"]
    .as_str()
    .ok_or("Missing device_code in response")?   // ← ERROR ORIGINATES HERE
    .to_string();
```

**Key constant**:
```rust
const GITHUB_CLIENT_ID: &str = "Ov23lit0Ok09PHqufOw7";
```

---

## 2. Root Cause Analysis

There are **two compounding root causes**:

### Root Cause #1 — CRITICAL: No GitHub error-response detection

When `start_device_flow` calls the GitHub device code endpoint, the call can succeed at the HTTP level (GitHub returns HTTP 200 with a JSON body) but still represent an application-level failure. GitHub's device code endpoint always returns HTTP 200 with a JSON body, but for error conditions the body is:

```json
{
  "error": "unauthorized_client",
  "error_description": "The device flow has not been enabled for this OAuth app.",
  "error_uri": "https://docs.github.com/developers/apps/authorizing-oauth-apps"
}
```

or for an invalid/unknown client_id:
```json
{
  "error": "not_found"
}
```

The current code does **not check for an `error` field** before trying to extract `device_code`. Since error responses do not contain a `device_code` key, `resp["device_code"].as_str()` returns `None`, and `ok_or("Missing device_code in response")` fires — producing a misleading error message that tells the user nothing actionable.

### Root Cause #2 — CRITICAL: OAuth App "Device Flow" not enabled

The `GITHUB_CLIENT_ID` constant (`"Ov23lit0Ok09PHqufOw7"`) appears to be a real GitHub OAuth client ID (starts with `Ov23`, which is the format GitHub auto-generates). However, **GitHub does not enable Device Flow by default on new OAuth App registrations**. The app owner must:

1. Navigate to https://github.com/settings/developers → the OAuth App
2. Scroll to the "Device Flow" section
3. Check "Enable Device Flow"
4. Save changes

Without this step, any POST to `https://github.com/login/device/code` returns `{"error":"unauthorized_client","error_description":"The device flow has not been enabled for this OAuth app."}` — with no `device_code`.

### Why These Two Causes Interact

Root Cause #2 is the _environmental_ cause (mis-configuration of the OAuth App). Root Cause #1 is the _code_ cause (the error goes undetected and produces a confusing second-order error message). Because both are present, **fixing only Root Cause #2** (enabling Device Flow) would make the error disappear, but **fixing only Root Cause #1** (adding error detection) would surface the real message.

**Both must be fixed**: Root Cause #1 is a code correctness issue that will mask any future OAuth errors; Root Cause #2 is an immediate configuration bug.

---

## 3. Secondary Issues (Non-Critical)

### Issue S1: README advertises PAT auth but the app now uses Device Flow

`README.md` line 12 still says:
```
| Authenticate via Personal Access Token (PAT) | ✅ |
```

The frontend (`src/index.html`) has no PAT input field — the UI has been fully migrated to Device Flow. The README is stale and misleads users/contributors.

### Issue S2: No PAT fallback path

Power users and CI consumers typically prefer PAT-based auth. The prior implementation had a `store_token` keyring mechanism that is fully generic (works for both OAuth tokens and PATs). Adding a PAT input path alongside the Device Flow button is low-risk and high-value.

### Issue S3: Cancel does not abort in-flight polling (pre-existing, from OAUTH_DEVICE_FLOW_review.md)

Once `poll_device_flow` is dispatched to Rust, the JS cancel flag is never re-checked when the Rust coroutine resolves. This is tracked in the prior review; it is out of scope for this spec but is noted here for completeness.

---

## 4. Proposed Solution

### 4.1 Fix 1 — Add GitHub error detection to `start_device_flow` (REQUIRED)

**File**: `src-tauri/src/github/auth.rs`

**After** parsing the JSON response, check for the `error` field **before** attempting to extract `device_code`:

```rust
#[tauri::command]
pub async fn start_device_flow(app_handle: tauri::AppHandle) -> Result<DeviceFlowStart, String> {
    let client = reqwest::Client::new();

    let resp: serde_json::Value = client
        .post(DEVICE_CODE_URL)
        .header("Accept", "application/json")
        .form(&[("client_id", GITHUB_CLIENT_ID), ("scope", OAUTH_SCOPES)])
        .send()
        .await
        .map_err(|e| format!("Failed to reach GitHub device code endpoint: {e}"))?
        .json()
        .await
        .map_err(|e| format!("Failed to parse device code response: {e}"))?;

    // ── NEW: Detect GitHub application-level errors ───────────────────────
    // GitHub always returns HTTP 200 for this endpoint. Errors are signalled
    // via an "error" field in the JSON body (RFC 8628 §3.2 / GitHub docs).
    if let Some(error) = resp["error"].as_str() {
        let description = resp["error_description"]
            .as_str()
            .unwrap_or("No description provided by GitHub.");
        return Err(format!(
            "GitHub returned an error: {error} — {description}\n\
             If the error is \"unauthorized_client\", ensure Device Flow is enabled \
             on your OAuth App at https://github.com/settings/developers"
        ));
    }
    // ─────────────────────────────────────────────────────────────────────

    let device_code = resp["device_code"]
        .as_str()
        .ok_or("Missing device_code in response")?
        .to_string();
    let user_code = resp["user_code"]
        .as_str()
        .ok_or("Missing user_code in response")?
        .to_string();
    let verification_uri = resp["verification_uri"]
        .as_str()
        .ok_or("Missing verification_uri in response")?
        .to_string();
    let expires_in = resp["expires_in"].as_u64().unwrap_or(900);
    let interval = resp["interval"].as_u64().unwrap_or(5);

    tauri::api::shell::open(&app_handle.shell_scope(), &verification_uri, None)
        .map_err(|e| format!("Failed to open browser: {e}"))?;

    Ok(DeviceFlowStart {
        user_code,
        verification_uri,
        device_code,
        expires_in,
        interval,
    })
}
```

### 4.2 Fix 2 — Add a documentation comment to `GITHUB_CLIENT_ID` (REQUIRED)

**File**: `src-tauri/src/github/auth.rs`

Replace the bare client ID constant with a comment block that explains Device Flow must be explicitly enabled:

```rust
/// The GitHub OAuth App Client ID.
///
/// To obtain this:
/// 1. Go to https://github.com/settings/developers → "OAuth Apps" → your app
///    (or "New OAuth App" to create one)
/// 2. Copy the "Client ID" value (format: Ov23xxxxxxxxxxxxxxxx)
/// 3. **IMPORTANT**: On the same settings page, scroll to "Device Flow" and
///    click "Enable Device Flow" — this is required for this app to work.
///    It is disabled by default on all new OAuth Apps.
///
/// The Client ID is not a secret (RFC 8628 §3.4 — public clients).
const GITHUB_CLIENT_ID: &str = "Ov23lit0Ok09PHqufOw7";
```

### 4.3 Fix 3 — Update README.md (REQUIRED)

**File**: `README.md`

Update the feature table to reflect the current OAuth Device Flow authentication, removing the stale PAT reference:

| Feature | Status |
|---|---|
| Authenticate via GitHub OAuth (Device Flow) | ✅ |
| Persistent credential storage (OS keyring) | ✅ |

Also add a **Prerequisites** note about registering the GitHub OAuth App and enabling Device Flow.

### 4.4 Fix 4 — Add PAT authentication fallback (RECOMMENDED)

The Rust helpers `authenticate_with_token`, `store_token`, `load_token`, and `delete_token` are already implemented and generic. Adding a PAT path requires minimal new code.

**New Tauri command** in `auth.rs`:
```rust
/// Authenticate using a GitHub Personal Access Token (PAT).
///
/// This is a fallback for users who prefer PATs or cannot complete
/// the OAuth Device Flow (e.g., restricted network environments).
#[tauri::command]
pub async fn authenticate_with_pat(
    token: String,
    state: tauri::State<'_, Mutex<AppState>>,
) -> Result<String, String> {
    let client = authenticate_with_token(&token)
        .await
        .map_err(|e| e.to_string())?;

    let user = client
        .current()
        .user()
        .await
        .map_err(|e| format!("Failed to fetch GitHub user: {e}"))?;

    let username = user.login.clone();

    if let Err(e) = store_token(&token) {
        eprintln!("Warning: could not store token in keyring: {e}");
    }

    let mut app = state.lock().map_err(|e| e.to_string())?;
    app.client = Some(client);
    app.token = Some(token);
    app.username = Some(username.clone());

    Ok(username)
}
```

**Frontend changes** (`src/index.html`): Add a collapsible PAT section below the Device Flow button.

**Frontend changes** (`src/main.js`): Register a PAT form submit handler that calls `invoke('authenticate_with_pat', { token })`.

**`main.rs`**: Add `authenticate_with_pat` to the `invoke_handler`.

---

## 5. Files to Modify

| File | Change | Priority |
|------|--------|----------|
| `src-tauri/src/github/auth.rs` | Add error detection block in `start_device_flow`; add doc comment on `GITHUB_CLIENT_ID`; add `authenticate_with_pat` command | REQUIRED (error detect) / RECOMMENDED (PAT) |
| `src-tauri/src/main.rs` | Add `authenticate_with_pat` to `invoke_handler` | RECOMMENDED |
| `src/index.html` | Add PAT input section (collapsible) | RECOMMENDED |
| `src/main.js` | Add PAT form submit handler | RECOMMENDED |
| `README.md` | Update auth feature row; add OAuth App setup instructions | REQUIRED |

---

## 6. Dependencies

**No new Cargo dependencies are required.**

All needed crates are already in `Cargo.toml`:
- `reqwest` with `json` feature — already present
- `keyring` — already present  
- `octocrab` — already present
- `serde_json` — already present

**No `tauri.conf.json` changes** are needed for the minimum fix. (The prior review flagged the shell `open: true` as overly broad; that remains a recommended improvement but is not the cause of the current bug.)

---

## 7. Implementation Steps

### Minimum fix (resolves the reported error):

1. **Edit `src-tauri/src/github/auth.rs`**:
   - Replace the `GITHUB_CLIENT_ID` bare constant with the documented version (Fix 4.2)
   - Add the error detection block immediately after `.json().await?` parse in `start_device_flow` (Fix 4.1)

2. **Edit `README.md`**:
   - Update the "Authenticate via Personal Access Token (PAT)" row to "Authenticate via GitHub OAuth (Device Flow)"
   - Add a short setup section: "Creating a GitHub OAuth App" with the Device Flow checkbox step

3. **Run `cargo build`** from `src-tauri/` to confirm clean compilation.

4. **Run `cargo clippy -- -D warnings`** from `src-tauri/` to confirm no lint regressions.

### Full fix (adds PAT fallback):

5. **Edit `src-tauri/src/github/auth.rs`**: Add the `authenticate_with_pat` command (Fix 4.4).

6. **Edit `src-tauri/src/main.rs`**: Import and register `authenticate_with_pat` in `invoke_handler`.

7. **Edit `src/index.html`**: Add a PAT input section below the Device Flow card, initially collapsed (e.g., inside a `<details>` element).

8. **Edit `src/main.js`**: Add event listener for PAT form submit; call `invoke('authenticate_with_pat', { token })`.

9. **Run `cargo build`** and **`cargo clippy -- -D warnings`** again.

---

## 8. Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|-----------|
| Client ID `Ov23lit0Ok09PHqufOw7` is deleted/invalid on GitHub | Medium | Device Flow permanently broken | Code now surfaces the actual GitHub error message, making root cause obvious to the developer; PAT fallback provides a working auth path in the interim |
| Adding PAT fallback re-introduces a field that was intentionally removed | Low | Minor UI regression | The PAT section is opt-in via a `<details>` disclosure element; not prominently displayed |
| Error message from GitHub changes format | Low | Slightly less helpful message | The fallback `ok_or("Missing device_code in response")` still fires; never regresses below current behavior |
| `authenticate_with_pat` token not validated for required scopes | Low | Auth succeeds but API calls fail later | Acceptable; the PAT validation occurs at the first API call, which returns a clear error |

---

## 9. Expected Post-Fix Behavior

**When Device Flow App is not configured / client_id is wrong:**
```
Failed to start sign-in: GitHub returned an error: unauthorized_client — The device flow has not been enabled for this OAuth app.
If the error is "unauthorized_client", ensure Device Flow is enabled on your OAuth App at https://github.com/settings/developers
```

**When Device Flow App is correctly configured:**
- Flow proceeds normally; user code is displayed; browser opens to `github.com/login/device`

**With PAT fallback (optional):**
- User expands the "Use a Personal Access Token instead" disclosure
- Enters a PAT
- App authenticates directly without browser redirect

---

## 10. Verification Checklist

- [ ] `cargo build` exits 0
- [ ] `cargo clippy -- -D warnings` exits 0
- [ ] `cargo test` exits 0
- [ ] Manually testing with an OAuth App where Device Flow is **disabled** shows the GitHub error description, not "Missing device_code in response"
- [ ] Manually testing with a valid OAuth App where Device Flow is **enabled** completes successfully
- [ ] PAT auth path (if implemented) successfully authenticates and lists repos
- [ ] README accurately describes the current auth mechanism
