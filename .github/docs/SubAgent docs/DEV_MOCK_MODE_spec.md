# DEV MOCK MODE — Specification

**Feature:** Dev Mock Mode (`dev-mock` Rust feature flag)  
**Project:** GitHub Export (Tauri v1, Rust + HTML/CSS/JS)  
**Date:** 2026-03-03  
**Status:** Draft  

---

## 1. Current State Analysis

### 1.1 All Tauri Commands and Their Return Types

| Command | Defined In | Return Type | Calls GitHub? |
|---------|-----------|-------------|---------------|
| `start_device_flow` | `src-tauri/src/github/auth.rs` | `Result<DeviceFlowStart, String>` | **YES** — POSTs to `github.com/login/device/code` |
| `poll_device_flow` | `src-tauri/src/github/auth.rs` | `Result<String, String>` (returns username) | **YES** — polls `github.com/login/oauth/access_token` |
| `restore_session` | `src-tauri/src/main.rs` | `Result<Option<String>, String>` (returns username or None) | **YES** — reads keyring, calls `client.current().user()` |
| `logout` | `src-tauri/src/main.rs` | `Result<(), String>` | NO — clears state + keyring delete |
| `list_repos` | `src-tauri/src/main.rs` | `Result<Vec<models::Repo>, String>` | **YES** — calls `issues::list_repos()` |
| `fetch_issues` | `src-tauri/src/main.rs` | `Result<Vec<models::Issue>, String>` | **YES** — calls `issues::fetch_issues()` |
| `fetch_pulls` | `src-tauri/src/main.rs` | `Result<Vec<models::PullRequest>, String>` | **YES** — calls `pulls::fetch_pulls()` |
| `fetch_security_alerts` | `src-tauri/src/main.rs` | `Result<Vec<models::SecurityAlert>, String>` | **YES** — calls `security::fetch_alerts()` |
| `export_data` | `src-tauri/src/main.rs` | `Result<String, String>` | NO — writes local file |

**New command to add:**

| Command | Return Type | Purpose |
|---------|-------------|---------|
| `get_dev_mode` | `bool` | Returns `true` when compiled with `dev-mock` feature; frontend uses this to show banner and skip login |

### 1.2 Exact Struct Definitions (from `src-tauri/src/models/mod.rs`)

```rust
pub struct Repo {
    pub id: u64,
    pub name: String,
    pub full_name: String,
    pub owner: String,
    pub description: Option<String>,
    pub private: bool,
    pub html_url: String,
    pub open_issues_count: u32,
}

pub struct Issue {
    pub number: u64,
    pub title: String,
    pub state: String,
    pub author: String,
    pub labels: Vec<String>,
    pub assignees: Vec<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub closed_at: Option<DateTime<Utc>>,
    pub html_url: String,
    pub body: Option<String>,
}

pub struct PullRequest {
    pub number: u64,
    pub title: String,
    pub state: String,
    pub author: String,
    pub labels: Vec<String>,
    pub reviewers: Vec<String>,
    pub head_branch: String,
    pub base_branch: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub merged_at: Option<DateTime<Utc>>,
    pub closed_at: Option<DateTime<Utc>>,
    pub html_url: String,
    pub draft: bool,
    pub body: Option<String>,
}

pub struct SecurityAlert {
    pub id: u64,
    pub severity: String,
    pub summary: String,
    pub description: String,
    pub package_name: Option<String>,
    pub vulnerable_version_range: Option<String>,
    pub patched_version: Option<String>,
    pub state: String,
    pub html_url: String,
    pub created_at: DateTime<Utc>,
}
```

### 1.3 `DeviceFlowStart` struct (from `src-tauri/src/github/auth.rs`)

```rust
pub struct DeviceFlowStart {
    pub user_code: String,
    pub verification_uri: String,
    pub device_code: String,
    pub expires_in: u64,
    pub interval: u64,
}
```

### 1.4 Frontend Boot Sequence (from `src/main.js`)

On `DOMContentLoaded`:
1. Calls `invoke("restore_session")`
2. If returns a username string → calls `showApp(user)` which hides login screen and shows app
3. If returns `null` → stays on login screen

**Mock entry point:** If `restore_session` returns `Some("octocat")` in mock mode, the entire login screen is bypassed naturally — no JS changes needed for the login skip, only a check for the dev banner.

### 1.5 Commands NOT Requiring Mocking

- `start_device_flow` and `poll_device_flow` — not called in mock mode because `restore_session` returns a user immediately, and the login screen is never shown. These commands remain as-is.
- `logout` — already safe: calls `let _ = github::auth::delete_token()` (result is discarded), so a missing keyring entry won't crash. Clears `AppState` correctly.
- `export_data` — purely local file I/O, no network calls. Remains as-is.

---

## 2. Implementation Plan

### 2.1 `Cargo.toml` Changes

Add `dev-mock` as an optional feature that implies no additional crate dependencies (all mock data is hardcoded).

**File:** `src-tauri/Cargo.toml`

```toml
[features]
default = ["custom-protocol"]
custom-protocol = ["tauri/custom-protocol"]
dev-mock = []
```

### 2.2 New File: `src-tauri/src/mock/mod.rs`

Create a new module containing all mock implementations. This entire module is gated by `#[cfg(feature = "dev-mock")]` so it produces zero code in production builds.

```rust
// src-tauri/src/mock/mod.rs
//
// Mock implementations of every GitHub-calling Tauri command.
// This module is compiled **only** when the `dev-mock` feature is active.
// Zero code ships in production builds.

#![cfg(feature = "dev-mock")]

use chrono::{TimeZone, Utc};
use crate::models::{Issue, PullRequest, Repo, SecurityAlert};

// ── Helper ─────────────────────────────────────────────────────────────────

fn dt(rfc3339: &str) -> chrono::DateTime<Utc> {
    chrono::DateTime::parse_from_rfc3339(rfc3339)
        .expect("hard-coded datetime must be valid RFC-3339")
        .with_timezone(&Utc)
}

// ── Mock Data ──────────────────────────────────────────────────────────────

/// Returns `Some("octocat")` — used by `restore_session` to auto-login.
pub fn mock_username() -> &'static str {
    "octocat"
}

/// Three realistic fake repositories.
pub fn mock_repos() -> Vec<Repo> {
    vec![
        Repo {
            id: 1001,
            name: "Hello-World".to_string(),
            full_name: "octocat/Hello-World".to_string(),
            owner: "octocat".to_string(),
            description: Some("My first repository on GitHub!".to_string()),
            private: false,
            html_url: "https://github.com/octocat/Hello-World".to_string(),
            open_issues_count: 5,
        },
        Repo {
            id: 1002,
            name: "linguist".to_string(),
            full_name: "octocat/linguist".to_string(),
            owner: "octocat".to_string(),
            description: Some("Language Savant. If your repository's language is wrong, send us a pull request!".to_string()),
            private: false,
            html_url: "https://github.com/octocat/linguist".to_string(),
            open_issues_count: 12,
        },
        Repo {
            id: 1003,
            name: "secret-repo".to_string(),
            full_name: "octocat/secret-repo".to_string(),
            owner: "octocat".to_string(),
            description: Some("A private test repository.".to_string()),
            private: true,
            html_url: "https://github.com/octocat/secret-repo".to_string(),
            open_issues_count: 2,
        },
    ]
}

/// Five realistic fake issues.
pub fn mock_issues() -> Vec<Issue> {
    vec![
        Issue {
            number: 1,
            title: "Bug: application crashes on startup when config is missing".to_string(),
            state: "Open".to_string(),
            author: "monalisa".to_string(),
            labels: vec!["bug".to_string(), "good first issue".to_string()],
            assignees: vec!["octocat".to_string()],
            created_at: dt("2025-11-01T09:00:00Z"),
            updated_at: dt("2025-11-10T14:30:00Z"),
            closed_at: None,
            html_url: "https://github.com/octocat/Hello-World/issues/1".to_string(),
            body: Some("Steps to reproduce:\n1. Delete config.toml\n2. Launch app\n3. Observe crash".to_string()),
        },
        Issue {
            number: 2,
            title: "Feature request: add dark mode support".to_string(),
            state: "Open".to_string(),
            author: "hubot".to_string(),
            labels: vec!["enhancement".to_string()],
            assignees: vec![],
            created_at: dt("2025-11-05T11:00:00Z"),
            updated_at: dt("2025-11-05T11:00:00Z"),
            closed_at: None,
            html_url: "https://github.com/octocat/Hello-World/issues/2".to_string(),
            body: Some("Would love a dark mode option in settings.".to_string()),
        },
        Issue {
            number: 3,
            title: "Docs: README missing installation instructions for Linux".to_string(),
            state: "Open".to_string(),
            author: "defunkt".to_string(),
            labels: vec!["documentation".to_string()],
            assignees: vec!["monalisa".to_string()],
            created_at: dt("2025-11-08T08:15:00Z"),
            updated_at: dt("2025-11-09T10:00:00Z"),
            closed_at: None,
            html_url: "https://github.com/octocat/Hello-World/issues/3".to_string(),
            body: Some("The README only covers macOS and Windows installation.".to_string()),
        },
        Issue {
            number: 4,
            title: "Performance: repo list loads slowly with >100 repos".to_string(),
            state: "Closed".to_string(),
            author: "pjhyett".to_string(),
            labels: vec!["performance".to_string()],
            assignees: vec![],
            created_at: dt("2025-10-15T16:00:00Z"),
            updated_at: dt("2025-10-20T12:00:00Z"),
            closed_at: Some(dt("2025-10-20T12:00:00Z")),
            html_url: "https://github.com/octocat/Hello-World/issues/4".to_string(),
            body: Some("Noticeable lag when the user has many repositories.".to_string()),
        },
        Issue {
            number: 5,
            title: "Security: tokens should not be logged to stdout".to_string(),
            state: "Closed".to_string(),
            author: "wanstrath".to_string(),
            labels: vec!["security".to_string(), "bug".to_string()],
            assignees: vec!["octocat".to_string()],
            created_at: dt("2025-10-01T13:45:00Z"),
            updated_at: dt("2025-10-02T09:30:00Z"),
            closed_at: Some(dt("2025-10-02T09:30:00Z")),
            html_url: "https://github.com/octocat/Hello-World/issues/5".to_string(),
            body: Some("Observed that the access token appears in debug logs.".to_string()),
        },
    ]
}

/// Three realistic fake pull requests.
pub fn mock_pulls() -> Vec<PullRequest> {
    vec![
        PullRequest {
            number: 10,
            title: "feat: add dark mode theme".to_string(),
            state: "Open".to_string(),
            author: "hubot".to_string(),
            labels: vec!["enhancement".to_string()],
            reviewers: vec!["octocat".to_string(), "monalisa".to_string()],
            head_branch: "hubot:feature/dark-mode".to_string(),
            base_branch: "octocat:main".to_string(),
            created_at: dt("2025-11-12T10:00:00Z"),
            updated_at: dt("2025-11-13T15:20:00Z"),
            merged_at: None,
            closed_at: None,
            html_url: "https://github.com/octocat/Hello-World/pull/10".to_string(),
            draft: false,
            body: Some("Implements dark mode via CSS variables. Closes #2.".to_string()),
        },
        PullRequest {
            number: 11,
            title: "fix: handle missing config file gracefully on startup".to_string(),
            state: "Open".to_string(),
            author: "monalisa".to_string(),
            labels: vec!["bug".to_string()],
            reviewers: vec!["defunkt".to_string()],
            head_branch: "monalisa:fix/startup-crash".to_string(),
            base_branch: "octocat:main".to_string(),
            created_at: dt("2025-11-14T08:30:00Z"),
            updated_at: dt("2025-11-14T08:30:00Z"),
            merged_at: None,
            closed_at: None,
            html_url: "https://github.com/octocat/Hello-World/pull/11".to_string(),
            draft: true,
            body: Some("WIP — creates a default config if none is found. Closes #1.".to_string()),
        },
        PullRequest {
            number: 9,
            title: "perf: paginate repo list to avoid N+1 API calls".to_string(),
            state: "Closed".to_string(),
            author: "pjhyett".to_string(),
            labels: vec!["performance".to_string()],
            reviewers: vec!["octocat".to_string()],
            head_branch: "pjhyett:perf/paginate-repos".to_string(),
            base_branch: "octocat:main".to_string(),
            created_at: dt("2025-10-16T14:00:00Z"),
            updated_at: dt("2025-10-19T11:00:00Z"),
            merged_at: Some(dt("2025-10-19T11:00:00Z")),
            closed_at: Some(dt("2025-10-19T11:00:00Z")),
            html_url: "https://github.com/octocat/Hello-World/pull/9".to_string(),
            draft: false,
            body: Some("Switches repo listing to paginated requests. Fixes #4.".to_string()),
        },
    ]
}

/// Two realistic fake security alerts.
pub fn mock_security_alerts() -> Vec<SecurityAlert> {
    vec![
        SecurityAlert {
            id: 1,
            severity: "critical".to_string(),
            summary: "Remote code execution in lodash via prototype pollution".to_string(),
            description: "Versions of lodash prior to 4.17.21 are vulnerable to prototype pollution via the `merge`, `mergeWith`, `defaultsDeep` and `set` functions.".to_string(),
            package_name: Some("lodash".to_string()),
            vulnerable_version_range: Some("< 4.17.21".to_string()),
            patched_version: Some("4.17.21".to_string()),
            state: "open".to_string(),
            html_url: "https://github.com/octocat/Hello-World/security/dependabot/1".to_string(),
            created_at: dt("2025-10-05T09:00:00Z"),
        },
        SecurityAlert {
            id: 2,
            severity: "high".to_string(),
            summary: "Path traversal vulnerability in tar extraction".to_string(),
            description: "The npm package `tar` allows arbitrary file writes by extracting specially crafted archives with `..` path segments.".to_string(),
            package_name: Some("tar".to_string()),
            vulnerable_version_range: Some("< 6.1.9".to_string()),
            patched_version: Some("6.1.9".to_string()),
            state: "open".to_string(),
            html_url: "https://github.com/octocat/Hello-World/security/dependabot/2".to_string(),
            created_at: dt("2025-10-12T14:30:00Z"),
        },
    ]
}
```

### 2.3 New Module Declaration in `src-tauri/src/main.rs`

Add `mod mock;` at the top of `main.rs`, gated by the feature flag:

```rust
mod export;
mod github;
mod models;

#[cfg(feature = "dev-mock")]
mod mock;
```

### 2.4 Changes to GitHub-Calling Commands in `main.rs`

Each GitHub-calling command is **defined twice**: once for the `dev-mock` feature (returns hardcoded data), and once for the normal case (existing implementation). This ensures **zero dead code in production**.

The pattern for each command:

```rust
// ── MOCK variant (only compiled when dev-mock feature is active) ──────────

#[cfg(feature = "dev-mock")]
#[tauri::command]
async fn restore_session(
    state: State<'_, Mutex<AppState>>,
) -> Result<Option<String>, String> {
    let mut app = state.lock().map_err(|e| e.to_string())?;
    app.username = Some(mock::mock_username().to_string());
    // app.client intentionally left as None — mock commands don't use it
    Ok(Some(mock::mock_username().to_string()))
}

// ── REAL variant (only compiled when dev-mock feature is NOT active) ──────

#[cfg(not(feature = "dev-mock"))]
#[tauri::command]
async fn restore_session(
    state: State<'_, Mutex<AppState>>,
) -> Result<Option<String>, String> {
    match github::auth::load_token() {
        // ... existing implementation unchanged ...
    }
}
```

Full command-by-command breakdown:

| Command | Mock Implementation |
|---------|-------------------|
| `restore_session` | Sets `app.username = Some("octocat")`, returns `Ok(Some("octocat"))` |
| `list_repos` | Returns `Ok(mock::mock_repos())` (ignores `State`) |
| `fetch_issues` | Returns `Ok(mock::mock_issues())` (ignores `owner`, `repo`, `filters`, `State`) |
| `fetch_pulls` | Returns `Ok(mock::mock_pulls())` (ignores `owner`, `repo`, `filters`, `State`) |
| `fetch_security_alerts` | Returns `Ok(mock::mock_security_alerts())` (ignores `owner`, `repo`, `State`) |

Commands **not duplicated** (use a single definition regardless of feature flag):  
`start_device_flow`, `poll_device_flow`, `logout`, `export_data` — these are unchanged. They either make no GitHub calls, or are unreachable in mock mode.

### 2.5 New `get_dev_mode` Command in `main.rs`

Add this single-definition command (always compiled, returns correct value):

```rust
/// Returns true when compiled with the `dev-mock` feature flag.
/// The frontend calls this on startup to show the DEV MODE banner.
#[tauri::command]
fn get_dev_mode() -> bool {
    cfg!(feature = "dev-mock")
}
```

Register it in `invoke_handler!`:

```rust
.invoke_handler(tauri::generate_handler![
    start_device_flow,
    poll_device_flow,
    restore_session,
    logout,
    list_repos,
    fetch_issues,
    fetch_pulls,
    fetch_security_alerts,
    export_data,
    get_dev_mode,   // ← add this
])
```

### 2.6 `package.json` Changes

Add the `dev:mock` npm script. For **Tauri v1 CLI**, cargo build args are passed after `-- `:

```json
{
  "scripts": {
    "dev": "tauri dev",
    "dev:mock": "tauri dev -- --features dev-mock",
    "build": "tauri build"
  }
}
```

**Note on Tauri v1 feature flag passthrough:**  
The Tauri v1 CLI (`@tauri-apps/cli`) passes everything after `--` directly to the underlying `cargo` invocation. So `tauri dev -- --features dev-mock` causes cargo to build with `--features dev-mock`. This is confirmed by the Tauri v1 CLI source and docs.

### 2.7 Frontend Changes: `src/index.html`

Add a DEV MODE banner `<div>` as the **first child of `<body>`**, hidden by default:

```html
<body>
  <!-- ─── Dev mode banner (hidden in production) ─── -->
  <div id="dev-mode-banner" class="hidden">
    ⚠ DEV MODE — Not connected to GitHub. All data is fake.
  </div>

  <!-- ─── Login screen ─── -->
  <div id="login-screen">
    ...
  </div>
  ...
</body>
```

### 2.8 Frontend Changes: `src/styles.css`

Add styles for the dev mode banner — a fixed, highly visible top bar:

```css
/* ── Dev mode banner ─────────────────────────────── */
#dev-mode-banner {
  position: fixed;
  top: 0;
  left: 0;
  right: 0;
  z-index: 9999;
  background: #f5a623;
  color: #1a1a1a;
  font-size: 13px;
  font-weight: 700;
  text-align: center;
  padding: 6px 12px;
  letter-spacing: 0.04em;
  border-bottom: 2px solid #d4880a;
  font-family: monospace;
}

/* Push all content down when banner is visible */
body:has(#dev-mode-banner:not(.hidden)) #login-screen,
body:has(#dev-mode-banner:not(.hidden)) #app-container {
  margin-top: 36px;
}
```

### 2.9 Frontend Changes: `src/main.js`

Add a `get_dev_mode` call in the `DOMContentLoaded` handler **before** `restore_session`, so the banner shows up immediately even while authentication resolves. If dev mode is active, the banner is shown. Because `restore_session` already returns a username in mock mode, `showApp()` is called automatically — no additional logic needed.

```javascript
document.addEventListener("DOMContentLoaded", async () => {
  // Check dev mode first — show banner before any auth attempt
  try {
    const isDevMode = await invoke("get_dev_mode");
    if (isDevMode) {
      const banner = document.getElementById("dev-mode-banner");
      if (banner) banner.classList.remove("hidden");
    }
  } catch (_) { /* non-fatal */ }

  // Attempt to restore a previous session (mock mode returns user immediately)
  try {
    const user = await invoke("restore_session");
    if (user) showApp(user);
  } catch (_) { /* no stored session */ }
});
```

---

## 3. Mock Data Design

### 3.1 Mock User
- **Username:** `octocat`

### 3.2 Mock Repos (3)

| id | full_name | private | open_issues_count |
|----|-----------|---------|-------------------|
| 1001 | `octocat/Hello-World` | false | 5 |
| 1002 | `octocat/linguist` | false | 12 |
| 1003 | `octocat/secret-repo` | true | 2 |

### 3.3 Mock Issues (5)

| number | title | state | author | labels |
|--------|-------|-------|--------|--------|
| 1 | Bug: application crashes on startup... | Open | monalisa | bug, good first issue |
| 2 | Feature request: add dark mode support | Open | hubot | enhancement |
| 3 | Docs: README missing installation for Linux | Open | defunkt | documentation |
| 4 | Performance: repo list loads slowly... | Closed | pjhyett | performance |
| 5 | Security: tokens should not be logged... | Closed | wanstrath | security, bug |

### 3.4 Mock Pull Requests (3)

| number | title | state | author | draft |
|--------|-------|-------|--------|-------|
| 10 | feat: add dark mode theme | Open | hubot | false |
| 11 | fix: handle missing config file gracefully | Open | monalisa | true |
| 9 | perf: paginate repo list | Closed | pjhyett | false (merged) |

### 3.5 Mock Security Alerts (2)

| id | severity | package | vulnerable range | patched |
|----|----------|---------|-----------------|---------|
| 1 | critical | lodash | `< 4.17.21` | `4.17.21` |
| 2 | high | tar | `< 6.1.9` | `6.1.9` |

All datetime values are hard-coded RFC-3339 strings parsed at runtime via `chrono::DateTime::parse_from_rfc3339`.

---

## 4. The `get_dev_mode` Command

```rust
/// Returns `true` when compiled with the `dev-mock` Cargo feature.
/// Always compiled regardless of feature flag.
/// Used by the frontend to show the DEV MODE banner and skip login.
#[tauri::command]
fn get_dev_mode() -> bool {
    cfg!(feature = "dev-mock")
}
```

Since `cfg!()` is a compile-time macro, in a production build this always evaluates to `false` with zero runtime overhead. The command is safe to ship in all builds.

---

## 5. Implementation Steps (Ordered)

1. **`src-tauri/Cargo.toml`** — Add `dev-mock = []` to `[features]`.

2. **`src-tauri/src/mock/mod.rs`** — Create the new file with the full `#![cfg(feature = "dev-mock")]` module containing `mock_username()`, `mock_repos()`, `mock_issues()`, `mock_pulls()`, and `mock_security_alerts()` as specified above.

3. **`src-tauri/src/main.rs`** — Make the following changes:
   a. Add `#[cfg(feature = "dev-mock")] mod mock;` after `mod models;`
   b. Add the `get_dev_mode()` command (single unconditional definition)
   c. For each of the 5 GitHub-calling commands (`restore_session`, `list_repos`, `fetch_issues`, `fetch_pulls`, `fetch_security_alerts`):
      - Wrap the existing implementation in `#[cfg(not(feature = "dev-mock"))]`
      - Add a parallel `#[cfg(feature = "dev-mock")]` variant that delegates to the `mock` module
   d. Add `get_dev_mode` to the `tauri::generate_handler![]` macro

4. **`package.json`** — Add `"dev:mock": "tauri dev -- --features dev-mock"` to the `scripts` object.

5. **`src/index.html`** — Add `<div id="dev-mode-banner" class="hidden">` as the first child of `<body>`.

6. **`src/styles.css`** — Add CSS for `#dev-mode-banner` (fixed top bar, orange background).

7. **`src/main.js`** — Add `get_dev_mode` call at the top of the `DOMContentLoaded` handler.

8. **Verify** — Run `npm run dev:mock` and confirm:
   - App opens directly to the main view with `@octocat` in the sidebar
   - Orange "⚠ DEV MODE" banner is visible at top
   - All 3 repos appear in the sidebar
   - Clicking a repo loads mock issues, PRs, and alerts
   - Export still works (uses whatever data is in state)

9. **Verify production** — Run `npm run build` and confirm:
   - `get_dev_mode` returns `false` (banner never shown)
   - No mock code compiled in (run `cargo build --release` from `src-tauri/`, check binary size is not inflated)
   - Login screen shown normally

---

## 6. Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|------------|
| `tauri dev -- --features dev-mock` syntax incorrect for installed CLI version | Low | High | Verify with `tauri --version`; if Tauri CLI v1 < 1.4, the passthrough may require `tauri dev` with `CARGO_BUILD_FEATURES` env instead |
| Mock `restore_session` leaves `app.client = None`; any code path that unwraps `client` outside of the 5 mocked commands would panic | Low | Medium | All 5 GitHub-calling commands are mocked; `logout` ignores `app.client`; `export_data` doesn't use it. All paths covered. |
| `body:has(...)` CSS selector not supported in older WebView2/webkit versions | Low | Low | Banner display logic can be moved to JS: add a class to `<body>` from JS instead of relying on `:has()` |
| `mock/mod.rs` module declaration without `#[cfg]` guard causes "unused import" warnings in non-mock builds | Low | Low | Guard the `mod mock;` declaration with `#[cfg(feature = "dev-mock")]` |
| Developer forgets banner and ships a screenshot with mock data as if it were real | Low | Medium | Banner is designed to be unmissable (fixed, full-width, bright orange, z-index 9999) |
| `chrono::DateTime::parse_from_rfc3339` panic on typo in hardcoded mock datetime | Low | Medium | String values are compile-time constants; use `.expect("hard-coded datetime must be valid RFC-3339")` to give clear error message during development |
| Production binary contains dead branches from `#[cfg(not(feature = "dev-mock"))]` when only `default` feature is used | None | None | Rust compiler DCE (dead code elimination) + `#[cfg]` guarantees zero dead code. Not a risk. |

---

## 7. File Change Summary

| File | Change Type |
|------|------------|
| `src-tauri/Cargo.toml` | Edit — add `dev-mock = []` feature |
| `src-tauri/src/mock/mod.rs` | **Create** — new mock data module |
| `src-tauri/src/main.rs` | Edit — add `mod mock`, `get_dev_mode`, dual `#[cfg]` command variants |
| `package.json` | Edit — add `dev:mock` script |
| `src/index.html` | Edit — add `#dev-mode-banner` div |
| `src/styles.css` | Edit — add banner styles |
| `src/main.js` | Edit — call `get_dev_mode` on startup |

---

*End of DEV_MOCK_MODE_spec.md*
