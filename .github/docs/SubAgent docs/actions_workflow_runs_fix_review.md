# Actions Workflow Runs Fix — Review & Quality Assurance

**Date:** 2026-03-06  
**Reviewer:** QA Subagent  
**Spec:** `actions_workflow_runs_fix_spec.md`  
**Verdict:** ✅ PASS

---

## 1. Specification Compliance

All 5 fixes specified in `actions_workflow_runs_fix_spec.md` are correctly implemented.

### Fix 1 — Stale state reset in `selectTrackedRepo()` ✅

**File:** `src/main.js`

The two reset lines are present and correctly positioned — **after** `selectedRepo` is set and **before** `refreshData()` is called:

```javascript
function selectTrackedRepo(repo) {
  selectedRepo = { owner: repo.owner, name: repo.name };
  // ... UI selection highlighting ...
  actionsLoaded = false;   // ✅ present
  workflowRuns = [];       // ✅ present
  refreshData();
}
```

This directly eliminates the primary root cause (stale `actionsLoaded = true` with empty `workflowRuns` from a prior repo causing "No workflow runs found." when clicking the Actions tab during or after a repo switch).

Additional observation: `clearTabBadges()` (called from `handleSwitchAccount()`) also resets `workflowRuns = []` and `actionsLoaded = false`, so account switching was already protected. The fix correctly targets the missing case: tracked repo switching.

### Fix 2 — Realistic mock workflow run data ✅

**File:** `src-tauri/src/mock/mod.rs`

`get_workflow_runs` now returns exactly 4 `WorkflowRun` entries as specified, covering all meaningful status/conclusion combinations:

| Run ID | Workflow | Branch | Event | Status | Conclusion |
|--------|----------|--------|-------|--------|------------|
| 12345678 | CI | main | push | completed | success |
| 12345677 | CI | feat/oauth-device-flow | pull_request | completed | failure |
| 12345676 | CodeQL | main | schedule | in_progress | *(None)* |
| 12345675 | Release | main | push | completed | cancelled |

The mock entries use realistic field values (octocat/Hello-World URLs, plausible run numbers, plausible actor logins including `github-actions[bot]`). All 4 entries match the spec exactly.

### Fix 3 — `event: Option<String>` in `RawWorkflowRun` ✅

**File:** `src-tauri/src/github/actions.rs`

Both required changes are present:

```rust
// Struct field — optional:
event: Option<String>,

// Mapping — absorbs null at the API boundary:
event: r.event.unwrap_or_default(),
```

This is consistent with the existing pattern for `status` (already optional) and `name` (already optional). The domain model `WorkflowRun.event: String` in `models/mod.rs` remains unchanged, correctly keeping optionality as an API-boundary concern.

### Fix 4 — Export button logic in `loadActions()` ✅

**File:** `src/main.js`

```javascript
document.getElementById('export-actions-btn').disabled = workflowRuns.length === 0;
```

This now mirrors the logic in `refreshData()`, which already used `workflowRuns.length === 0`. The inconsistency has been eliminated.

### Fix 5 — Unconditional `renderWorkflowRuns` in `refreshData()` ✅

**File:** `src/main.js`

The `if (activeTab === "actions")` guard has been removed:

```javascript
if (runsRes.status === "fulfilled") {
    workflowRuns = runsRes.value;
    actionsLoaded = true;
    renderWorkflowRuns(workflowRuns);    // ← now unconditional ✅
    updateActionStatusDot(workflowRuns);
    document.getElementById("export-actions-btn").disabled = workflowRuns.length === 0;
}
```

The Actions table is always kept in sync after a data refresh, regardless of which tab is active. This addresses the prior R2 recommendation from `actions_tab_review.md`. DOM operations on hidden elements are safe and incur negligible cost.

---

## 2. Best Practices

### Rust Code

- **No unnecessary clones:** `r.event.unwrap_or_default()` consumes the `Option<String>` value directly. No intermediate allocation. ✅
- **Consistent error propagation:** `with_context(|| ...)` from `anyhow` is used correctly. ✅
- **Pattern consistency:** `event: Option<String>` with `unwrap_or_default()` is the same pattern used for `status`, `name`, `head_branch`, and `actor`. ✅
- **Mock data:** No `.clone()` or unnecessary allocations beyond what the data naturally requires. All strings are owned. ✅

### JavaScript Code

- **Consistent style:** The two added lines (`actionsLoaded = false; workflowRuns = [];`) follow the existing assignment style used elsewhere in the file (e.g., `handleSwitchAccount` which clears `repos = []; issues = []; pulls = []; alerts = [];`). ✅
- **No new globals or side effects** introduced. ✅
- **`loadActions()` export button fix** is a one-line change, minimal and targeted. ✅

### Minor observation (pre-existing, out of scope)

`selectRepo()` (the legacy function, not the tracked-repo path) does not reset `actionsLoaded` and `workflowRuns`. This was not part of the spec and is a pre-existing gap. The primary user path uses `selectTrackedRepo()`.

The rejection branch in `refreshData()` only shows the error UI when `activeTab === "actions"` at completion time. This means that if the user is on another tab when the error occurs, they will see no error indicator when switching to Actions (just the stale state). This is also a pre-existing gap not addressed by this spec.

---

## 3. Functionality

### Would the fixes actually resolve "No workflow runs found."?

**Yes — all primary failure paths are closed:**

1. **Mock mode (was permanently broken):** Now returns 4 runs on every call. The Actions tab will render correctly in dev-mock builds. ✅
2. **Repo switch with stale `actionsLoaded`:** The `actionsLoaded = false; workflowRuns = [];` reset in `selectTrackedRepo()` forces a fresh fetch via `loadActions()` on the next tab click. ✅
3. **Race condition (user on Actions tab during repo switch):** After the reset, `actionsLoaded` is `false`. If the user clicks the Actions tab before `refreshData()` completes, `loadActions()` will be invoked (fresh fetch). If they click after `refreshData()` completes, `actionsLoaded` is `true` and `workflowRuns` is correct. ✅
4. **Null `event` field from GitHub API:** Absorbed at the serde layer as empty string, preventing full-page deserialization failure. ✅

### Edge cases still present (not introduced by this fix)

- If `refreshData()` completes while the user is on the Actions tab, and then returns `runsRes.status === "rejected"`, only the error UI is shown when `activeTab === "actions"`. If the user was on another tab, the error div remains hidden when they switch to Actions. This is pre-existing.
- The first-time `loadActions()` deep path (actions tab click before any `refreshData()` completes) still has the `actionsLoaded` flag guarding correctly. ✅

---

## 4. Code Quality & Security

- **No new `unsafe` code** introduced. ✅
- **No new `unwrap()` calls** that could panic at runtime. `unwrap_or_default()` is safe by construction. ✅
- **No injection risks:** The JS changes only modify boolean flags and `.disabled` property. No new HTML injection paths opened. All user-facing content in `renderWorkflowRuns` was already using `esc()` for sanitization. ✅
- **No new external API calls or networking changes.** ✅

---

## 5. Consistency — Field Name Alignment (Rust ↔ JS)

`WorkflowRun` in `models/mod.rs` has **no `#[serde(rename_all)]` attribute**, so all fields serialize to their snake_case names in JSON.

| Rust field (snake_case) | JS access (snake_case) | Match |
|-------------------------|------------------------|-------|
| `name` | `r.name` | ✅ |
| `head_branch` | `r.head_branch` | ✅ |
| `status` | `r.status` | ✅ |
| `conclusion` | `r.conclusion` | ✅ |
| `actor_login` | `r.actor_login` | ✅ |
| `run_started_at` | `r.run_started_at` | ✅ |
| `created_at` | `r.created_at` | ✅ |
| `html_url` | `r.html_url` | ✅ |

The JS frontend correctly uses snake_case throughout `renderWorkflowRuns` and `updateActionStatusDot`. No mismatches found.

---

## 6. Build Validation

### `cargo build`

```
Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.30s
EXIT_CODE: 0
```

✅ **PASS** — Clean build, no errors, no new warnings.

### `cargo clippy -- -D warnings`

```
Compiling github-export v0.1.0 (C:\Projects\github-export\src-tauri)
Finished `dev` profile [unoptimized + debuginfo] target(s) in 17.67s
EXIT_CODE: 0
```

✅ **PASS** — Zero warnings. All lint checks passed under strict `-D warnings` mode.

### `cargo test`

```
Compiling github-export v0.1.0 (C:\Projects\github-export\src-tauri)
Finished `test` profile [unoptimized + debuginfo] target(s) in 7.66s
Running unittests src\main.rs (target\debug\deps\github_export-1f2ea18b7d5f672d.exe)

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
EXIT_CODE: 0
```

✅ **PASS** — Test suite passes. No tests exist for the Rust backend yet; this is a pre-existing gap and not introduced by this change.

---

## 7. Score Table

| Category | Score | Grade |
|----------|-------|-------|
| Specification Compliance | 100% | A |
| Best Practices | 95% | A |
| Functionality | 98% | A |
| Code Quality | 100% | A |
| Security | 100% | A |
| Performance | 100% | A |
| Consistency | 100% | A |
| Build Success | 100% | A |

**Overall Grade: A (99%)**

Minor deduction in Best Practices (95%) reflects two pre-existing gaps that were observed during review but are explicitly out of scope for this fix: `selectRepo()` not resetting actions state, and the error branch in `refreshData()` being conditional on `activeTab`.

---

## Summary

All 5 specified fixes are correctly implemented with no regressions, no new lint warnings, no new panics, and no security issues. The implementation is idiomatic, minimal, and consistent with the existing codebase conventions.

**Final Verdict: ✅ PASS**
