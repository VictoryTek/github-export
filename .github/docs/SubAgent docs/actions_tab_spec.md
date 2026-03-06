# Actions Tab Feature Specification

**Feature**: GitHub Actions Workflow Runs Tab  
**Priority**: High  
**Author**: Research & Specification Subagent  
**Date**: 2025

---

## 1. Overview

Add a new "Actions" navigation tab to the GitHub Export Tauri v1 desktop application. The tab is placed **to the LEFT of the existing "Issues" tab**, making it the first tab in the navigation row. The tab displays a **notification-style status dot** (not a count badge) reflecting the pass/fail status of the most recent GitHub Actions workflow run in the selected repository.

---

## 2. Research Sources

1. **GitHub REST API — Workflow Runs**: `https://docs.github.com/en/rest/actions/workflow-runs`  
   → Confirms `GET /repos/{owner}/{repo}/actions/runs` endpoint, response shape, `status` and `conclusion` field enumerations.

2. **octocrab 0.38 docs.rs — `Octocrab` struct**: `https://docs.rs/octocrab/0.38.0/octocrab/struct.Octocrab.html`  
   → Confirms `client.actions()` → `ActionsHandler` EXISTS in octocrab 0.38. Also has `client.workflows()` → `WorkflowsHandler`. Direct HTTP via `client.get()` remains the recommended approach for consistency with the existing codebase pattern established in `security.rs`.

3. **Existing codebase — `src-tauri/src/github/security.rs`**:  
   → Establishes the direct HTTP call pattern: `client.get(&url, None::<&()>).await` with raw deserialization structs. This is the codebase's standard approach for API endpoints that do not have a fully builder-compatible native octocrab method.

4. **Existing codebase — `src/styles.css`**:  
   → CSS variable system, `.tab`, `.tab-badge`, `.spinner`/`@keyframes spin` patterns. Color values confirmed: `--green: #3fb950`, `--red: #f85149`, `--orange: #d29922`.

5. **Existing codebase — `src/main.js`** (lines 1–900):  
   → Tab switching pattern, `updateTabBadges()`, `refreshData()`, `Promise.allSettled`, `invoke()`, `stateBadge()`, `esc()`, `shortDate()`.

6. **Existing codebase — `src-tauri/src/export/csv_export.rs`**:  
   → `csv::Writer::from_writer(file)` pattern, `write_record()` for headers and rows, `wtr.flush()` required.

7. **GitHub REST API — Workflow Run Response Shape (confirmed fields)**:  
   - `id: u64`, `name: String`, `head_branch: Option<String>`, `run_number: u64`
   - `status: String` → values: `queued`, `in_progress`, `completed`
   - `conclusion: Option<String>` → `null` when not completed; values when completed: `success`, `failure`, `cancelled`, `skipped`, `timed_out`, `action_required`, `neutral`, `stale`
   - `event: String`, `actor.login: String`, `created_at: String`, `run_started_at: Option<String>`, `html_url: String`, `workflow_id: u64`

8. **Existing codebase — `src-tauri/src/main.rs`**:  
   → `#[tauri::command]` registration pattern, `#[cfg(not(feature = "dev-mock"))]` guard, `AppState` extraction pattern.

---

## 3. Current State Analysis

### Navigation (HTML)
```html
<nav id="tabs">
  <button id="issues-tab" class="tab active" data-tab="issues">Issues<span class="tab-badge" aria-hidden="true"></span></button>
  <button id="pulls-tab" class="tab" data-tab="pulls">Pull Requests<span class="tab-badge" aria-hidden="true"></span></button>
  <button id="security-tab" class="tab" data-tab="alerts">Security Alerts<span class="tab-badge" aria-hidden="true"></span></button>
</nav>
```

### Tab Panels (HTML)
```html
<div id="tab-issues" class="tab-panel active">...</div>
<div id="tab-pulls" class="tab-panel">...</div>
<div id="tab-alerts" class="tab-panel">...</div>
```

### State Variables (JS)
```js
let issues = [], pulls = [], alerts = [], activeTab = "issues", selectedRepo = null;
```

### Registered Tauri Commands (Rust, non-mock handler)
```
get_dev_mode, start_device_flow, poll_device_flow, authenticate_with_pat,
restore_session, list_accounts, add_account, switch_account, remove_account,
logout, list_repos, fetch_issues, fetch_pulls, fetch_security_alerts,
get_pull_detail, export_data, get_tracked_repos, add_tracked_repo,
remove_tracked_repo, list_all_repos
```

### `src-tauri/src/github/mod.rs` current content
```rust
pub mod auth;
pub mod detail;
pub mod issues;
pub mod pulls;
pub mod security;
```

---

## 4. Proposed Solution

### 4.1 Architecture Summary

A new `actions` module is added to the Rust backend (`src-tauri/src/github/actions.rs`) that fetches workflow runs using the established `client.get()` direct HTTP pattern. Two new Tauri commands are registered: `get_workflow_runs` (fetches runs for display) and `export_actions_csv` (exports runs to CSV). The frontend gains a new "Actions" tab button placed **before** the Issues tab, a `#tab-actions` panel with a table of workflow runs, and a visual status dot on the tab using a new CSS class `.tab-status-dot`.

### 4.2 Status Dot Logic

The status dot is derived from the **most recent workflow run** (first item in `workflow_runs` array, which is sorted newest-first by the API):

| Run Status | Conclusion | Dot State |
|---|---|---|
| `queued` or `in_progress` | (any/null) | Yellow/orange, pulsing |
| `completed` | `success` | Green, solid |
| `completed` | `failure`, `timed_out`, `action_required` | Red, solid |
| `completed` | `cancelled`, `skipped`, `neutral`, `stale` | Grey, solid |
| No runs returned | — | Hidden |

---

## 5. Implementation Steps

### Step 1: CSS — Add `.tab-status-dot` classes to `src/styles.css`

Add after the existing `.tab-badge` block:

```css
/* ─── Actions tab status dot ─────────────────────────────── */
.tab-status-dot {
  position: absolute;
  top: 5px;
  right: 5px;
  width: 8px;
  height: 8px;
  border-radius: 50%;
  display: none;
  flex-shrink: 0;
}

.tab-status-dot--success {
  background-color: var(--green);
}

.tab-status-dot--failure {
  background-color: var(--red);
}

.tab-status-dot--pending {
  background-color: var(--orange);
  animation: pulse-dot 1.2s ease-in-out infinite;
}

.tab-status-dot--neutral {
  background-color: var(--text-muted);
}

@keyframes pulse-dot {
  0%, 100% { opacity: 1; transform: scale(1); }
  50%       { opacity: 0.5; transform: scale(0.85); }
}
```

### Step 2: HTML — Add Actions tab button and panel in `src/index.html`

**2a. Add tab button** — Insert **before** the `issues-tab` button inside `<nav id="tabs">`:

```html
<button id="actions-tab" class="tab" data-tab="actions" aria-label="Actions">Actions<span class="tab-status-dot" aria-hidden="true"></span></button>
```

**2b. Add tab panel** — Insert **before** `<div id="tab-issues" ...>`:

```html
<div id="tab-actions" class="tab-panel">
  <div class="table-container">
    <table id="actions-table">
      <thead>
        <tr>
          <th>#</th>
          <th>Workflow</th>
          <th>Branch</th>
          <th>Event</th>
          <th>Status</th>
          <th>Actor</th>
          <th>Started</th>
          <th>Link</th>
        </tr>
      </thead>
      <tbody></tbody>
    </table>
  </div>
</div>
```

### Step 3: Rust model — Add `WorkflowRun` struct to `src-tauri/src/models/mod.rs`

Add the following struct at the end of the models file:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowRun {
    pub id: u64,
    pub name: String,
    pub head_branch: Option<String>,
    pub run_number: u64,
    pub event: String,
    pub status: String,
    pub conclusion: Option<String>,
    pub actor_login: String,
    pub created_at: String,
    pub run_started_at: Option<String>,
    pub html_url: String,
    pub workflow_id: u64,
}
```

### Step 4: Rust — Create `src-tauri/src/github/actions.rs`

Create new file modelled on `security.rs`:

```rust
use anyhow::{Context, Result};
use octocrab::Octocrab;
use serde::Deserialize;

use crate::models::WorkflowRun;

/// Raw deserialization struct matching the GitHub API response shape.
/// We only capture the fields we need; all others are ignored.
#[derive(Debug, Deserialize)]
struct RawActor {
    login: String,
}

#[derive(Debug, Deserialize)]
struct RawWorkflowRun {
    id: u64,
    name: Option<String>,
    head_branch: Option<String>,
    run_number: u64,
    event: String,
    status: String,
    conclusion: Option<String>,
    actor: Option<RawActor>,
    created_at: String,
    run_started_at: Option<String>,
    html_url: String,
    workflow_id: u64,
}

#[derive(Debug, Deserialize)]
struct WorkflowRunsPage {
    // total_count is present in the API response but we don't need it
    workflow_runs: Vec<RawWorkflowRun>,
}

pub async fn fetch_workflow_runs(
    client: &Octocrab,
    owner: &str,
    repo: &str,
) -> Result<Vec<WorkflowRun>> {
    let url = format!(
        "/repos/{owner}/{repo}/actions/runs?per_page=30&page=1"
    );

    let page: WorkflowRunsPage = client
        .get(&url, None::<&()>)
        .await
        .with_context(|| format!("Failed to fetch workflow runs for {owner}/{repo}"))?;

    let runs = page
        .workflow_runs
        .into_iter()
        .map(|r| WorkflowRun {
            id: r.id,
            name: r.name.unwrap_or_default(),
            head_branch: r.head_branch,
            run_number: r.run_number,
            event: r.event,
            status: r.status,
            conclusion: r.conclusion,
            actor_login: r.actor.map(|a| a.login).unwrap_or_default(),
            created_at: r.created_at,
            run_started_at: r.run_started_at,
            html_url: r.html_url,
            workflow_id: r.workflow_id,
        })
        .collect();

    Ok(runs)
}
```

### Step 5: Rust — Register `actions` module in `src-tauri/src/github/mod.rs`

Add `pub mod actions;` to the file:

```rust
pub mod actions;
pub mod auth;
pub mod detail;
pub mod issues;
pub mod pulls;
pub mod security;
```

### Step 6: Rust — Add CSV export for actions to `src-tauri/src/export/csv_export.rs`

Add a new function `write_actions_section` (or a standalone `export_actions_csv` if exported separately):

```rust
pub fn write_actions_section(
    wtr: &mut csv::Writer<std::fs::File>,
    runs: &[crate::models::WorkflowRun],
) -> anyhow::Result<()> {
    wtr.write_record(["[Workflow Runs]", "", "", "", "", "", "", ""])?;
    wtr.write_record(["ID", "Workflow", "Branch", "Event", "Status", "Conclusion", "Actor", "Started", "URL"])?;
    for run in runs {
        wtr.write_record([
            &run.id.to_string(),
            &run.name,
            run.head_branch.as_deref().unwrap_or(""),
            &run.event,
            &run.status,
            run.conclusion.as_deref().unwrap_or(""),
            &run.actor_login,
            run.run_started_at.as_deref().unwrap_or(&run.created_at),
            &run.html_url,
        ])?;
    }
    Ok(())
}
```

If a standalone CSV export command is needed (separate from the existing `export_data` command), create `export_actions_csv` in `csv_export.rs`:

```rust
pub fn export_actions_csv(
    runs: &[crate::models::WorkflowRun],
    path: &str,
) -> anyhow::Result<()> {
    let file = std::fs::File::create(path).context("Could not create CSV file")?;
    let mut wtr = csv::Writer::from_writer(file);
    write_actions_section(&mut wtr, runs)?;
    wtr.flush()?;
    Ok(())
}
```

### Step 7: Rust — Add Tauri commands to `src-tauri/src/main.rs`

**7a. Add `get_workflow_runs` command** (non-mock):

```rust
#[cfg(not(feature = "dev-mock"))]
#[tauri::command]
async fn get_workflow_runs(
    owner: String,
    repo: String,
    state: State<'_, Mutex<AppState>>,
) -> Result<Vec<models::WorkflowRun>, String> {
    let client = {
        let app = state.lock().map_err(|e| e.to_string())?;
        app.client.clone().ok_or("Not authenticated")?
    };
    github::actions::fetch_workflow_runs(&client, &owner, &repo)
        .await
        .map_err(|e| e.to_string())
}
```

**7b. Add `export_actions_csv` command** (non-mock):

```rust
#[cfg(not(feature = "dev-mock"))]
#[tauri::command]
async fn export_actions_csv(
    runs: Vec<models::WorkflowRun>,
    file_path: String,
) -> Result<String, String> {
    export::csv_export::export_actions_csv(&runs, &file_path)
        .map(|_| format!("Exported {} workflow runs to {}", runs.len(), file_path))
        .map_err(|e| e.to_string())
}
```

**7c. Register both commands** in the `tauri::generate_handler![]` macro — add to the existing non-mock list:

```
get_workflow_runs, export_actions_csv,
```

**7d. Add mock stubs** inside the `#[cfg(feature = "dev-mock")]` block matching the existing mock pattern:

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

### Step 8: JavaScript — Update `src/main.js`

**8a. Add state variable** — alongside existing state variables:

```js
let actions = [];
```

**8b. Add `loadActions` function** — to fetch and render workflow runs:

```js
async function loadActions(owner, name) {
  const tbody = $("#actions-table tbody");
  tbody.innerHTML = `<tr><td colspan="8" class="fetch-info"><span class="spinner"></span> Loading…</td></tr>`;
  try {
    actions = await invoke("get_workflow_runs", { owner, repo: name });
    renderActions(actions);
    updateActionStatusDot(actions);
  } catch (err) {
    tbody.innerHTML = `<tr><td colspan="8" class="fetch-error">Failed to load workflow runs: ${esc(String(err))}</td></tr>`;
    updateActionStatusDot([]);
  }
}
```

**8c. Add `renderActions` function**:

```js
function renderActions(runs) {
  const tbody = $("#actions-table tbody");
  if (!runs || runs.length === 0) {
    tbody.innerHTML = `<tr><td colspan="8" class="fetch-info">No workflow runs found.</td></tr>`;
    return;
  }
  tbody.innerHTML = runs.map(r => {
    const conclusionBadge = r.conclusion
      ? `<span class="badge badge-${r.conclusion === 'success' ? 'open' : r.conclusion === 'failure' ? 'closed' : 'label'}">${esc(r.conclusion)}</span>`
      : `<span class="badge badge-label">${esc(r.status)}</span>`;
    return `<tr>
      <td>${r.run_number}</td>
      <td>${esc(r.name)}</td>
      <td>${esc(r.head_branch || '')}</td>
      <td>${esc(r.event)}</td>
      <td>${conclusionBadge}</td>
      <td>${esc(r.actor_login)}</td>
      <td>${shortDate(r.run_started_at || r.created_at)}</td>
      <td><a href="#" data-url="${esc(r.html_url)}" class="ext-link">View ↗</a></td>
    </tr>`;
  }).join('');

  // Wire up external links via Tauri shell.open
  tbody.querySelectorAll('.ext-link').forEach(a => {
    a.addEventListener('click', e => {
      e.preventDefault();
      window.__TAURI__.shell.open(a.dataset.url);
    });
  });
}
```

**8d. Add `updateActionStatusDot` function**:

```js
function updateActionStatusDot(runs) {
  const dot = document.querySelector('#actions-tab .tab-status-dot');
  if (!dot) return;

  // Reset all modifier classes
  dot.classList.remove(
    'tab-status-dot--success',
    'tab-status-dot--failure',
    'tab-status-dot--pending',
    'tab-status-dot--neutral'
  );

  if (!runs || runs.length === 0) {
    dot.style.display = 'none';
    dot.closest('button').removeAttribute('data-status');
    return;
  }

  const latest = runs[0];
  let cls, label;

  if (latest.status === 'queued' || latest.status === 'in_progress') {
    cls = 'tab-status-dot--pending';
    label = `Latest run: ${latest.status}`;
  } else if (latest.conclusion === 'success') {
    cls = 'tab-status-dot--success';
    label = 'Latest run: passed';
  } else if (['failure', 'timed_out', 'action_required'].includes(latest.conclusion)) {
    cls = 'tab-status-dot--failure';
    label = `Latest run: ${latest.conclusion}`;
  } else {
    cls = 'tab-status-dot--neutral';
    label = `Latest run: ${latest.conclusion || latest.status}`;
  }

  dot.classList.add(cls);
  dot.style.display = 'block';
  // Accessibility: expose status on the button itself
  dot.closest('button').setAttribute('data-status', label);
  dot.setAttribute('title', label);
}
```

**8e. Add `clearActionStatusDot` call** — update the existing `clearTabBadges()` function to also reset the status dot:

```js
function clearTabBadges() {
  updateTabBadges(0, 0, 0);
  updateActionStatusDot([]);   // <-- add this line
}
```

**8f. Wire `loadActions` into `refreshData`** — add to `Promise.allSettled` in the existing `refreshData` function:

```js
// Add actions to the allSettled call alongside issues/pulls/alerts
const [issuesRes, pullsRes, alertsRes, actionsRes] = await Promise.allSettled([
  invoke("fetch_issues", { owner, repo: name, filters }),
  invoke("fetch_pulls",  { owner, repo: name, filters }),
  invoke("fetch_security_alerts", { owner, repo: name, state: filters.state }),
  invoke("get_workflow_runs", { owner, repo: name }),
]);
```

Then after resolving actionsRes:

```js
if (actionsRes.status === 'fulfilled') {
  actions = actionsRes.value;
} else {
  actions = [];
}
renderActions(actions);
updateActionStatusDot(actions);
```

**8g. Add export handler** for actions CSV export button (wire to a new button `#export-actions-btn` or extend the existing export modal):

```js
// In the export section, add handling for actions format:
// Pass actions array alongside issues/pulls/alerts when format is 'csv'
// For actions-only export:
async function doExportActions() {
  const ext = 'csv';
  const filePath = await save({
    filters: [{ name: 'CSV', extensions: ['csv'] }],
    defaultPath: `github-actions-export.csv`,
  });
  if (!filePath) return;
  const msg = await invoke("export_actions_csv", { runs: actions, filePath });
  alert(msg);
}
```

---

## 6. DOM Position Specification

### Tab Button Order (left to right in `<nav id="tabs">`)
1. **Actions** ← new, placed FIRST
2. Issues (existing)
3. Pull Requests (existing)
4. Security Alerts (existing)

### Tab Panel Order (in DOM, before existing panels)
1. `<div id="tab-actions" class="tab-panel">` ← new, placed FIRST
2. `<div id="tab-issues" class="tab-panel active">` (existing)
3. `<div id="tab-pulls" class="tab-panel">` (existing)
4. `<div id="tab-alerts" class="tab-panel">` (existing)

---

## 7. File Change Map

| File | Change Type | Description |
|---|---|---|
| `src/index.html` | Modified | Add `#actions-tab` button (before `#issues-tab`) + `#tab-actions` panel (before `#tab-issues`) |
| `src/styles.css` | Modified | Add `.tab-status-dot` and modifier classes + `@keyframes pulse-dot` |
| `src/main.js` | Modified | Add `actions` state var, `loadActions()`, `renderActions()`, `updateActionStatusDot()`, wire into `refreshData()`, extend `clearTabBadges()`, add export handler |
| `src-tauri/src/github/actions.rs` | Created | New file: `fetch_workflow_runs()` using `client.get()` pattern |
| `src-tauri/src/github/mod.rs` | Modified | Add `pub mod actions;` |
| `src-tauri/src/models/mod.rs` | Modified | Add `WorkflowRun` struct |
| `src-tauri/src/export/csv_export.rs` | Modified | Add `write_actions_section()` and `export_actions_csv()` functions |
| `src-tauri/src/main.rs` | Modified | Add `get_workflow_runs` and `export_actions_csv` commands; register in both non-mock and mock invoke handlers |

---

## 8. Dependencies

**No new Cargo.toml dependencies required.** All needed crates are already present:
- `octocrab = "0.38"` — HTTP client via `client.get()`
- `serde` with `derive` — for `RawWorkflowRun` deserialization and `WorkflowRun` serialization
- `anyhow = "1"` — error handling with `.with_context()`
- `csv = "1.3"` — CSV writing
- `chrono = "0.4"` — date formatting (already used in other export functions)

---

## 9. API Details

### Endpoint
```
GET /repos/{owner}/{repo}/actions/runs?per_page=30&page=1
```

### Response Shape (relevant fields)
```json
{
  "total_count": 42,
  "workflow_runs": [
    {
      "id": 30433642,
      "name": "Build",
      "head_branch": "main",
      "run_number": 562,
      "event": "push",
      "status": "completed",
      "conclusion": "success",
      "workflow_id": 159038,
      "created_at": "2020-01-22T19:33:08Z",
      "updated_at": "2020-01-22T19:33:08Z",
      "run_started_at": "2020-01-22T19:33:08Z",
      "html_url": "https://github.com/octo-org/octo-repo/actions/runs/30433642",
      "actor": {
        "login": "octocat"
      }
    }
  ]
}
```

### `status` field values
- `queued` — run is queued but not started
- `in_progress` — run is currently executing
- `completed` — run has finished (inspect `conclusion` for outcome)

### `conclusion` field values (only present when `status == "completed"`)
- `success` — all jobs passed
- `failure` — one or more jobs failed
- `cancelled` — workflow was cancelled
- `skipped` — workflow was skipped
- `timed_out` — workflow exceeded time limit
- `action_required` — workflow requires manual approval
- `neutral` — workflow completed with neutral result
- `stale` — workflow became stale

---

## 10. Accessibility Considerations

- The `.tab-status-dot` span has `aria-hidden="true"` so screen readers skip the decorative dot.
- Status information is exposed via `title` attribute on the dot span (for mouse hover) and `data-status` attribute on the parent `<button>` element.
- The button's accessible name remains "Actions" (the text content), satisfying WCAG 2.1 SC 4.1.2.
- The dot uses CSS `animation` (not JavaScript-driven animation), respecting `prefers-reduced-motion` if added. Implementor should consider adding:
  ```css
  @media (prefers-reduced-motion: reduce) {
    .tab-status-dot--pending { animation: none; }
  }
  ```

---

## 11. Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|---|---|---|---|
| Repository has no Actions workflows | Medium | Low | Return empty array; dot remains hidden; table shows "No workflow runs found." |
| Token lacks `actions: read` permission | Low–Medium | Medium | API returns 403; caught in `loadActions` catch block; error displayed in table |
| octocrab `client.get()` API breaking change in future update | Low | High | Pattern is already in use in `security.rs`; no new risk introduced |
| Large number of runs slowing the UI | Low | Low | `per_page=30` limit applied at query level |
| `refreshData()` now has 4 parallel calls instead of 3 | Low | Negligible | `Promise.allSettled` handles any number of promises; failure of one does not block others |
| Status dot not visible on small tab buttons | Low | Medium | Dot is 8×8px positioned `top:5px; right:5px`; CSS absolute positioning keeps it within the relative-positioned `.tab` container |

---

## 12. Implementation Order

Execute in this sequence to ensure compilability at each step:

1. `src-tauri/src/models/mod.rs` — add `WorkflowRun` struct
2. `src-tauri/src/github/actions.rs` — create file (depends on `WorkflowRun` in models)
3. `src-tauri/src/github/mod.rs` — add `pub mod actions;`
4. `src-tauri/src/export/csv_export.rs` — add actions export functions
5. `src-tauri/src/main.rs` — add Tauri commands referencing `github::actions` and `export::csv_export`
6. `src/styles.css` — add `.tab-status-dot` CSS
7. `src/index.html` — add tab button and panel
8. `src/main.js` — add action state, functions, wire into existing refresh flow

---

*Spec complete. Output path: `.github/docs/SubAgent docs/actions_tab_spec.md`*
