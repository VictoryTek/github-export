# Multi-Account Support — Feature Specification

**Feature:** Multi-Account Support  
**Project:** GitHub Export (Tauri v1 desktop app)  
**Date:** 2026-03-05  
**Status:** DRAFT

---

## Table of Contents

1. [Current State Analysis](#1-current-state-analysis)
2. [Proposed Data Model](#2-proposed-data-model)
3. [Tauri Commands](#3-tauri-commands)
4. [Keyring Storage Strategy](#4-keyring-storage-strategy)
5. [Frontend Changes](#5-frontend-changes)
6. [Implementation Steps](#6-implementation-steps)
7. [Dependencies](#7-dependencies)
8. [Risks and Mitigations](#8-risks-and-mitigations)

---

## 1. Current State Analysis

### 1.1 Application State (`src-tauri/src/models/mod.rs`)

The entire authenticated session is held in a single struct managed by Tauri:

```rust
#[derive(Default)]
pub struct AppState {
    pub client:   Option<Octocrab>,   // GitHub API client (one at a time)
    pub token:    Option<String>,     // Raw token string
    pub username: Option<String>,     // Resolved GitHub login name
}
```

This is registered at startup as `Mutex<AppState>` via `tauri::Builder::manage(...)` in `main.rs`.

### 1.2 Authentication Module (`src-tauri/src/github/auth.rs`)

The keyring constants are hardcoded to a single entry:

```rust
const KEYRING_SERVICE: &str = "github-export";
const KEYRING_USER:    &str = "github-token";
```

Three helper functions manage this single entry:

| Function | Purpose |
|---|---|
| `store_token(token: &str)` | Saves token to OS credential store |
| `load_token()` | Retrieves token from OS credential store |
| `delete_token()` | Removes token from OS credential store |

All three are private, called internally by Tauri commands.

### 1.3 Tauri Commands (auth-related, in `auth.rs` and `main.rs`)

| Command | Signature | Behavior |
|---|---|---|
| `start_device_flow` | `(app_handle) -> DeviceFlowStart` | Starts OAuth flow, opens browser |
| `poll_device_flow` | `(device_code, expires_in, interval, state) -> String` | Polls for token, calls `store_token`, sets `app.client/token/username` |
| `authenticate_with_pat` | `(token, state) -> String` | Validates PAT, calls `store_token`, sets `app.client/token/username` |
| `restore_session` | `(state) -> Option<String>` | Calls `load_token`, rebuilds `app.client`, sets `app.username` |
| `logout` | `(state) -> ()` | Nulls `app.client/token/username`, calls `delete_token` |

### 1.4 Non-Auth Tauri Commands (all in `main.rs`)

All read-only data commands follow the same pattern — lock `AppState`, clone `app.client`, pass it to the API function:

- `list_repos(state)`
- `fetch_issues(owner, repo, filters, state)`
- `fetch_pulls(owner, repo, filters, state)`
- `fetch_security_alerts(owner, repo, state, app_state)`
- `get_pull_detail(owner, repo, pull_number, state)`
- `export_data(format, issues, pulls, alerts, file_path)` — no auth dependency

### 1.5 Frontend (`src/main.js`, `src/index.html`)

- Single `#username` span in the sidebar header showing `@<login>`.
- Single `#logout-btn` button next to the username.
- `showApp(username)` function that transitions from login screen to main app and calls `loadRepos()`.
- PAT form (`#pat-input` / `#pat-submit-btn`) and OAuth device flow (`#signin-btn`) both call `showApp(username)` on success.
- No concept of multiple accounts at any layer.

### 1.6 Cargo Dependencies Relevant to This Feature

```toml
keyring = { version = "3", features = ["apple-native", "windows-native", "sync-secret-service"] }
serde      = { version = "1", features = ["derive"] }
serde_json = "1"
```

No UUID or random-ID crate is currently present. The `uuid` crate will be needed.

---

## 2. Proposed Data Model

### 2.1 New `Account` Struct

Add to `src-tauri/src/models/mod.rs`:

```rust
/// A single stored GitHub account (PAT or OAuth token).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Account {
    /// Stable unique identifier (UUID v4).
    pub id: String,
    /// User-visible display name (e.g. "Work", "@octocat", or just the GitHub login).
    /// Defaults to the GitHub login name resolved at add-time.
    pub label: String,
    /// Resolved GitHub username (login) at the time the account was added.
    pub username: String,
}

/// Serialisable view of an account returned to the frontend.
/// Identical to `Account` but adding `is_active` for the switcher.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountInfo {
    pub id: String,
    pub label: String,
    pub username: String,
    pub is_active: bool,
}
```

### 2.2 Updated `AppState`

```rust
#[derive(Default)]
pub struct AppState {
    // ── Active session (same as today) ──────────
    pub client:            Option<Octocrab>,
    pub token:             Option<String>,
    pub username:          Option<String>,

    // ── Multi-account additions ──────────────────
    /// The id of the currently active account (matches one entry in `accounts`).
    pub active_account_id: Option<String>,
    /// All known accounts (metadata only — tokens stay in the keyring).
    pub accounts:          Vec<Account>,
}
```

`AppState` still derives `Default`, which zero-initialises both new fields.

### 2.3 Account Index in the Keyring

The account list (`Vec<Account>`) is persisted as a JSON blob in a dedicated keyring entry:

| Field | Value |
|---|---|
| service | `"github-export"` |
| username | `"accounts-index"` |
| password (stored value) | JSON-serialised `Vec<Account>` |

**Rationale:** The `keyring` crate on Windows (Credential Manager), macOS (Keychain), and Linux (libsecret) exposes no enumeration API — you cannot list all entries for a service. A manifest entry is therefore the only portable way to track which accounts exist without a separate database file.

### 2.4 Per-Account Token Storage Key

Each account's token is stored as a separate keyring entry:

| Field | Value |
|---|---|
| service | `"github-export"` |
| username | `"token-<account-id>"` (e.g., `"token-550e8400-e29b-41d4-a716-446655440000"`) |
| password | Raw GitHub token string |

**Rationale:** Storing tokens individually (rather than in the manifest blob) means that the JSON manifest never contains secret material. This reduces the blast radius of any manifest serialisation bug.

---

## 3. Tauri Commands

### 3.1 New Commands (to be added in `src-tauri/src/github/auth.rs`)

---

#### `list_accounts`

```rust
#[tauri::command]
pub fn list_accounts(
    state: tauri::State<'_, Mutex<AppState>>,
) -> Result<Vec<AccountInfo>, String>
```

- Locks `AppState`, reads `app.accounts` and `app.active_account_id`.
- Maps each `Account` to `AccountInfo { ..., is_active: account.id == active_id }`.
- **Does not** touch the keyring (accounts are already loaded into memory on session restore).
- Returns empty `Vec` if no accounts are stored.

---

#### `add_account`

```rust
#[tauri::command]
pub async fn add_account(
    token: String,
    label: Option<String>,
    state: tauri::State<'_, Mutex<AppState>>,
) -> Result<AccountInfo, String>
```

- Validates the token by calling `authenticate_with_token(&token)` and resolving the GitHub username.
- Checks that no existing account has the same resolved `username` to prevent duplicates.
- Generates a new UUID v4 for `id`.
- Uses `label.unwrap_or_else(|| username.clone())` as the display name.
- Stores the token in the keyring under `"token-<id>"`.
- Appends the new `Account` to `app.accounts` in memory.
- Persists the updated account index to the keyring under `"accounts-index"`.
- Returns the new `AccountInfo` (marking it `is_active: false` — a separate `switch_account` call makes it active).

---

#### `switch_account`

```rust
#[tauri::command]
pub async fn switch_account(
    account_id: String,
    state: tauri::State<'_, Mutex<AppState>>,
) -> Result<String, String>
```

- Looks up `account_id` in `app.accounts` — returns an error if not found.
- Loads the token from the keyring entry `"token-<account_id>"`.
- Builds a new `Octocrab` client via `authenticate_with_token(&token)`.
- Updates `app.client`, `app.token`, `app.username`, and `app.active_account_id` atomically under the mutex.
- Returns the GitHub login name of the newly active account (for frontend display).

---

#### `remove_account`

```rust
#[tauri::command]
pub fn remove_account(
    account_id: String,
    state: tauri::State<'_, Mutex<AppState>>,
) -> Result<(), String>
```

- Removes the keyring entry `"token-<account_id>"` (ignores not-found errors gracefully).
- Removes the `Account` from `app.accounts`.
- If `account_id == app.active_account_id`, clears `app.client`, `app.token`, `app.username`, and `app.active_account_id`.
- Persists the updated account index (`app.accounts`) to the keyring.
- **Does not** auto-switch to another account — the frontend must call `switch_account` or redirect to the login screen.

---

### 3.2 Modified Existing Commands

---

#### `restore_session` (modified)

**Current behaviour:** Loads the single `"github-token"` keyring entry and authenticates.

**New behaviour:**

```rust
#[tauri::command]
pub async fn restore_session(
    state: tauri::State<'_, Mutex<AppState>>,
) -> Result<Option<RestoreResult>, String>
```

Where `RestoreResult` is:

```rust
#[derive(Debug, Serialize, Deserialize)]
pub struct RestoreResult {
    pub username: String,
    pub accounts: Vec<AccountInfo>,
}
```

New logic:

1. Attempt to load the account index from the keyring (`"accounts-index"`).
2. **Migration:** If the index is absent, check for the legacy `"github-token"` entry  
   and, if found, import it as the first account (label = resolved username).  
   Delete the `"github-token"` entry after migration.
3. If no accounts exist at all, return `Ok(None)` — show the login screen.
4. Determine which account to restore: prefer the last-used account. A `"active-account-id"` keyring entry stores the last-active account ID between sessions.
5. Load that account's token and build the Octocrab client.
6. Populate `app.accounts`, `app.active_account_id`, `app.client`, `app.token`, `app.username`.
7. Return `Ok(Some(RestoreResult { username, accounts }))`.

---

#### `logout` (modified)

**Current behaviour:** Clears in-memory state and deletes the single token from the keyring.

**New behaviour (renamed semantics — "disconnect current account"):**

The command name stays `logout` to preserve backend/frontend symmetry, but its behaviour changes:

- Clears `app.client`, `app.token`, `app.username`, `app.active_account_id`.
- **Does not remove the account or its token** from the keyring — the account remains available to switch back to.
- Deletes the `"active-account-id"` keyring entry so the next `restore_session` returns `None`.
- Frontend should redirect to the account-switcher or login screen.

A separate action in the UI ("Remove account") calls `remove_account` to permanently delete.

---

#### `authenticate_with_pat` (modified)

**Current behaviour:** Validates PAT, stores token in single keyring slot, populates `AppState`.

**New behaviour:**

This command becomes a thin wrapper around `add_account` + `switch_account`:

```rust
#[tauri::command]
pub async fn authenticate_with_pat(
    token: String,
    label: Option<String>,
    state: tauri::State<'_, Mutex<AppState>>,
) -> Result<String, String>
```

- Calls the `add_account` logic (validates, stores in keyring, appends to index).
- Immediately calls the `switch_account` logic to make the new account active.
- Returns the GitHub username.

---

#### `poll_device_flow` (modified)

Same pattern as `authenticate_with_pat`:

- On token acquisition success, calls `add_account` logic then `switch_account` logic.
- Returns `Ok(username)` as before.

---

### 3.3 Helper Functions to Add in `auth.rs`

```rust
/// Load the account index from the keyring.
fn load_account_index() -> Result<Vec<Account>>

/// Persist the account index to the keyring.
fn save_account_index(accounts: &[Account]) -> Result<()>

/// Load a token for a specific account ID.
fn load_account_token(account_id: &str) -> Result<String>

/// Store a token for a specific account ID.
fn store_account_token(account_id: &str, token: &str) -> Result<()>

/// Delete a token for a specific account ID.
fn delete_account_token(account_id: &str) -> Result<()>

/// Load the active account ID preference.
fn load_active_account_id() -> Result<Option<String>>

/// Persist the active account ID preference.
fn save_active_account_id(account_id: &str) -> Result<()>

/// Delete the active account ID preference.
fn delete_active_account_id() -> Result<()>
```

`load_active_account_id` / `save_active_account_id` use the keyring entry:  
service = `"github-export"`, username = `"active-account-id"`.

---

### 3.4 Command Registration (`main.rs`)

Add to `tauri::generate_handler![...]`:

```
list_accounts,
add_account,
switch_account,
remove_account,
```

---

## 4. Keyring Storage Strategy

### 4.1 Key Schema

| Keyring Entry | `service` | `username` | `value` | Purpose |
|---|---|---|---|---|
| Account index | `github-export` | `accounts-index` | JSON `Vec<Account>` | Tracks all known accounts (no tokens) |
| Per-account token | `github-export` | `token-<uuid>` | Raw token string | Secret credential for account `<uuid>` |
| Active account ID | `github-export` | `active-account-id` | UUID string | Persists last-used account between launches |
| **Legacy (read-once)** | `github-export` | `github-token` | Raw token | Migrated from single-account version |

### 4.2 JSON Schema for `accounts-index`

```json
[
  {
    "id": "550e8400-e29b-41d4-a716-446655440000",
    "label": "octocat",
    "username": "octocat"
  },
  {
    "id": "7a38c2f0-bb3e-11ec-8422-0242ac120002",
    "label": "Work Account",
    "username": "corp-bot"
  }
]
```

### 4.3 Why Not a JSON Blob Containing Tokens?

Storing all tokens in one JSON blob (a common simpler approach) would mean:

1. **A single serialisation or deserialisation bug exposes all tokens at once.**
2. **OS credential stores have size limits** — Windows Credential Manager limits the secret value to 2,560 bytes. Multiple long PAT strings would quickly exhaust this.
3. Individual entries mean individual revocability: removing an account only touches one credential, not the entire store.

### 4.4 Enumeration Not Required

The `keyring` crate (v3) does not expose a `list_credentials` API on any platform. The `accounts-index` manifest entry solves this portably without relying on platform-specific enumeration.

### 4.5 Token Validation on Every Switch

When `switch_account` loads a token from the keyring, it always builds a fresh `Octocrab` client and resolves the current user from the GitHub API. This detects revoked tokens before the user encounters a 401 mid-session.

---

## 5. Frontend Changes

### 5.1 HTML Changes (`src/index.html`)

#### 5.1.1 Replace Sidebar Header

**Remove:**
```html
<div class="sidebar-header">
  <span id="username"></span>
  <button id="logout-btn" title="Logout">⎋</button>
</div>
```

**Replace with:**
```html
<div class="sidebar-header">
  <!-- Active account chip -->
  <div id="account-chip" class="account-chip">
    <span id="username" class="account-username"></span>
    <button id="account-menu-btn" class="account-menu-btn" title="Manage accounts" aria-haspopup="true" aria-expanded="false">▾</button>
  </div>

  <!-- Account dropdown menu (hidden by default) -->
  <div id="account-menu" class="account-menu hidden" role="menu">
    <div class="account-menu-section-label">Switch account</div>
    <ul id="account-list" class="account-switcher-list"></ul>
    <div class="account-menu-divider"></div>
    <button id="add-account-btn" class="account-menu-action" role="menuitem">+ Add account</button>
    <button id="remove-account-btn" class="account-menu-action account-menu-danger" role="menuitem">Remove this account</button>
    <div class="account-menu-divider"></div>
    <button id="logout-btn" class="account-menu-action" role="menuitem" title="Disconnect (keep account saved)">Disconnect</button>
  </div>
</div>
```

#### 5.1.2 Add Account Modal

Insert before the closing `</body>` tag (after `<script src="main.js">`):

```html
<!-- Add Account modal overlay -->
<div id="add-account-modal" class="modal-overlay hidden" role="dialog" aria-modal="true" aria-labelledby="add-account-title">
  <div class="modal-card">
    <h2 id="add-account-title" class="modal-title">Add GitHub Account</h2>
    <p class="modal-subtitle">Enter a Personal Access Token with <code>repo</code> and <code>security_events</code> scopes.</p>
    <input id="add-account-label" type="text" placeholder="Display name (optional)" class="pat-input" autocomplete="off" />
    <input id="add-account-token" type="password" placeholder="ghp_… or github_pat_…" class="pat-input" autocomplete="off" spellcheck="false" />
    <div id="add-account-error" class="login-error hidden"></div>
    <div class="modal-actions">
      <button id="add-account-submit-btn" class="btn-github-signin">Add Account</button>
      <button id="add-account-cancel-btn" class="btn-cancel">Cancel</button>
    </div>
  </div>
</div>
```

#### 5.1.3 Active Account Indicator on Login Screen

In the login card, update the submit button text to reflect "Sign in" on first launch and "Add Account" when accounts already exist. This is handled dynamically in JS — no HTML change required beyond what is already there.

### 5.2 JavaScript Changes (`src/main.js`)

#### 5.2.1 New State Variables

```js
let accounts = [];           // AccountInfo[] — mirrors backend state
let activeAccountId = null;  // String — currently active account id
```

#### 5.2.2 Updated `DOMContentLoaded` Bootstrap

Replace the current `restore_session` call with:

```js
const result = await invoke("restore_session");
if (result) {
  accounts = result.accounts;
  activeAccountId = accounts.find(a => a.is_active)?.id ?? null;
  await showApp(result.username);
} else {
  // No accounts — show login screen as today
}
```

#### 5.2.3 Updated `showApp(username)`

```js
async function showApp(username) {
  usernameEl.textContent = `@${username}`;
  renderAccountSwitcher();           // NEW: populate the dropdown
  loginScreen.classList.add("hidden");
  appScreen.classList.remove("hidden");
  await loadRepos();
}
```

#### 5.2.4 `renderAccountSwitcher()`

```js
function renderAccountSwitcher() {
  const list = document.getElementById("account-list");
  list.innerHTML = "";
  accounts.forEach((acct) => {
    const li = document.createElement("li");
    li.className = "account-switcher-item" + (acct.is_active ? " account-active" : "");
    li.setAttribute("role", "menuitem");
    li.innerHTML = `
      <span class="account-item-username">@${esc(acct.username)}</span>
      <span class="account-item-label">${esc(acct.label)}</span>
      ${acct.is_active ? '<span class="account-active-dot" aria-label="Active">●</span>' : ""}
    `;
    if (!acct.is_active) {
      li.addEventListener("click", () => handleSwitchAccount(acct.id));
    }
    list.appendChild(li);
  });
}
```

#### 5.2.5 Account Menu Toggle

```js
document.getElementById("account-menu-btn").addEventListener("click", (e) => {
  e.stopPropagation();
  const menu = document.getElementById("account-menu");
  const btn = document.getElementById("account-menu-btn");
  const isOpen = !menu.classList.contains("hidden");
  menu.classList.toggle("hidden", isOpen);
  btn.setAttribute("aria-expanded", String(!isOpen));
});

// Close menu on outside click
document.addEventListener("click", () => {
  document.getElementById("account-menu")?.classList.add("hidden");
  document.getElementById("account-menu-btn")?.setAttribute("aria-expanded", "false");
});
```

#### 5.2.6 `handleSwitchAccount(accountId)`

```js
async function handleSwitchAccount(accountId) {
  document.getElementById("account-menu").classList.add("hidden");
  try {
    const username = await invoke("switch_account", { accountId });
    // Refresh accounts list from backend
    accounts = await invoke("list_accounts");
    activeAccountId = accountId;
    usernameEl.textContent = `@${username}`;
    renderAccountSwitcher();
    // Reset repo selection — repos are per-account
    selectedRepo = null;
    repos = [];
    issues = [];
    pulls = [];
    alerts = [];
    repoList.innerHTML = "";
    placeholder.classList.remove("hidden");
    await loadRepos();
  } catch (err) {
    alert(`Failed to switch account: ${err}`);
  }
}
```

#### 5.2.7 Add Account Modal Handlers

```js
document.getElementById("add-account-btn").addEventListener("click", () => {
  document.getElementById("account-menu").classList.add("hidden");
  document.getElementById("add-account-modal").classList.remove("hidden");
  document.getElementById("add-account-token").value = "";
  document.getElementById("add-account-label").value = "";
  document.getElementById("add-account-error").classList.add("hidden");
});

document.getElementById("add-account-cancel-btn").addEventListener("click", () => {
  document.getElementById("add-account-modal").classList.add("hidden");
});

document.getElementById("add-account-submit-btn").addEventListener("click", async () => {
  const token = document.getElementById("add-account-token").value.trim();
  const label = document.getElementById("add-account-label").value.trim() || null;
  const errEl = document.getElementById("add-account-error");
  if (!token) {
    errEl.textContent = "Please enter a Personal Access Token.";
    errEl.classList.remove("hidden");
    return;
  }
  document.getElementById("add-account-submit-btn").disabled = true;
  errEl.classList.add("hidden");
  try {
    const newAcct = await invoke("add_account", { token, label });
    // Switch to the new account immediately
    await handleSwitchAccount(newAcct.id);
    document.getElementById("add-account-modal").classList.add("hidden");
  } catch (err) {
    errEl.textContent = String(err);
    errEl.classList.remove("hidden");
    document.getElementById("add-account-submit-btn").disabled = false;
  }
});
```

#### 5.2.8 Remove Account Handler

```js
document.getElementById("remove-account-btn").addEventListener("click", async () => {
  const acct = accounts.find(a => a.is_active);
  if (!acct) return;
  const confirmed = confirm(`Remove account @${acct.username} from this app?\n\nYour GitHub token will be deleted from the OS credential store. You will not be logged out of GitHub itself.`);
  if (!confirmed) return;
  document.getElementById("account-menu").classList.add("hidden");
  try {
    await invoke("remove_account", { accountId: acct.id });
    accounts = await invoke("list_accounts");
    if (accounts.length === 0) {
      // No accounts left — go back to login screen
      loginScreen.classList.remove("hidden");
      appScreen.classList.add("hidden");
      repos = []; issues = []; pulls = []; alerts = [];
      selectedRepo = null;
    } else {
      // Switch to the first remaining account
      await handleSwitchAccount(accounts[0].id);
    }
  } catch (err) {
    alert(`Failed to remove account: ${err}`);
  }
});
```

#### 5.2.9 Logout (Disconnect) Handler

The existing `logoutBtn` listener is replaced:

```js
logoutBtn.addEventListener("click", async () => {
  await invoke("logout");
  accounts = [];
  activeAccountId = null;
  // Return to login screen
  loginScreen.classList.remove("hidden");
  appScreen.classList.add("hidden");
  document.getElementById("pat-input").value = "";
  document.getElementById("pat-submit-btn").disabled = false;
  document.getElementById("signin-btn").disabled = false;
  document.getElementById("device-code-card").classList.add("hidden");
  loginError.classList.add("hidden");
});
```

#### 5.2.10 Post-Auth from Login Screen

After successful `authenticate_with_pat` or `poll_device_flow`, refresh the accounts list:

```js
// In pat-submit-btn click handler, after successful invoke:
const username = await invoke("authenticate_with_pat", { token, label: null });
accounts = await invoke("list_accounts");
activeAccountId = accounts.find(a => a.is_active)?.id ?? null;
await showApp(username);
```

Same pattern applies to the device flow success path.

### 5.3 CSS Changes (`src/styles.css`)

New selectors needed:

```css
/* Account chip in sidebar header */
.account-chip { display: flex; align-items: center; gap: 6px; }
.account-menu-btn { background: none; border: none; cursor: pointer; color: inherit; }

/* Dropdown menu */
.account-menu {
  position: absolute;
  top: 100%;
  left: 0;
  right: 0;
  background: var(--sidebar-bg, #24292e);
  border: 1px solid var(--border-color, #444);
  border-radius: 6px;
  z-index: 100;
  min-width: 200px;
  box-shadow: 0 4px 12px rgba(0,0,0,0.3);
}
.account-menu-section-label { font-size: 11px; color: #888; padding: 8px 12px 4px; }
.account-switcher-list { list-style: none; margin: 0; padding: 0; }
.account-switcher-item { padding: 8px 12px; cursor: pointer; display: flex; align-items: center; gap: 8px; }
.account-switcher-item:hover { background: rgba(255,255,255,0.08); }
.account-active { font-weight: 600; }
.account-active-dot { color: #2ea44f; font-size: 10px; margin-left: auto; }
.account-item-label { font-size: 11px; color: #888; }
.account-menu-divider { height: 1px; background: var(--border-color, #444); margin: 4px 0; }
.account-menu-action { display: block; width: 100%; text-align: left; padding: 8px 12px; background: none; border: none; cursor: pointer; color: inherit; }
.account-menu-action:hover { background: rgba(255,255,255,0.08); }
.account-menu-danger { color: #f85149; }

/* Modal overlay */
.modal-overlay {
  position: fixed; inset: 0;
  background: rgba(0,0,0,0.6);
  display: flex; align-items: center; justify-content: center;
  z-index: 200;
}
.modal-card {
  background: var(--card-bg, #1c2128);
  border: 1px solid var(--border-color, #444);
  border-radius: 10px;
  padding: 24px;
  width: 400px;
  max-width: 90vw;
}
.modal-title { margin: 0 0 8px; }
.modal-subtitle { margin: 0 0 16px; color: #888; font-size: 13px; }
.modal-actions { display: flex; gap: 8px; margin-top: 16px; }
```

---

## 6. Implementation Steps

### Phase A — Rust Backend

1. **Add `uuid` crate** to `src-tauri/Cargo.toml`:
   ```toml
   uuid = { version = "1", features = ["v4"] }
   ```

2. **Update `models/mod.rs`:**
   - Add `Account` struct (with `id`, `label`, `username`).
   - Add `AccountInfo` struct (with `is_active` field).
   - Add `RestoreResult` struct (with `username` and `accounts` fields) — or place in `auth.rs` if preferred.
   - Update `AppState` to add `active_account_id: Option<String>` and `accounts: Vec<Account>`.

3. **Update `github/auth.rs` — helper functions:**
   - Add `load_account_index()`.
   - Add `save_account_index(accounts: &[Account])`.
   - Add `load_account_token(account_id: &str)`.
   - Add `store_account_token(account_id: &str, token: &str)`.
   - Add `delete_account_token(account_id: &str)`.
   - Add `load_active_account_id()`.
   - Add `save_active_account_id(account_id: &str)`.
   - Add `delete_active_account_id()`.
   - Keep `store_token`, `load_token`, `delete_token` (for migration read path) but mark them `#[allow(dead_code)]` after migration logic is in place, or remove them post-migration.

4. **Update `github/auth.rs` — new Tauri commands:**
   - Implement `list_accounts`.
   - Implement `add_account`.
   - Implement `switch_account`.
   - Implement `remove_account`.

5. **Update `github/auth.rs` — modify existing commands:**
   - Refactor `restore_session` to the new multi-account logic including the legacy migration path.
   - Refactor `authenticate_with_pat` to accept `label: Option<String>` and call `add_account` + `switch_account` logic internally.
   - Refactor `poll_device_flow` to store new account and switch to it on success.
   - Update `logout` to clear session without deleting the account's token.

6. **Update `main.rs` — command registration:**
   - Add `list_accounts`, `add_account`, `switch_account`, `remove_account` to `tauri::generate_handler![...]`.

### Phase B — Frontend

7. **Update `src/index.html`:**
   - Replace sidebar header with account-chip + account-menu elements.
   - Add the "Add Account" modal overlay.

8. **Update `src/main.js`:**
   - Add `accounts` and `activeAccountId` state variables.
   - Update `DOMContentLoaded` bootstrap to use new `restore_session` return shape.
   - Update `showApp()` to call `renderAccountSwitcher()`.
   - Add `renderAccountSwitcher()` function.
   - Add account-menu toggle event listener.
   - Add `handleSwitchAccount(accountId)` function.
   - Add "Add Account" modal event listeners.
   - Add "Remove Account" event listener.
   - Update `logoutBtn` listener.
   - Update PAT form and OAuth device flow success handlers to refresh `accounts`.

9. **Update `src/styles.css`:**
   - Add all new CSS classes specified in §5.3.

### Phase C — Validation

10. **Run `cargo build`** from `src-tauri/` to confirm no compilation errors.
11. **Run `cargo clippy -- -D warnings`** from `src-tauri/`.
12. **Run `cargo test`** from `src-tauri/`.
13. **Run `npm run build`** from project root for full Tauri build.
14. **Manual smoke test:**
    - Add two accounts via PAT.
    - Switch between them and verify repo list changes.
    - Remove one account and verify the other remains.
    - Restart the app and verify the last-active account is restored automatically.
    - Test legacy migration: copy a `"github-token"` keyring entry and verify it is imported.

---

## 7. Dependencies

### 7.1 New Cargo Dependency

| Crate | Version | Features | Purpose |
|---|---|---|---|
| `uuid` | `"1"` | `["v4"]` | Generate stable unique identifiers for accounts |

Add to `src-tauri/Cargo.toml`:
```toml
uuid = { version = "1", features = ["v4"] }
```

### 7.2 No New JavaScript Libraries Required

The account switcher UI is implemented with vanilla JS and the existing `esc()` helper. No additional NPM packages are needed.

### 7.3 No New Tauri Plugins Required

All required OS credential store operations are already provided by `keyring = "3"`.

---

## 8. Risks and Mitigations

### 8.1 Security: Token Exposure in JSON Index

**Risk:** The `accounts-index` keyring entry is a JSON blob. If it were to accidentally include tokens, all credentials would be exposed in a single read.

**Mitigation:** The `Account` struct intentionally does not contain a `token` field. Tokens are stored exclusively in separate `"token-<id>"` keyring entries. Code review must enforce this separation.

### 8.2 Security: OS Keyring Access Control

**Risk:** On Linux with `libsecret`, other local applications can query the Secret Service for credentials if the user is already unlocked. This is the same risk as the current single-account design.

**Mitigation:** No change from the current posture. Document in README that users should use full-disk encryption and OS-level access controls. This is a systemic limitation of the `keyring` crate and the platforms it supports, not specific to this feature.

### 8.3 Migration: Lost Token on Upgrade

**Risk:** A user who upgraded from the single-account version opens the app and their session is gone because `"github-token"` is no longer read.

**Mitigation:** `restore_session` always checks for the legacy `"github-token"` entry first. If found:
  - Import as a new account with the resolved GitHub username as both `id`-source and `label`.
  - Delete `"github-token"` after successful import (no double-storage).
  - Proceed transparently — the user will not notice the upgrade.

### 8.4 Consistency: Mutex Lock Scope

**Risk:** Holding the `Mutex<AppState>` lock while making async HTTP calls (token validation, user resolution) would block all other Tauri commands.

**Mitigation:** Follow the existing pattern in the codebase: drop the lock before any `await` by cloning or moving the needed data out, then re-acquire the lock to commit results. For example, in `add_account`, resolve the username before locking.

### 8.5 Edge Case: Duplicate GitHub Accounts

**Risk:** A user adds the same GitHub account twice (with different PATs). Both tokens would be valid but the account list would show `@octocat` twice.

**Mitigation:** In `add_account`, after resolving the username, check `app.accounts.iter().any(|a| a.username == resolved_username)` and return an error: `"Account @octocat is already saved."`.

### 8.6 Edge Case: Revoked Token During Session

**Risk:** A token stored in the keyring is revoked with GitHub. The user tries to switch to that account and receives a 401 error from `switch_account`.

**Mitigation:** `switch_account` builds a fresh Octocrab client and immediately resolves the current user. A 401 is surfaced as a human-readable error: "Token for @octocat has been revoked. Please remove this account and add it again with a valid token." The account is not auto-removed — the user retains control.

### 8.7 Edge Case: Last Account Removed

**Risk:** The user removes the only stored account. The app is left in a state with no accounts and no active session.

**Mitigation:** In the `remove_account` JS handler, after calling `invoke("list_accounts")`, if `accounts.length === 0`, the frontend transitions back to the login screen. The login screen's `authenticate_with_pat`/`poll_device_flow` success path calls `add_account` + `switch_account` and transitions to `showApp()` as normal.

### 8.8 Keyring Size Limits

**Risk:** Windows Credential Manager limits a secret value to 2,560 bytes. The `accounts-index` JSON blob could theoretically exceed this with many accounts.

**Mitigation:** Each `Account` in the index is approximately 120 bytes of JSON. The limit would not be reached until ~21 accounts — an unlikely scenario for a desktop developer tool. If needed in future, the index could be split into pages. For now, no action required. A defensive check can log a warning if `serde_json::to_string(&accounts)` produces a value over 2,000 bytes.

### 8.9 UI: Account Menu Click-Outside Behaviour

**Risk:** The account dropdown staying open when the user clicks elsewhere is a poor UX pattern.

**Mitigation:** A `document.addEventListener("click", ...)` handler (see §5.2.5) closes the menu on any click outside `#account-menu-btn`. The handler uses `stopPropagation` on the button click to avoid immediately closing the menu it just opened.

### 8.10 `dev-mock` Feature Compatibility

**Risk:** The `dev-mock` feature uses `#[cfg(not(feature = "dev-mock"))]` guards on many commands. The new account management commands must follow the same pattern or the mock build will fail.

**Mitigation:** `list_accounts`, `add_account`, `switch_account`, `remove_account` interact with the OS keyring and GitHub API — they should be guarded with `#[cfg(not(feature = "dev-mock"))]`. A mock implementation in `src-tauri/src/mock/mod.rs` should be provided that returns a static list of fake accounts (consistent with the existing mock pattern for `restore_session`).

---

## Summary

This specification defines a complete multi-account system for GitHub Export that:

- Stores an arbitrary number of GitHub accounts (PAT or OAuth tokens) in the OS keyring using a manifest + per-account token strategy.
- Provides four new Tauri commands (`list_accounts`, `add_account`, `switch_account`, `remove_account`) and updates four existing commands.
- Migrates transparently from the legacy single-account keyring entry on first launch.
- Adds an account-switcher dropdown to the sidebar header with active-account indicator, add-account modal, and remove-account confirmation.
- Requires one new Cargo dependency (`uuid = "1"`) and no new JS libraries.
- Addresses all key security risks: token isolation, revocation detection, duplicate prevention, and mock-mode compatibility.
