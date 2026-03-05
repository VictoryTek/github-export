# Multi-Account Feature — Code Review

**Feature:** Multi-Account Support  
**Project:** GitHub Export (Tauri v1)  
**Reviewer:** QA Subagent  
**Date:** 2026-03-05  
**Spec Reference:** `.github/docs/SubAgent docs/multi_account_spec.md`

---

## Build Validation Results

| Command | Exit Code | Result |
|---------|-----------|--------|
| `cargo build` | 0 | **PASS** |
| `cargo clippy -- -D warnings` | 0 | **PASS** |
| `cargo test` | 0 | **PASS** (0 tests exist) |

All three build checks passed without errors or warnings. The Rust backend compiles cleanly and all new dependencies (`uuid = { version = "1", features = ["v4"] }`) are correctly declared in `Cargo.toml`.

---

## Detailed Findings

---

### 1. Specification Compliance

**Score: 92% — A-**

#### ✓ PASSED

- **`list_accounts`**: Implemented as a synchronous `#[tauri::command]` in `auth.rs`. Correctly reads `app.accounts` and `app.active_account_id` under the mutex, maps to `AccountInfo`, and returns `Result<Vec<AccountInfo>, String>`.

- **`add_account`**: Implemented as async with `token: String, label: Option<String>` signature. Performs token validation and username resolution before acquiring the lock (correct pattern). Checks for duplicate usernames. Generates UUID v4 for `id`. Stores token in keyring via `?` propagation (hard error on keyring write failure). Appends to accounts and persists index. Returns `AccountInfo`.

- **`switch_account`**: Implemented correctly with lock-release-IO-relock discipline. Validates account existence, loads token from keyring without holding lock, builds Octocrab client and resolves current user via API call, persists `active-account-id`, then commits all state under lock. Returns resolved username.

- **`remove_account`**: Deletes keyring token first (using `?`), removes from accounts list, persists updated index, clears active session fields if the removed account was active. Calls `delete_active_account_id()` on active removal.

- **`restore_session`**: Updated to return `Option<RestoreResult>`. Loads account index from keyring. Includes legacy migration path. Correctly populates `app.accounts` before returning so later operations see them. Prefers `active-account-id` keyring entry, falls back to first account.

- **`authenticate_with_pat`**: Updated with `label: Option<String>` parameter. Follows add-then-activate pattern inline (functionally equivalent to calling `add_account` + `switch_account`). Returns username string.

- **`poll_device_flow`**: Updated to use multi-account pattern. Handles the case where a matching username already exists (updates its token) vs. brand-new account.

- **Command registration**: All four new commands (`list_accounts`, `add_account`, `switch_account`, `remove_account`) are registered in `tauri::generate_handler![...]` in `main.rs`. Confirmed present.

- **Keyring manifest strategy**: Correctly implemented. `accounts-index` entry holds JSON `Vec<Account>` (no tokens). Each account token stored separately under `token-<uuid>`. `active-account-id` entry persists last-used account.

- **`AppState`** updated with `active_account_id: Option<String>` and `accounts: Vec<Account>`. `Account` and `AccountInfo` structs added to `models/mod.rs`. `RestoreResult` struct added and used correctly.

#### ⚠ MINOR DEVIATIONS

- **Code duplication**: The spec states `authenticate_with_pat` and `poll_device_flow` should be "thin wrappers" around `add_account` + `switch_account` logic. The implementation reimplements the logic inline rather than calling the Tauri command functions directly. This is functionally equivalent (Tauri commands cannot easily call each other when they take `tauri::State` parameters) but results in ~60 lines of duplicated logic.

---

### 2. Best Practices

**Score: 78% — C+**

#### ✓ PASSED

- No unchecked `.unwrap()` calls in new code paths. All new code uses `?`, `.map_err(|e| e.to_string())`, `unwrap_or_default()`, or explicit match.
- All Tauri commands use `Result<T, String>` return types consistently.
- HTTP calls and keyring I/O occur outside the mutex lock (correct async pattern).
- `Option` handling is exhaustive throughout.

#### ⚠ ISSUES FOUND

**[MODERATE] Inconsistent keyring error propagation across auth flows:**

`add_account` uses the `?` operator for keyring writes (hard error):
```rust
store_account_token(&id, &token).map_err(|e| e.to_string())?;
```

But `poll_device_flow` and `authenticate_with_pat` (for the existing-account update branch) use silent `eprintln!` warnings:
```rust
if let Err(e) = store_account_token(&existing_id, &token) {
    eprintln!("Warning: could not update token in keyring: {e}");
}
```

This inconsistency means that if keyring writes fail during device flow or PAT auth, the account is added to in-memory state but the token is not persisted. On next app launch, `restore_session` will silently fail to load the token for that account (returns `Ok(None)` for token-load failure), leaving users confused about why their session didn't restore.

---

### 3. Security

**Score: 75% — C**

#### ✓ PASSED

- Token values are never logged or returned to the frontend.
- In `add_account`, the keyring write uses `?`, ensuring the token **is successfully stored before** `app.accounts.push(account)` executes. Correct ordering.
- `remove_account` deletes the keyring entry first using `?` before modifying in-memory state.
- No token material appears in the `accounts-index` JSON blob (tokens stored in isolated entries).

#### ✗ CRITICAL ISSUE FOUND

**[CRITICAL] Data loss in legacy migration — token deleted even on migration failure:**

In `restore_session`, the legacy `github-token` keyring entry is **always deleted** regardless of whether migration to the new format succeeded:

```rust
if accounts.is_empty() {
    if let Ok(legacy_token) = load_token() {
        if let Ok(client) = authenticate_with_token(&legacy_token).await {
            if let Ok(user) = client.current().user().await {
                // ...
                if store_account_token(&id, &legacy_token).is_ok() {
                    accounts.push(account);
                    let _ = save_account_index(&accounts);
                }
            }
        }
        // ⚠ THIS RUNS EVEN IF store_account_token FAILED
        let _ = delete_token();   // <── data loss bug
    }
}
```

**Failure scenarios:**
1. Network unavailable at startup → `authenticate_with_token` fails → legacy token is deleted → user cannot log in on next restart even when network is restored.
2. OS keyring write fails for `token-<uuid>` → `store_account_token` fails → `delete_token()` still removes the legacy entry → permanent session loss.

**Fix required:** Move `delete_token()` inside the `if store_account_token(&id, &legacy_token).is_ok()` block, so the legacy token is only removed after successful migration.

---

### 4. Functionality

**Score: 85% — B**

#### ✓ PASSED

- Account switcher (`renderAccountSwitcher`) correctly renders all accounts, marks active with `.account-active` class and `●` indicator, and attaches click handlers only to inactive accounts.
- `handleSwitchAccount` calls `invoke("list_accounts")` to refresh the JS state, updates `usernameEl`, resets repo/issue/PR/alert state, and calls `await loadRepos()`.
- `add_account` modal correctly opens/closes, passes `{ token, label }` to Rust.
- `remove_account` flow: confirms with the user, calls `remove_account`, then either redirects to login (no accounts remain) or switches to the first available account.
- Auth flows (`authenticate_with_pat` and `poll_device_flow`) correctly invoke `list_accounts` after success and call `showApp(username)`.
- `logout` (disconnect) correctly clears JS state (`accounts = []`) and shows login screen without removing accounts from keyring.

#### ⚠ MINOR ISSUE

**[MINOR] `add-account-submit-btn` not re-enabled after a successful add-account operation:**

The modal open handler does not reset `disabled` state:
```js
document.getElementById("add-account-btn").addEventListener("click", () => {
  document.getElementById("add-account-modal").classList.remove("hidden");
  document.getElementById("add-account-token").value = "";
  document.getElementById("add-account-label").value = "";
  document.getElementById("add-account-error").classList.add("hidden");
  // ← missing: document.getElementById("add-account-submit-btn").disabled = false;
});
```

After a successful account addition (button clears, modal hides), if the user opens "Add Account" again, the submit button remains disabled and the modal is unusable until page reload.

---

### 5. Code Quality

**Score: 82% — B-**

#### ✓ PASSED

- Import section in `auth.rs` is clean and minimal.
- `models/mod.rs` is well-structured and types are correctly derived.
- Naming is consistent with Rust conventions (snake_case, descriptive).
- No dead code introduced (confirmed by `clippy` passing with `-D warnings`).

#### ⚠ ISSUES

- **No new tests added**: `cargo test` reports 0 tests. The multi-account feature adds significant new logic (8 new helper functions, 4 new Tauri commands) with no unit test coverage. While the project had no existing tests, the complexity of the migration path and keyring interaction warrants at least integration-level tests.
- **Duplication**: Authentication logic is implemented three times (`authenticate_with_pat`, `poll_device_flow`, and implicitly in `add_account`). If account-creation logic changes in `add_account`, the other two will diverge.

---

### 6. Performance

**Score: 92% — A-**

#### ✓ PASSED

- All async HTTP operations (token validation, user resolution) occur outside the `Mutex<AppState>` lock. The lock is held only for in-memory state reads/writes.
- `switch_account` correctly releases the lock for keyring I/O and HTTP, then reacquires for state commit.
- `list_accounts` is synchronous (no I/O) and appropriately so — the data is already in memory.
- No unnecessary clones detected in hot paths.

---

### 7. Consistency

**Score: 88% — B+**

#### ✓ PASSED

- New CSS classes (`.account-chip`, `.account-menu`, `.account-switcher-item`, `.account-active`, `.account-active-dot`, `.modal-overlay`, `.modal-card`, `.modal-actions`, etc.) are all present in `styles.css` and match the HTML elements they style.
- Account menu dropdown color scheme and button styles match the existing sidebar design language.
- `esc()` helper used consistently throughout the new `renderAccountSwitcher()` to prevent XSS.
- Existing PAT auth, device flow, and logout flows are all preserved and still functional with no breaking changes.
- `invoke()` parameter name casing is correct: JS camelCase (`accountId`, `deviceCode`, `expiresIn`) maps to Rust snake_case (`account_id`, `device_code`, `expires_in`) as expected by Tauri v1's IPC layer.

#### ⚠ MINOR

- The `logout` button label changed from "Logout" / "⎋" to "Disconnect" in the account menu. This is intentional per the spec (semantics changed: disconnect session, keep account) but represents a visible behavior change for existing users.

---

## Score Table

| Category | Score | Grade |
|----------|-------|-------|
| Specification Compliance | 92% | A- |
| Best Practices | 78% | C+ |
| Functionality | 85% | B |
| Code Quality | 82% | B- |
| Security | 75% | C |
| Performance | 92% | A- |
| Consistency | 88% | B+ |
| Build Success | 100% | A+ |

**Overall Grade: B (86.5%)**

---

## Issues Summary

| Severity | Count | Description |
|----------|-------|-------------|
| CRITICAL | 1 | Legacy migration deletes token even on migration failure → data loss |
| MODERATE | 1 | Inconsistent keyring error handling in `poll_device_flow` / `authenticate_with_pat` |
| MINOR | 2 | Add-account-submit-btn not re-enabled on modal reopen; code duplication in auth flows |

---

## Required Fixes for Refinement

### Fix 1 (CRITICAL) — `src-tauri/src/github/auth.rs`, `restore_session`

Move `delete_token()` inside the success block so the legacy token is only deleted after successful migration:

```rust
// BEFORE (buggy):
if store_account_token(&id, &legacy_token).is_ok() {
    accounts.push(account);
    let _ = save_account_index(&accounts);
}
// Always remove the legacy token after migration attempt  ← BUG
let _ = delete_token();

// AFTER (correct):
if store_account_token(&id, &legacy_token).is_ok() {
    accounts.push(account);
    let _ = save_account_index(&accounts);
    let _ = delete_token();  // Only delete after successful migration
}
```

### Fix 2 (MODERATE) — `src-tauri/src/github/auth.rs`

In `poll_device_flow` and `authenticate_with_pat`, upgrade keyring write failures from silent `eprintln!` warnings to returned errors, consistent with `add_account`:

```rust
// In the "existing account" update branch of both functions:
store_account_token(&existing_id, &token).map_err(|e| e.to_string())?;

// In the "new account" branch:
store_account_token(&new_id, &token).map_err(|e| e.to_string())?;
save_account_index(&app.accounts).map_err(|e| e.to_string())?;
```

### Fix 3 (MINOR) — `src/main.js`, add-account modal open handler

Re-enable the submit button when the modal is opened:

```js
document.getElementById("add-account-btn").addEventListener("click", () => {
  document.getElementById("account-menu").classList.add("hidden");
  document.getElementById("add-account-modal").classList.remove("hidden");
  document.getElementById("add-account-token").value = "";
  document.getElementById("add-account-label").value = "";
  document.getElementById("add-account-error").classList.add("hidden");
  document.getElementById("add-account-submit-btn").disabled = false;  // ← ADD THIS
});
```

---

## Final Verdict

**NEEDS_REFINEMENT**

The CRITICAL data-loss bug in legacy migration (`delete_token()` called unconditionally regardless of whether migration succeeded) must be fixed before this feature is considered shippable. Users upgrading from the single-account version and encountering any transient failure (network unavailable, keyring write error) during their first launch after upgrade will permanently lose their stored session.

The MODERATE inconsistent error handling in `poll_device_flow` and `authenticate_with_pat` must also be resolved to ensure keyring failures surface as actionable errors rather than silent in-memory state that cannot survive an app restart.

All build checks PASS. The overall architecture, design, and implementation quality are sound. The number and severity of required fixes are small and well-scoped.
