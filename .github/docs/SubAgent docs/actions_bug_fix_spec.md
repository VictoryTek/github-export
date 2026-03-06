# Actions Tab Bug Fix Specification

**Date:** 2026-03-05  
**Symptom:** "No workflow runs found." shown for ALL repositories, including repos with known GitHub Actions workflow runs.  
**Status:** Root causes identified — 2 bugs (1 Rust, 1 JavaScript) working in combination.

---

## Root Cause Analysis

Two separate bugs combine to produce the symptom. Neither alone is sufficient to explain "all repos, even repos that definitely have runs" — together they form a complete failure chain.

### The Failure Chain

1. User selects a repo while on the Actions tab (or selects a new repo after navigating to Actions)
2. `refreshData()` is called in `src/main.js`; it fires four parallel `invoke` calls including `get_workflow_runs`
3. `fetch_workflow_runs()` in `src-tauri/src/github/actions.rs` calls `client.get(url, None::<&()>)` asking octocrab to deserialize the response into `WorkflowRunsPage`
4. GitHub's API returns `{"total_count": N, "workflow_runs": [{..., "status": null, ...}, ...]}` — the `status` field is marked **`nullable: true`** in GitHub's OpenAPI specification and is commonly `null` for runs in `queued`, `pending`, or `waiting` states
5. Serde encounters `null` for `status: String` (a non-optional Rust field) → **entire `Vec<RawWorkflowRun>` deserialization fails** → `client.get()` returns `Err(octocrab::Error::Json { ... })`
6. `fetch_workflow_runs()` propagates the error via `?` → `get_workflow_runs` Tauri command returns `Err(String)`
7. In `refreshData()`, `runsRes.status === "rejected"` — which executes:
   ```js
   workflowRuns = [];
   actionsLoaded = false;
   if (activeTab === "actions") renderWorkflowRuns([]); // ← shows "No workflow runs found."
   console.error("get_workflow_runs failed:", runsRes.reason); // ← error only in DevTools
   ```
8. Because `activeTab === "actions"`, `renderWorkflowRuns([])` is called → the `actions-empty` div is revealed, showing **"No workflow runs found."** — **no error message is ever shown to the user**
9. The Rust error is swallowed. The user has no indication anything went wrong.

---

## Bugs Found

---

### BUG 1 — CRITICAL | Rust | `status: String` is non-nullable but GitHub field is nullable

**File:** `src-tauri/src/github/actions.rs`  
**Function:** `RawWorkflowRun` struct  
**Category:** Type mismatch / Deserialization failure

#### Explanation

GitHub's REST API OpenAPI specification defines the `status` field for workflow runs as:

```yaml
status:
  type: string
  nullable: true
  enum: [queued, in_progress, completed, waiting, requested, pending, action_required]
```

It is `nullable: true`. Any repo that has at least one run in a `queued`, `waiting`, `pending`, or other transitional state before GitHub assigns a status will have `"status": null` in the API response. When serde attempts to deserialize `null` into `String` (a non-optional Rust type), the entire `Vec<RawWorkflowRun>` deserialization fails — not gracefully per-item, but for the **entire response**. This causes `client.get()` to return `Err(octocrab::Error::Json)`, propagated as a Tauri command error.

The working reference modules (`security.rs`, `issues.rs`, `pulls.rs`) avoid this class of bug by using `Option<String>` for all potentially-nullable fields.

#### Current Code

```rust
// src-tauri/src/github/actions.rs — lines ~17-29
#[derive(Debug, Deserialize)]
struct RawWorkflowRun {
    id: u64,
    name: Option<String>,
    head_branch: Option<String>,
    run_number: u64,
    event: String,
    status: String,          // ← BUG: nullable in GitHub OpenAPI spec
    conclusion: Option<String>,
    actor: Option<RawActor>,
    created_at: String,
    run_started_at: Option<String>,
    html_url: String,
    workflow_id: u64,
}
```

#### Fix

Change `status: String` to `status: Option<String>`. Update the conversion in `fetch_workflow_runs()` to unwrap with a default (keeping the `WorkflowRun` model's `status: String` unchanged, as it is a domain model, not a raw API struct):

```rust
// src-tauri/src/github/actions.rs — RawWorkflowRun struct
#[derive(Debug, Deserialize)]
struct RawWorkflowRun {
    id: u64,
    name: Option<String>,
    head_branch: Option<String>,
    run_number: u64,
    event: String,
    status: Option<String>,      // ← FIXED: matches GitHub's nullable: true
    conclusion: Option<String>,
    actor: Option<RawActor>,
    created_at: String,
    run_started_at: Option<String>,
    html_url: String,
    workflow_id: u64,
}
```

And update the mapping inside `fetch_workflow_runs()` (the `.map(|r| WorkflowRun { ... })` block):

```rust
// Current:
status: r.status,

// Fixed:
status: r.status.unwrap_or_default(),
```

No change is needed to `models/mod.rs` — `WorkflowRun.status: String` remains correct because the domain model represents a processed value, not raw API data.

---

### BUG 2 — CRITICAL | JavaScript | `refreshData()` silently hides `get_workflow_runs` errors as empty state

**File:** `src/main.js`  
**Function:** `refreshData()`  
**Category:** Error swallowing / Silent failure

#### Explanation

`refreshData()` uses `Promise.allSettled()` to run all four tab fetches in parallel. In the rejection handler for `runsRes`, when the promise is rejected (i.e., the Rust command returned an error), the code calls `renderWorkflowRuns([])` when `activeTab === "actions"`. This renders the empty state div (`actions-empty`: "No workflow runs found.") **without ever showing the `actions-error` div or any error text**.

The error is only written to `console.error`, which users never see.

By contrast, `loadActions()` (which is called on Actions tab click) **correctly** handles errors by showing `errorEl.textContent = 'Failed to load workflow runs: ' + String(e)`. But `loadActions()` is only triggered when `actionsLoaded === false` AND the user explicitly clicks the Actions tab again. If the user is already on the Actions tab when they select a repo, they never trigger `loadActions()` — they just see the silently empty state.

This is the primary mechanism by which Bug 1's error becomes invisible to the user.

#### Current Code

```js
// src/main.js — inside refreshData(), the runsRes rejection path
  if (runsRes.status === "fulfilled") {
    workflowRuns = runsRes.value;
    actionsLoaded = true;
    if (activeTab === "actions") renderWorkflowRuns(workflowRuns);
    updateActionStatusDot(workflowRuns);
    document.getElementById("export-actions-btn").disabled = workflowRuns.length === 0;
  } else {
    workflowRuns = [];
    actionsLoaded = false;
    if (activeTab === "actions") renderWorkflowRuns([]);   // ← BUG: shows empty state, not error
    updateActionStatusDot([]);
    document.getElementById("export-actions-btn").disabled = true;
    console.error("get_workflow_runs failed:", runsRes.reason);  // ← error never shown to user
  }
```

#### Fix

When `activeTab === "actions"` and the run fetch was rejected, show the `actions-error` div with the error text instead of calling `renderWorkflowRuns([])`. This matches the pattern used by `loadActions()`:

```js
  if (runsRes.status === "fulfilled") {
    workflowRuns = runsRes.value;
    actionsLoaded = true;
    if (activeTab === "actions") renderWorkflowRuns(workflowRuns);
    updateActionStatusDot(workflowRuns);
    document.getElementById("export-actions-btn").disabled = workflowRuns.length === 0;
  } else {
    workflowRuns = [];
    actionsLoaded = false;
    if (activeTab === "actions") {
      // Show the error inline — do NOT call renderWorkflowRuns([]) which hides the cause
      const actionsErrorEl = document.getElementById("actions-error");
      const actionsEmptyEl = document.getElementById("actions-empty");
      const actionsTableEl = document.getElementById("actions-table");
      if (actionsErrorEl) {
        actionsErrorEl.textContent = "Failed to load workflow runs: " + String(runsRes.reason);
        actionsErrorEl.classList.remove("hidden");
      }
      if (actionsEmptyEl) actionsEmptyEl.classList.add("hidden");
      if (actionsTableEl) actionsTableEl.classList.add("hidden");
    }
    updateActionStatusDot([]);
    document.getElementById("export-actions-btn").disabled = true;
    console.error("get_workflow_runs failed:", runsRes.reason);
  }
```

---

## Summary Table

| # | Severity | File | Location | Description |
|---|----------|------|----------|-------------|
| 1 | **CRITICAL** | `src-tauri/src/github/actions.rs` | `RawWorkflowRun` struct | `status: String` should be `status: Option<String>` — GitHub API marks it nullable |
| 2 | **CRITICAL** | `src/main.js` | `refreshData()` rejection path for `runsRes` | Error silently converted to empty state; `actions-error` div never shown |

---

## Ordered List of Files to Modify

### 1. `src-tauri/src/github/actions.rs`

**Change 1a — `RawWorkflowRun` struct field:**

| | Code |
|---|---|
| **Before** | `status: String,` |
| **After** | `status: Option<String>,` |

**Change 1b — `fetch_workflow_runs()` mapping block:**

| | Code |
|---|---|
| **Before** | `status: r.status,` |
| **After** | `status: r.status.unwrap_or_default(),` |

### 2. `src/main.js`

**Change 2a — `refreshData()` rejection handler for `runsRes`:**

Replace the `else { ... }` block for `runsRes` rejection (the block that currently calls `renderWorkflowRuns([])` when `activeTab === "actions"`) with the fixed version that shows `actions-error` instead.

Full replacement shown in the Bug 2 fix section above.

---

## Verification

After applying these fixes, the following behaviors should be confirmed:

1. **Repos with only completed runs (no null-status runs):** Workflow runs load and display correctly.
2. **Repos with queued/pending runs (null status):** Workflow runs load and display correctly — the run with null status appears with an empty status string (or a fallback label if desired).
3. **When `get_workflow_runs` returns an error for any reason:** The `actions-error` div is shown with the error message. The empty state ("No workflow runs found.") is NOT shown when there is an error.
4. **`loadActions()` (click-triggered):** Behavior is unchanged — it already correctly shows errors.
5. **Token scope errors:** If the GitHub API returns 403 (token lacks `workflow` or `repo` scope), the error message is shown to the user (not silently hidden as empty).

---

## Notes: Token Scope (Not a Code Bug)

For completeness: the GitHub Actions API (`GET /repos/{owner}/{repo}/actions/runs`) requires:
- **Classic PAT**: `repo` scope (for private repos); public repos work with `public_repo`
- **Fine-grained PAT**: `Actions: Read` under Repository permissions
- **OAuth app**: must have `workflow` scope or `repo` scope

If a user's token lacks these scopes for private repos, GitHub returns HTTP 403 (not an empty array), which octocrab converts to `Err(...)` and which (after Bug 2 is fixed) will be shown as an error message to the user. No code changes are needed for this case — the error surfacing fix (Bug 2) handles it.
