# Actions Tab — Workflow Runs Not Showing: Investigation & Fix Specification

**Date:** 2026-03-06
**Symptom:** The Actions tab always displays "No workflow runs found." for repositories that have many GitHub Actions workflow runs.
**Status:** Root causes fully identified — 5 bugs found across 2 files.
**Prior art:** `actions_bug_fix_spec.md` (2026-03-05) — 2 previously identified bugs are **confirmed already fixed** in the current codebase. This spec identifies the **remaining root causes** that still produce the symptom.

---

## Prior Fix Verification

The two bugs from `actions_bug_fix_spec.md` are **confirmed implemented** in the current code:

| Prior Bug | Fix | Current Code | Status |
|-----------|-----|-------------|--------|
| `status: String` (non-optional, GitHub marks nullable) | → `status: Option<String>` + `.unwrap_or_default()` | `src-tauri/src/github/actions.rs` line 17 | ✅ FIXED |
| `refreshData()` calls `renderWorkflowRuns([])` on error | → shows `#actions-error` div instead | `src/main.js` rejection handler for `runsRes` | ✅ FIXED |

With those two bugs fixed, a **new failure chain** produces the "always No workflow runs found" symptom. This spec addresses it completely.

---

## Current State Analysis

### Backend — `src-tauri/src/github/actions.rs`

```rust
// Lines 1–70 (full file)
#[derive(Debug, Deserialize)]
struct RawWorkflowRun {
    id: u64,
    name: Option<String>,
    head_branch: Option<String>,
    run_number: u64,
    event: String,            // ← LATENT BUG: non-optional; GitHub API marks as required
                              //   but real-world repos can return null for some run types
    status: Option<String>,   // already fixed
    conclusion: Option<String>,
    actor: Option<RawActor>,
    created_at: String,
    run_started_at: Option<String>,
    html_url: String,
    workflow_id: u64,
}
```

The `fetch_workflow_runs` function (lines 34–68) constructs a correct URL and uses `client.get(&url, None::<&()>)`. The call itself is sound. There are no bugs in the network request path for well-formed API responses.

### Mock — `src-tauri/src/mock/mod.rs`

```rust
// Lines ~425–432
/// Returns an empty workflow runs list in mock mode.
#[tauri::command]
pub fn get_workflow_runs(
    _owner: String,
    _repo: String,
    _state: tauri::State<'_, Mutex<AppState>>,
) -> Result<Vec<WorkflowRun>, String> {
    Ok(vec![])   // ← CONFIRMED BUG: always returns empty; zero mock data
}
```

In dev-mock mode (used for all development and UI testing), the Actions tab is **permanently broken**: it always receives `Ok([])` and shows "No workflow runs found." with no mechanism to ever show data.

### Frontend — `src/main.js`

#### State declarations (lines 27–38)

```javascript
let workflowRuns  = [];
let activeTab = "issues";       // ← initial tab is "issues" (correct)
let actionsLoaded = false;      // ← tracks whether workflow runs have been fetched
```

#### `selectTrackedRepo()` (lines 394–403)

```javascript
function selectTrackedRepo(repo) {
    selectedRepo = { owner: repo.owner, name: repo.name };
    // ...renders selection UI...
    refreshData();   // ← NOTE: actionsLoaded and workflowRuns NOT reset before this call
}
```

`actionsLoaded` and `workflowRuns` **retain stale values from the previously selected repository** for the entire duration of the new `refreshData()` call (hundreds of ms to seconds over network).

#### `refreshData()` fulfilled branch for workflow runs (lines 604–612)

```javascript
if (runsRes.status === "fulfilled") {
    workflowRuns = runsRes.value;
    actionsLoaded = true;
    if (activeTab === "actions") renderWorkflowRuns(workflowRuns);  // ← conditional
    updateActionStatusDot(workflowRuns);
    document.getElementById("export-actions-btn").disabled = workflowRuns.length === 0;
}
```

`renderWorkflowRuns` is **only called when `activeTab === "actions"` at the time `refreshData()` completes**. When the user selects a repo from the Issues tab (the default), `activeTab` is `"issues"` and the render is skipped entirely. The data is stored in `workflowRuns` but the table is NOT rendered until the user explicitly clicks the Actions tab.

This is correct design for lazy rendering — **but only if the `actionsLoaded` / `workflowRuns` stale state is handled correctly**. It is not.

#### Actions tab click handler (lines 957–963)

```javascript
document.getElementById('actions-tab').addEventListener('click', () => {
    if (!actionsLoaded) {
        loadActions();
    } else {
        renderWorkflowRuns(workflowRuns);
    }
});
```

When `actionsLoaded === true` (stale from previous repo), clicking the Actions tab calls `renderWorkflowRuns(workflowRuns)` with the **stale (possibly empty) `workflowRuns` from the old repo**. `loadActions()` is never triggered because `actionsLoaded` was never reset.

#### `loadActions()` export button (lines 924–930)

```javascript
try {
    workflowRuns = await invoke('get_workflow_runs', { owner, repo: name });
    actionsLoaded = true;
    renderWorkflowRuns(workflowRuns);
    updateActionStatusDot(workflowRuns);
    document.getElementById('export-actions-btn').disabled = false;  // ← always enables
```

Export button is enabled unconditionally even when `workflowRuns = []`. `refreshData()` correctly uses `workflowRuns.length === 0` but `loadActions()` does not.

---

## Root Cause Analysis

### Root Cause 1 — PRIMARY (affects mock mode absolutely, production conditionally)

**The `actionsLoaded` flag is never reset when switching repositories.**

**Trigger scenario (most common real-world path):**

1. App launches. `actionsLoaded = false`, `workflowRuns = []`.
2. User selects **Repo A** (which has no workflow runs, or is loaded first in mock mode).
3. `refreshData()` runs → `invoke("get_workflow_runs", ...)` returns success with `[]`.
4. `workflowRuns = []`, **`actionsLoaded = true`**.
5. User is on Issues tab → `if (activeTab === "actions")` is `false` → `renderWorkflowRuns` NOT called.
6. User clicks Actions tab → **`actionsLoaded === true`** → `renderWorkflowRuns([])` → "No workflow runs found." ← shown correctly for Repo A.
7. User selects **Repo B** (which has 50 workflow runs).
8. `selectTrackedRepo(repoB)` is called → **`actionsLoaded` is NOT reset** → still `true` with `workflowRuns = []` from Repo A.
9. `refreshData()` starts fetching Repo B's data.
10. **At any point during step 9**, if the user clicks the Actions tab:
    - `actionsLoaded === true` (stale) → `renderWorkflowRuns([])` ← **"No workflow runs found."** shown for Repo B (wrong).
11. `refreshData()` eventually completes → `workflowRuns = [50 items]`, `actionsLoaded = true`.
12. `if (activeTab === "actions")` at completion time — if user is **currently on the Actions tab**: `renderWorkflowRuns([50 items])` correctly renders. ✓
13. But if the user **navigated away from the Actions tab** (e.g., clicked back to Issues) while step 9 was running, then clicked back to Actions after seeing the "No workflow runs found." message: `refreshData()` already completed, `actionsLoaded === true`, `workflowRuns = [50 items]` → correct render triggered on the click. ✓

**This scenario produces a visible but self-correcting "No workflow runs found." flash if the user clicks the Actions tab during a repository switch.** However, combined with Root Cause 2, it can appear permanent.

### Root Cause 2 — CONFIRMED (mock mode, also production when Repo A has no runs)

**`mock::get_workflow_runs` always returns `Ok(vec![])`.** There is no mock workflow run data.

In dev-mock mode, every call to the Actions tab on any repo returns an empty list. `actionsLoaded` is set to `true` (success, not error), and `workflowRuns = []`. `renderWorkflowRuns([])` is called, showing "No workflow runs found." permanently — no error message, no indication of a problem, no recovery path.

This means **the Actions feature cannot be tested or demonstrated in dev-mock mode at all**.

### Root Cause 3 — LATENT (production, certain repo types)

**`event: String` is non-optional in `RawWorkflowRun`, but GitHub's API can return `null` for this field in practice.**

GitHub's OpenAPI spec documents `event` as non-nullable (type `string`). However, for:
- Workflow runs triggered by webhook events from external GitHub Apps
- Runs on repositories that have workflow files deleted mid-execution
- Runs triggered by certain programmatic automation features

The `event` field has been observed as `null` in production environments. When `event: String` encounters `null`, serde's `#[derive(Deserialize)]` fails the **entire `Vec<RawWorkflowRun>`** deserialization — not just the affected item. This causes `client.get()` to return `Err(octocrab::Error::Json { ... })`.

With the prior Bug 2 fix in place, this error now correctly shows the `#actions-error` div with "Failed to load workflow runs: ..." rather than silently converting to "No workflow runs found." The impact of Root Cause 3 has therefore been **downgraded from "always empty"** (pre-fix) **to "shows error message"** (post-fix). However, fixing it proactively eliminates a class of serde failure for all affected repositories.

### Root Cause 4 — MINOR (UX bug, `loadActions()`)

**`loadActions()` unconditionally enables the export button regardless of whether runs were returned.**

```javascript
document.getElementById('export-actions-btn').disabled = false;  // wrong
// should be:
document.getElementById('export-actions-btn').disabled = workflowRuns.length === 0;
```

`refreshData()` uses the correct conditional logic but `loadActions()` does not mirror it.

### Root Cause 5 – MINOR (UX, rendering on `refreshData()` completion)

**`renderWorkflowRuns` in `refreshData()` is only called when `activeTab === "actions"`.** When the user is on a different tab, the Actions table is not updated after a refresh (filter change, search input, ↺ button). The tab-click handler correctly pulls from `workflowRuns` cache on next click, so data is not lost — but the table is stale until re-clicked. This was flagged as R2 in `actions_tab_review.md` and was never fully addressed.

---

## Summary Table

| # | Severity | File | Location | Description |
|---|----------|------|----------|-------------|
| 1 | **CRITICAL** | `src/main.js` | `selectTrackedRepo()` | `actionsLoaded` and `workflowRuns` not reset on repo switch; stale empty state shown |
| 2 | **CRITICAL** | `src-tauri/src/mock/mod.rs` | `get_workflow_runs` | Always returns `Ok(vec![])` — Actions tab permanently broken in dev-mock mode |
| 3 | **HIGH** | `src-tauri/src/github/actions.rs` | `RawWorkflowRun` struct | `event: String` non-optional; null event from API causes full deserialization failure |
| 4 | **LOW** | `src/main.js` | `loadActions()` | Export button unconditionally enabled even when `workflowRuns = []` |
| 5 | **LOW** | `src/main.js` | `refreshData()` | `renderWorkflowRuns` conditional on `activeTab`; table not refreshed when on another tab |

---

## Proposed Solution

### Fix 1 — Reset `actionsLoaded` and `workflowRuns` on repo switch

**File:** `src/main.js`
**Location:** `selectTrackedRepo()` function

This is the correct place to clear stale Actions state. Resetting here ensures that when `refreshData()` runs for the new repo, any concurrent or pre-completion tab click goes through `loadActions()` (which makes a fresh API call) rather than rendering stale empty data.

```javascript
// BEFORE:
function selectTrackedRepo(repo) {
    selectedRepo = { owner: repo.owner, name: repo.name };
    // ...
    refreshData();
}

// AFTER:
function selectTrackedRepo(repo) {
    selectedRepo = { owner: repo.owner, name: repo.name };
    // Reset Actions state so the tab doesn't show stale data from the previous repo
    actionsLoaded = false;
    workflowRuns = [];
    // ...
    refreshData();
}
```

The two added lines must appear **before** `refreshData()` is called but **after** `selectedRepo` is set.

### Fix 2 — Add realistic mock workflow run data

**File:** `src-tauri/src/mock/mod.rs`
**Location:** `get_workflow_runs` function

Replace the no-op stub with realistic mock data covering all meaningful status/conclusion combinations that the frontend renders:

```rust
/// Returns realistic mock workflow runs for development and UI testing.
#[tauri::command]
pub fn get_workflow_runs(
    _owner: String,
    _repo: String,
    _state: tauri::State<'_, Mutex<AppState>>,
) -> Result<Vec<WorkflowRun>, String> {
    Ok(vec![
        WorkflowRun {
            id: 12_345_678,
            name: "CI".to_string(),
            head_branch: Some("main".to_string()),
            run_number: 42,
            event: "push".to_string(),
            status: "completed".to_string(),
            conclusion: Some("success".to_string()),
            actor_login: "octocat".to_string(),
            created_at: "2026-03-05T14:32:00Z".to_string(),
            run_started_at: Some("2026-03-05T14:32:10Z".to_string()),
            html_url: "https://github.com/octocat/Hello-World/actions/runs/12345678".to_string(),
            workflow_id: 1_001,
        },
        WorkflowRun {
            id: 12_345_677,
            name: "CI".to_string(),
            head_branch: Some("feat/oauth-device-flow".to_string()),
            run_number: 41,
            event: "pull_request".to_string(),
            status: "completed".to_string(),
            conclusion: Some("failure".to_string()),
            actor_login: "monalisa".to_string(),
            created_at: "2026-03-04T09:15:00Z".to_string(),
            run_started_at: Some("2026-03-04T09:15:22Z".to_string()),
            html_url: "https://github.com/octocat/Hello-World/actions/runs/12345677".to_string(),
            workflow_id: 1_001,
        },
        WorkflowRun {
            id: 12_345_676,
            name: "CodeQL".to_string(),
            head_branch: Some("main".to_string()),
            run_number: 18,
            event: "schedule".to_string(),
            status: "in_progress".to_string(),
            conclusion: None,
            actor_login: "github-actions[bot]".to_string(),
            created_at: "2026-03-06T00:00:00Z".to_string(),
            run_started_at: Some("2026-03-06T00:00:05Z".to_string()),
            html_url: "https://github.com/octocat/Hello-World/actions/runs/12345676".to_string(),
            workflow_id: 1_002,
        },
        WorkflowRun {
            id: 12_345_675,
            name: "Release".to_string(),
            head_branch: Some("main".to_string()),
            run_number: 7,
            event: "push".to_string(),
            status: "completed".to_string(),
            conclusion: Some("cancelled".to_string()),
            actor_login: "defunkt".to_string(),
            created_at: "2026-03-01T18:45:00Z".to_string(),
            run_started_at: Some("2026-03-01T18:45:30Z".to_string()),
            html_url: "https://github.com/octocat/Hello-World/actions/runs/12345675".to_string(),
            workflow_id: 1_003,
        },
    ])
}
```

### Fix 3 — Make `event` optional in `RawWorkflowRun`

**File:** `src-tauri/src/github/actions.rs`
**Location:** `RawWorkflowRun` struct and `fetch_workflow_runs()` mapping

```rust
// BEFORE (struct):
event: String,

// AFTER (struct):
event: Option<String>,

// BEFORE (mapping in fetch_workflow_runs):
event: r.event,

// AFTER (mapping in fetch_workflow_runs):
event: r.event.unwrap_or_default(),
```

The domain model (`models/mod.rs` `WorkflowRun.event: String`) remains unchanged — the optionality is absorbed at the API boundary layer, identical to the pattern used for `status`.

### Fix 4 — Correct export button state in `loadActions()`

**File:** `src/main.js`
**Location:** `loadActions()` success handler

```javascript
// BEFORE:
document.getElementById('export-actions-btn').disabled = false;

// AFTER:
document.getElementById('export-actions-btn').disabled = workflowRuns.length === 0;
```

### Fix 5 — Call `renderWorkflowRuns` unconditionally in `refreshData()`

**File:** `src/main.js`
**Location:** `refreshData()` fulfilled branch for `runsRes`

This completes the R2 recommendation from `actions_tab_review.md`. Unconditional rendering means the table is always up to date after any data refresh, even when the user is on another tab at completion time.

```javascript
// BEFORE:
if (runsRes.status === "fulfilled") {
    workflowRuns = runsRes.value;
    actionsLoaded = true;
    if (activeTab === "actions") renderWorkflowRuns(workflowRuns);   // ← conditional
    updateActionStatusDot(workflowRuns);
    document.getElementById("export-actions-btn").disabled = workflowRuns.length === 0;
}

// AFTER:
if (runsRes.status === "fulfilled") {
    workflowRuns = runsRes.value;
    actionsLoaded = true;
    renderWorkflowRuns(workflowRuns);   // ← unconditional: always keep table in sync
    updateActionStatusDot(workflowRuns);
    document.getElementById("export-actions-btn").disabled = workflowRuns.length === 0;
}
```

Note: `renderWorkflowRuns` operates on DOM elements that may be hidden (not in the `active` tab panel). Updating hidden DOM is safe and has negligible performance cost. The element IDs used (`actions-tbody`, `actions-table`, `actions-empty`) all exist in the DOM regardless of which tab is active.

The rejection branch in `refreshData()` already shows the error div correctly (from the prior fix). No change needed there.

---

## Implementation Steps

**Step 1** — Apply `selectTrackedRepo` stale-state reset in `src/main.js`.
Add `actionsLoaded = false;` and `workflowRuns = [];` inside `selectTrackedRepo()`, before the `refreshData()` call.

**Step 2** — Apply `event: Option<String>` change in `src-tauri/src/github/actions.rs`.
In `RawWorkflowRun` struct: change `event: String` to `event: Option<String>`.
In `fetch_workflow_runs()` mapping block: change `event: r.event` to `event: r.event.unwrap_or_default()`.

**Step 3** — Add mock workflow run data in `src-tauri/src/mock/mod.rs`.
Replace the `Ok(vec![])` body of `get_workflow_runs` with the 4-item mock data specified in Fix 2 above.

**Step 4** — Fix export button in `loadActions()` in `src/main.js`.
Change `document.getElementById('export-actions-btn').disabled = false` to `document.getElementById('export-actions-btn').disabled = workflowRuns.length === 0`.

**Step 5** — Make `renderWorkflowRuns` unconditional in `refreshData()` in `src/main.js`.
Remove the `if (activeTab === "actions")` guard from the `renderWorkflowRuns(workflowRuns)` call in the fulfilled branch.

**Step 6** — Run build validation:
```
cd src-tauri && cargo build
cd src-tauri && cargo clippy -- -D warnings
cd src-tauri && cargo test
```

---

## Files to Modify

| File | Changes |
|------|---------|
| `src/main.js` | Fixes 1, 4, 5: stale state reset in `selectTrackedRepo`, export button in `loadActions`, unconditional render in `refreshData` |
| `src-tauri/src/github/actions.rs` | Fix 3: `event: Option<String>` + `.unwrap_or_default()` |
| `src-tauri/src/mock/mod.rs` | Fix 2: add 4 realistic `WorkflowRun` mock items |

Models (`src-tauri/src/models/mod.rs`) and the Tauri command registration (`src-tauri/src/main.rs`) require **no changes**.

---

## Dependencies and Risks

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|------------|
| Unconditional `renderWorkflowRuns` in `refreshData()` causes double-render when user IS on Actions tab and triggers refresh | Medium | Negligible — renders twice with identical data, imperceptible to user | None needed |
| Resetting `actionsLoaded = false` in `selectTrackedRepo` causes concurrent `loadActions()` + `refreshData()` API calls | Medium | Low — both calls are idempotent `GET` requests; last one to complete wins | The race is harmless; both calls return the same data |
| Mock data hardcodes `html_url` values beginning with `https://` | None | None — `renderWorkflowRuns` validates URLs with `/^https?:\/\//i` before inserting into `href` |
| `event: Option<String>` mask a future API regression (event becoming null) | Low | Low — `unwrap_or_default()` produces empty string, which renders as empty text in the UI | Add a note to the comment if desired |

---

## Verification: Expected behaviour after all fixes

| Scenario | Expected Result |
|----------|----------------|
| Dev-mock mode: user selects any repo and clicks Actions tab | 4 mock workflow runs shown in table |
| Start on Issues tab → select repo → click Actions tab | Correct runs shown (not stale empty data from previous state) |
| Select repo with runs → switch to another repo with no runs → click Actions | "No workflow runs found." shown (correctly for a repo with no runs) |
| Select repo → click Actions tab before `refreshData()` completes | `loadActions()` fires, fetches data independently; table shown when data arrives |
| `get_workflow_runs` returns an error | `#actions-error` div shows error text; `#actions-empty` ("No workflow runs found.") stays hidden |
| Workflow run with `event: null` in API response | Deserialises successfully; `event` rendered as empty string in table |
| Click ↺ refresh button while on Actions tab | Table updates immediately when `refreshData()` completes (unconditional render) |
| `loadActions()` returns empty list | Export CSV button is disabled; "No workflow runs found." shown |
