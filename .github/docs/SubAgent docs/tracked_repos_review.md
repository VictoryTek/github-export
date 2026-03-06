# Tracked Repositories — Review & Quality Assurance

**Feature:** Tracked Repositories (GitHub Desktop-style sidebar panel)
**Project:** GitHub Export (Tauri v1 — Rust + HTML/CSS/JS)
**Reviewer:** QA Subagent
**Date:** 2026-03-05
**Spec Reference:** `.github/docs/SubAgent docs/tracked_repos_spec.md`

---

## Build Validation Results

### 1. `cargo build 2>&1`
**Exit code: 0 ✅**
```
   Compiling tauri v1.8.3
   Compiling tauri-macros v1.4.7
   Compiling github-export v0.1.0 (C:\Projects\github-export\src-tauri)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 14.47s
```

### 2. `cargo clippy -- -D warnings 2>&1`
**Exit code: 0 ✅**
```
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.42s
```
No warnings. Zero clippy findings.

### 3. `cargo test 2>&1`
**Exit code: 0 ✅**
```
   Compiling github-export v0.1.0 (C:\Projects\github-export\src-tauri)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 4.63s
     Running unittests src\main.rs (target\debug\deps\github_export-5352030d8f3a8e9e.exe)

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```
No test failures. (No new tests were added, consistent with the project baseline of 0 tests.)

---

## Score Table

| Category | Score | Grade |
|----------|-------|-------|
| Specification Compliance | 90% | B+ |
| Best Practices | 95% | A |
| Functionality | 85% | B |
| Code Quality | 93% | A- |
| Security | 98% | A+ |
| Performance | 92% | A- |
| Consistency | 95% | A |
| Build Success | 100% | A+ |

**Overall Grade: B+ (93.5%)**

---

## Issues Found

### ⚠ WARNING-1 — Missing `repoSearch` event listener (Sidebar filter is broken)

**Severity:** WARNING
**File:** `src/main.js`
**Spec Reference:** Section 5.5 — "Modify `repoSearch` Event Handler"

The spec explicitly requires updating the `#repo-search` input event listener:

> _"The existing `repoSearch.addEventListener("input", ...)` currently filters the `repos` array into a call to `renderRepoList`. Update it to filter `trackedRepos`."_

**Finding:** No `repoSearch.addEventListener("input", ...)` exists anywhere in `src/main.js`. The constant `repoSearch` (pointing to `#repo-search`) is declared at line 15 and is read inside `renderTrackedRepoList()` (line 333), but the event listener that would trigger re-filtering when the user types was **never attached**.

**Impact:** When a user types in the sidebar Filter repos… input, nothing happens. The filter logic in `renderTrackedRepoList()` is correct (it reads `repoSearch.value.toLowerCase()`), but it is never triggered. This is a functional regression — the original code had a working real-time sidebar filter, and this feature removed it without the replacement handler.

**Fix required (single line):**
```javascript
// Add after the existing add-repo-search listener block
repoSearch.addEventListener("input", () => {
  renderTrackedRepoList(trackedRepos);
});
```

---

### SUGGESTION-1 — Dead code: `renderRepoList` and `selectRepo` remain in use only by `loadRepos`

**Severity:** SUGGESTION
**File:** `src/main.js`

`loadRepos()`, `renderRepoList()`, and `selectRepo()` remain in the codebase. Since `showApp()` now calls `loadTrackedRepos()`, these functions are unreachable from normal execution paths. The spec (Section 4.5) documents that `list_repos` is kept intentionally for backward compatibility, so this is an acceptable trade-off. However, the dead JS functions could be annotated with a comment or eventually removed.

**No action required** — this is acknowledged in the spec.

---

### SUGGESTION-2 — No unit tests for `storage.rs`

**Severity:** SUGGESTION
**File:** `src-tauri/src/storage.rs`

`storage.rs` is a new, non-trivial module with load/save/round-trip behavior worth unit-testing. The overall project has 0 tests (pre-existing), so this is consistent with the baseline — but the `load_all` / `save_all` / `load_tracked_repos` / `save_tracked_repos` chain would benefit from in-module tests using `tempfile`.

**No action required for PASS**, given the project's existing test posture.

---

### SUGGESTION-3 — `pickerLoaded` and `allRepos` not reset on logout

**Severity:** SUGGESTION
**File:** `src/main.js`

The logout handler does not reset `pickerLoaded = false` or `allRepos = []`. This is a low-risk gap: since `showApp()` always resets both when called, any subsequent login into a different account will correctly re-fetch repos. A user who logs out and logs back in as the **same** account within the same Tauri session would see stale `allRepos` data in the picker (unlikely in practice, and the data is still valid). Not a blocker.

---

## Detailed Review

### 1. Specification Compliance

All four new Tauri commands are implemented and registered:
- `get_tracked_repos` — ✅ present in `main.rs`, registered in both `#[cfg(not(feature = "dev-mock"))]` and mock `generate_handler![]`
- `add_tracked_repo` — ✅ same
- `remove_tracked_repo` — ✅ same
- `list_all_repos` — ✅ same

`TrackedRepo` model matches spec exactly (3 fields: `full_name`, `owner`, `name`).

`storage.rs` is present, declared as `mod storage;` in `main.rs`, and implements `load_tracked_repos` / `save_tracked_repos` as specified.

`showApp()` calls `loadTrackedRepos()` instead of `loadRepos()`. ✅

Modal HTML (`#add-repo-modal`) is present in `index.html` with correct structure:
`role="dialog"`, `aria-modal="true"`, `aria-labelledby="add-repo-title"`. ✅

**Gap:** `repoSearch.addEventListener("input", ...)` is missing (see WARNING-1).

**Spec compliance score: 90%** — one functional piece missing.

---

### 2. Best Practices

**Rust:**
- No `unwrap()` in command handlers. All error paths use `map_err(|e| e.to_string())` or `?`. ✅
- `create_dir_all` is called inside `tracked_repos_path()` before any write occurs. ✅
- `load_all()` returns a graceful empty default on missing or unparseable file. ✅
- `add_tracked_repo` validates input: `full_name == format!("{owner}/{name}")` + `valid_chars` charset check. ✅
- Mutex lock is always released before calling into `storage::*` (no lock held across I/O). ✅
- `AppState.tracked_repos` is conditionally compiled (`#[cfg(feature = "dev-mock")]`) — correct. ✅

**JavaScript:**
- `async/await` used correctly with `invoke()`. ✅
- `invoke()` calls are inside `try/catch` blocks. ✅
- Event listeners for the add-repo modal are registered at module load time (not on each modal open), preventing duplicate listener accumulation. ✅
- `pickerLoaded` flag correctly prevents re-fetching on each modal open. ✅

---

### 3. Functionality & Correctness

**get_tracked_repos:** Reads `active_account_id` from `AppState`, falls back to `"default"` if none. Returns empty list gracefully if file is absent. ✅

**add_tracked_repo (idempotency):** `repos.iter().any(|r| r.full_name == full_name)` guard returns the current list without modification/error if already tracked. ✅

**remove_tracked_repo:** Uses `retain()`, only writes to disk if the length actually changed (avoiding unnecessary I/O). ✅

**list_all_repos:** Uses `per_page(100)` as specified. ✅

**Picker modal:** Already-tracked repos are rendered with `.add-repo-item-tracked` class, reduced opacity, `✓` checkmark, and `cursor: default` — click listener not attached. ✅

**Sidebar empty state:** When `trackedRepos` is empty and no filter query is active, renders `"No repositories tracked yet."` hint. ✅

**Sidebar filter:** Logic is correct inside `renderTrackedRepoList()` (reads `repoSearch.value`), but **never fired** without the missing event listener. ⚠

**Selected state re-applied after re-render:** `renderTrackedRepoList()` iterates the newly rendered DOM and re-adds `.selected` to the item matching `selectedRepo`. ✅

**Account switch:** `handleSwitchAccount()` resets `pickerLoaded`, `allRepos`, and calls `loadTrackedRepos()` — tracked repos are per-account as specified. ✅

---

### 4. Security

**Input validation (Rust):**
```rust
let expected = format!("{owner}/{name}");
if full_name != expected { return Err(...) }
let valid_chars = |s: &str| s.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_' || c == '.');
if !valid_chars(&owner) || !valid_chars(&name) { return Err(...) }
```
This prevents path traversal via `..` in the repo owner/name fields, and prevents injection of characters that could form malicious JSON keys. ✅

**XSS prevention (JS):**
- `nameSpan.textContent = r.full_name` — safe DOM text insertion. ✅
- `li.innerHTML = \`<span ...>${esc(r.full_name)}</span>...\`` — uses `esc()` helper which creates a temporary `span`, sets `.textContent`, and reads `.innerHTML` — the standard browser-safe escaping idiom. ✅
- Error messages displayed using `esc(String(e))`. ✅
- Markdown rendering goes through DOMPurify (existing pattern, not new to this PR). ✅

**File path:** `tauri::api::path::app_data_dir()` is used rather than constructing paths from user input, so no file system traversal is possible. ✅

---

### 5. Consistency

**CSS:** All new classes follow the existing naming convention (`kebab-case`, dark-theme variables `var(--bg)`, `var(--surface)`, `var(--border)`, `var(--accent)`). New classes:
- `.btn-add-repo` — dashed border, accent color text, hover with blue tint ✅
- `.repo-list-item` / `.repo-list-name` / `.repo-remove-btn` — matches existing `#repo-list li` sizing ✅
- `.add-repo-*` — consistent with `.modal-*` patterns ✅
- `.modal-card-large` extends `.modal-card` ✅

**JS:** New functions follow the existing camelCase naming pattern (`loadTrackedRepos`, `renderTrackedRepoList`, `selectTrackedRepo`, `handleAddTrackedRepo`, `handleRemoveTrackedRepo`, `openAddRepoModal`, `renderPickerList`). ✅

**Rust:** New commands follow the existing pattern of locking `AppState`, extracting minimal data, releasing lock, then performing I/O. ✅

---

### 6. Completeness

| Checklist Item | Status |
|---|---|
| `mod storage;` declared in `main.rs` | ✅ |
| `TrackedRepo` struct in `models/mod.rs` | ✅ |
| `AppState.tracked_repos` (dev-mock only) | ✅ |
| `storage.rs` file created | ✅ |
| `list_all_repos` in `github/issues.rs` | ✅ |
| 4 new commands in non-mock `generate_handler![]` | ✅ |
| 4 corresponding mock commands in `mock/mod.rs` | ✅ |
| Mock commands in `generate_handler![]` | ✅ |
| `#add-repo-btn` in `index.html` | ✅ |
| `#add-repo-modal` in `index.html` | ✅ |
| `showApp()` calls `loadTrackedRepos()` | ✅ |
| `repoSearch` event listener wired up | ❌ MISSING |
| New CSS classes present | ✅ |

---

## Final Verdict

**NEEDS_REFINEMENT**

The implementation is high quality in all structural and functional respects. Build, lint, and tests all pass cleanly. However, the spec's Section 5.5 requirement — updating (or replacing) the `repoSearch` event listener to call `renderTrackedRepoList(trackedRepos)` when the user types — was omitted. As a result, the sidebar filter input is non-functional: typing in the `Filter repos…` box has no effect.

This is a single-line fix:
```javascript
repoSearch.addEventListener("input", () => {
  renderTrackedRepoList(trackedRepos);
});
```
placed in `src/main.js` alongside the other top-level event listener registrations.

Once this fix is applied, the feature should be re-reviewed for PASS.

---

## Issues Summary

| # | Severity | File | Description |
|---|----------|------|-------------|
| WARNING-1 | ⚠ WARNING | `src/main.js` | Missing `repoSearch.addEventListener("input", ...)` — sidebar filter does nothing |
| SUGGESTION-1 | 💡 SUGGESTION | `src/main.js` | Dead code: `loadRepos`, `renderRepoList`, `selectRepo` unreachable in normal flow |
| SUGGESTION-2 | 💡 SUGGESTION | `src-tauri/src/storage.rs` | No unit tests for storage round-trip |
| SUGGESTION-3 | 💡 SUGGESTION | `src/main.js` | `pickerLoaded`/`allRepos` not reset on logout path |
