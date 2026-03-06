# Actions Tab — Review & Quality Assurance

**Reviewer**: QA Subagent  
**Date**: 2026-03-05  
**Spec**: `.github/docs/SubAgent docs/actions_tab_spec.md`  
**Verdict**: **NEEDS_REFINEMENT**

---

## Build Validation

All three build commands were executed from `c:\Projects\github-export\src-tauri\`.

| Command | Result | Output |
|---|---|---|
| `cargo build` | ✅ PASS | `Finished 'dev' profile [unoptimized + debuginfo] target(s) in 0.38s` |
| `cargo clippy -- -D warnings` | ✅ PASS | `Finished 'dev' profile [unoptimized + debuginfo] target(s) in 0.99s` |
| `cargo test` | ✅ PASS | `test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured` |

**Build Result: PASS** — No compiler errors, no clippy warnings, no test failures.

---

## Findings

### CRITICAL Issues

#### C1 — Security: `html_url` placed in `href` without scheme validation

**File**: `src/main.js`, `renderWorkflowRuns()` function  
**Code**:
```js
const linkHtml = `<a href="${esc(r.html_url)}" target="_blank" rel="noopener noreferrer">View</a>`;
```
`esc()` performs HTML entity encoding (`&`, `<`, `>`, `"`, `'` → entities). This prevents HTML attribute breakout. However, it does **not** validate the URL scheme. A value of `javascript:alert(1)` contains no HTML special characters, so it passes through `esc()` unchanged and produces a working JavaScript URL in the `href`.

In the current application the value comes from GitHub's API via octocrab, so it will always be an `https://github.com/...` URL in normal operation. However, the spec review checklist explicitly asks: *"Are HTML URLs validated before being placed in href (or at minimum sanitized?"* — the answer is no. Defense-in-depth requires scheme validation.

**Required fix**: Add a URL scheme guard before inserting into the template:
```js
const safeUrl = /^https?:\/\//i.test(r.html_url) ? r.html_url : '#';
const linkHtml = `<a href="${esc(safeUrl)}" target="_blank" rel="noopener noreferrer">View</a>`;
```

---

### RECOMMENDED Issues

#### R1 — Mock handler missing `get_workflow_runs` and `export_actions_csv`

**File**: `src-tauri/src/main.rs`  
**Spec reference**: Section 5, Step 7d  

The spec requires mock stubs for both new commands registered in the `#[cfg(feature = "dev-mock")]` invoke handler. Neither is present:

```rust
// dev-mock handler — current state (incomplete)
#[cfg(feature = "dev-mock")]
let builder = builder.invoke_handler(tauri::generate_handler![
    mock::get_dev_mode,
    mock::restore_session,
    // ... other mocks ...
    // ❌ get_workflow_runs NOT registered
    // ❌ export_actions_csv NOT registered
]);
```

In `dev-mock` mode, any JS call to `get_workflow_runs` or `export_actions_csv` will fail with a Tauri "command not registered" error. Since `refreshData()` includes `get_workflow_runs` in its `Promise.allSettled`, this failure is caught silently, but the Actions tab data will be permanently empty in mock mode. The dev-mock build is used for development and UI testing; this gap makes the Actions feature untestable without a real GitHub connection.

**Required fix**: Add standalone `#[cfg(feature = "dev-mock")]` stub functions and register them:
```rust
#[cfg(feature = "dev-mock")]
#[tauri::command]
async fn get_workflow_runs(
    _owner: String,
    _repo: String,
    _state: State<'_, Mutex<AppState>>,
) -> Result<Vec<models::WorkflowRun>, String> {
    Ok(vec![])
}

#[cfg(feature = "dev-mock")]
#[tauri::command]
async fn export_actions_csv(
    _runs: Vec<models::WorkflowRun>,
    _file_path: String,
) -> Result<String, String> {
    Ok("Mock export complete".to_string())
}
```
Then add `get_workflow_runs, export_actions_csv,` to the mock `generate_handler![]`.

#### R2 — `refreshData()` does not re-render the Actions table

**File**: `src/main.js`, `refreshData()` function  
**Lines ~582–613**  

When `refreshData()` completes, it updates `workflowRuns` in memory, updates the status dot, and enables the export button. However, it does **not** call `renderWorkflowRuns(workflowRuns)`. If the user is on the Actions tab when Refresh is triggered (e.g., via the toolbar ↺ button or `stateFilter` change), the status dot will update correctly but the table will continue displaying stale data until the user clicks away and back.

**Required fix**: Add a render call inside the `runsRes.status === "fulfilled"` branch in `refreshData()`:
```js
if (runsRes.status === "fulfilled") {
    workflowRuns = runsRes.value;
    actionsLoaded = true;
    renderWorkflowRuns(workflowRuns);   // ← add this
    updateActionStatusDot(workflowRuns);
    document.getElementById("export-actions-btn").disabled = workflowRuns.length === 0;
} else {
    workflowRuns = [];
    actionsLoaded = false;
    renderWorkflowRuns([]);              // ← add this
    updateActionStatusDot([]);
    console.error("get_workflow_runs failed:", runsRes.reason);
}
```

#### R3 — Missing `prefers-reduced-motion` override for `pulse-dot` animation

**File**: `src/styles.css`  
**Spec reference**: Section 10, Accessibility Considerations  

The spec explicitly documents this as a recommended implementation concern. The `pulse-dot` animation fires continuously for in-progress runs. Users with vestibular disorders or motion sensitivity depend on `prefers-reduced-motion` to suppress animations.

**Required fix**:
```css
@media (prefers-reduced-motion: reduce) {
  .tab-status-dot--pending { animation: none; }
}
```

---

### MINOR Issues

#### M1 — Dead code: unused `linkEl` DOM element in `renderWorkflowRuns`

**File**: `src/main.js`, `renderWorkflowRuns()` function

```js
// These four lines create a DOM element that is immediately discarded:
const linkEl = document.createElement('a');
linkEl.href = r.html_url;
linkEl.target = '_blank';
linkEl.rel = 'noopener noreferrer';
linkEl.textContent = 'View';
// The actual output uses a separate template literal instead:
const linkHtml = `<a href="${esc(r.html_url)}" target="_blank" rel="noopener noreferrer">View</a>`;
```

The `linkEl` variable is dead code — it is created, has properties assigned, and is then never read or inserted into the DOM. The actual link HTML comes from the `linkHtml` template literal. The dead code adds noise and may confuse future maintainers.

**Fix**: Remove the five dead `linkEl.*` lines.

#### M2 — Export button stays enabled after `refreshData()` run failure

**File**: `src/main.js`, `refreshData()` function  

When `runsRes` fails, `workflowRuns` is reset to `[]` but `document.getElementById("export-actions-btn").disabled` is not set to `true`. If the user had previously loaded runs successfully, the button remains enabled after a failed refresh, and clicking Export would silently produce an empty CSV (0 rows). This is consistent with the existing export functions' behavior but is slightly confusing UX.

**Fix**: Set `document.getElementById("export-actions-btn").disabled = true;` in the failure branch (already addressed by the R2 fix above if adopted as written).

#### M3 — Table columns diverge from detailed spec (minor scope issue)

**File**: `src/index.html`, `#actions-table`  

The spec Step 2b specifies 8 columns: `#, Workflow, Branch, Event, Status, Actor, Started, Link`. The implementation uses 6 columns: `Workflow, Branch, Status, Actor, Started, Link` (dropping `#` run number and `Event`). The review checklist explicitly lists the 6-column version, so this **passes the formal checklist**. The implementation matches the review checklist. No change required; noted for completeness.

---

## Specification Compliance Checklist

| Check | Result |
|---|---|
| `#actions-tab` placed before `#issues-tab` | ✅ PASS |
| Tab button uses `.tab-status-dot` (not `.tab-badge`) | ✅ PASS |
| `#tab-actions` panel exists | ✅ PASS |
| Table columns match checklist (`Workflow, Branch, Status, Actor, Started, Link`) | ✅ PASS |
| Export CSV button (`#export-actions-btn`) present | ✅ PASS |
| `get_workflow_runs` in non-mock `invoke_handler` | ✅ PASS |
| `export_actions_csv` in non-mock `invoke_handler` | ✅ PASS |
| `pub mod actions;` in `github/mod.rs` | ✅ PASS |
| `get_workflow_runs` in mock `invoke_handler` | ❌ FAIL |
| `export_actions_csv` in mock `invoke_handler` | ❌ FAIL |
| Status dot: `in_progress\|queued` → pending/yellow | ✅ PASS |
| Status dot: `conclusion=success` → green | ✅ PASS |
| Status dot: `conclusion=failure\|timed_out\|action_required` → red | ✅ PASS |
| Status dot: other conclusions → neutral/grey | ✅ PASS |
| Status dot hidden when runs array is empty | ✅ PASS |
| `renderWorkflowRuns`: user data via `textContent`/`esc()`, not raw `innerHTML` | ✅ PASS |
| External links have `rel="noopener noreferrer"` | ✅ PASS |
| `html_url` validated before use in `href` | ❌ FAIL |
| `actions.rs` follows `security.rs` pattern | ✅ PASS |
| Errors wrapped with `map_err` / `with_context` | ✅ PASS |
| No `unwrap()` in production paths | ✅ PASS |
| `.tab-status-dot` has `position: absolute` | ✅ PASS |
| `@keyframes pulse-dot` defined | ✅ PASS |
| `.tab` has `position: relative` | ✅ PASS |
| `updateActionStatusDot` updates `aria-label` | ✅ PASS |
| `loadActions()` called when Actions tab clicked | ✅ PASS |
| `updateActionStatusDot` called in `refreshData()` | ✅ PASS |
| Actions state cleared on repo change / logout | ✅ PASS |

---

## Score Table

| Category | Score | Grade |
|---|---|---|
| Specification Compliance | 82% | B |
| Best Practices | 78% | C+ |
| Functionality | 83% | B |
| Code Quality | 84% | B |
| Security | 72% | C |
| Performance | 96% | A |
| Consistency | 91% | A- |
| Build Success | 100% | A+ |

**Overall Grade: B- (85%)**

---

## Summary

The Actions tab implementation is substantial and correct in most respects. All three build commands pass cleanly with zero warnings. The Rust backend (`actions.rs`, `models/mod.rs`, `csv_export.rs`, `main.rs`) is well-structured, follows established codebase patterns, and has no clippy warnings. The frontend status-dot logic, CSS, and accessibility attributes are correctly implemented.

Three issues require remediation before this can be considered production-ready:

1. **C1 (CRITICAL) — URL scheme not validated**: `html_url` is HTML-encoded but not scheme-guarded before insertion into `href`. A `javascript:` URL would not be blocked. Fix requires a one-line scheme check (`/^https?:\/\//i.test(...)`) before the template literal.

2. **R1 (RECOMMENDED) — Mock stubs missing**: `get_workflow_runs` and `export_actions_csv` are absent from the `dev-mock` invoke handler, making the Actions feature non-functional in development mode. The spec explicitly requires these stubs.

3. **R2 (RECOMMENDED) — Actions table not re-rendered on refresh**: `refreshData()` updates `workflowRuns` and the status dot but does not call `renderWorkflowRuns()`, leaving the visible table stale when the user is on the Actions tab. Fix requires one additional call in `refreshData()`.

The remaining issues (M1–M3) are minor and do not affect correctness or security.

---

## Final Verdict: **NEEDS_REFINEMENT**

The CRITICAL security finding (C1) and two RECOMMENDED gaps (R1, R2) must be addressed. Once these three issues are resolved and a re-build confirms no regressions, this implementation should achieve PASS status.
