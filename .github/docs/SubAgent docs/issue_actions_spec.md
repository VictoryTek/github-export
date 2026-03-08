# Issue Actions Specification
## Feature: Open, Close, and Comment on GitHub Issues

**Project:** GitHub Export  
**Type:** Tauri v1 Desktop App (Rust + Vanilla JS)  
**Date:** 2026-03-07  
**Spec Author:** Research Subagent

---

## Table of Contents

1. [Current State Analysis](#1-current-state-analysis)
2. [Credible Sources Researched](#2-credible-sources-researched)
3. [Proposed Solution](#3-proposed-solution)
4. [Rust Implementation Steps](#4-rust-implementation-steps)
5. [Frontend Implementation Steps](#5-frontend-implementation-steps)
6. [Tauri Command Registration](#6-tauri-command-registration)
7. [Mock Stub Implementation](#7-mock-stub-implementation)
8. [Dependencies](#8-dependencies)
9. [Security Considerations](#9-security-considerations)
10. [Risks and Mitigations](#10-risks-and-mitigations)

---

## 1. Current State Analysis

### 1.1 Issues Tab — What Exists Today

**Data fetching (`src/main.js`):**
- `refreshData()` calls `invoke("fetch_issues", { owner, repo: name, filters })` in a `Promise.allSettled` call alongside pulls, alerts, and workflow runs.
- Results stored in the module-level `issues` array.
- Filters built by `buildFilters()`: state (open/closed/all), sort, direction, search, page, per_page.

**Rendering (`renderIssues`):**
- Iterates `issues` array, building HTML rows with `template literals`.
- Each issue renders a `data-row` (clickable) and a `detail-row` (collapsed by default).
- Columns in the data row: `#`, `Title`, `State` (badge), `Author`, `Labels`, `Created`.
- Clicking a data row calls `toggleDetailRow('issues', idx)`.

**Detail panel (`buildIssueDetail(issue)`):**
- Builds a `detail-content` div inside the detail row's `detail-body`.
- Structure:
  1. `detail-header`: type badge ("Issue #N"), state badge, close button (`×`).
  2. `detail-body-text markdown-body`: rendered Markdown body via `renderMarkdown()`.
  3. `detail-meta-grid`: Author, Assignees, Labels, Milestone, Comments count, Created, Updated, optionally Closed.
  4. `detail-footer`: "Open on GitHub ↗" external link.
- **No action buttons exist** — the panel is read-only.

**Expandable row mechanics (`toggleDetailRow`):**
- Collapses any currently open row first.
- Expands the target row by adding class `expanded` to the `detail-row` and `row-expanded` to the `data-row`.
- For PRs, lazily calls `invoke("get_pull_detail", ...)` on first expand to load diff stats.
- The lazy-load pattern uses `document.getElementById(\`pull-stats-${pull.number}\`)` to target a known placeholder `<div>` in the already-rendered HTML.

**`Issue` model (`src-tauri/src/models/mod.rs`):**
```rust
pub struct Issue {
    pub number: u64,
    pub title: String,
    pub state: String,       // "Open" or "Closed" via format!("{:?}", i.state)
    pub author: String,
    pub labels: Vec<String>,
    pub assignees: Vec<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub closed_at: Option<DateTime<Utc>>,
    pub html_url: String,
    pub body: Option<String>,
    pub comments: u32,
    pub milestone: Option<String>,
}
```

**`issues.rs` (`src-tauri/src/github/issues.rs`):**
- Contains `list_repos`, `list_all_repos`, `fetch_issues`.
- All functions take `&Octocrab` as first parameter, return `Result<T>` via `anyhow`.
- Issue mapping is inline in `fetch_issues` using `format!("{:?}", i.state)` for the state string.
- No `close_issue`, `reopen_issue`, or `add_issue_comment` functions exist.

**Tauri command registration (`src-tauri/src/main.rs`):**
- `#[cfg(not(feature = "dev-mock"))]` guards all GitHub-calling commands.
- Commands registered in `tauri::generate_handler![...]` macro in `main()`.
- Client extracted from `state.lock()?.client.clone().ok_or("Not authenticated")?` pattern (used consistently in all commands).

**CSS state (`src/styles.css`):**
- Dark GitHub-themed design (`--bg: #0d1117`, `--surface: #161b22`, `--border: #30363d`).
- `detail-body` transitions: `max-height: 0` → `700px` on expand, with padding and opacity transitions.
- Existing action-adjacent UI: `.detail-close-btn` (circular ×), `.detail-open-link` (external link), `.btn-cancel`, `.btn-github-signin`.
- Badge colors: `--green: #3fb950` (open), `--red: #f85149` (closed), `--accent: #58a6ff` (buttons/links).

**Mock data (`src-tauri/src/mock/mod.rs`):**
- Issues have hardcoded states: "open" or "closed" (lowercase string, not pascal-case from `format!("{:?}", ...)`).
- Mock commands do NOT include `close_issue`, `reopen_issue`, or `add_issue_comment`.

**Key gap identified:** The `state` field in mock data uses lowercase strings ("open", "closed") while the real `fetch_issues` produces pascal-case ("Open", "Closed") from `format!("{:?}", i.state)`. The frontend `stateBadge()` already lowercases: `const s = state.toLowerCase()`. This is handled correctly. The new `close_issue`/`reopen_issue` Rust functions must also return pascal-case state strings via `format!("{:?}", ...)`.

---

## 2. Credible Sources Researched

1. **GitHub REST API — Update an Issue**
   `PATCH /repos/{owner}/{repo}/issues/{issue_number}`
   Body: `{"state": "closed"}` or `{"state": "open"}`
   Requires: `repo` write scope. Returns the full updated issue object.
   Source: https://docs.github.com/en/rest/issues/issues#update-an-issue

2. **GitHub REST API — Create an Issue Comment**
   `POST /repos/{owner}/{repo}/issues/{issue_number}/comments`
   Body: `{"body": "comment text"}`
   Requires: `repo` write scope. Returns the created comment object.
   Maximum comment body length: 65,536 characters.
   Source: https://docs.github.com/en/rest/issues/comments#create-an-issue-comment

3. **octocrab 0.38 — `IssueHandler::update`**
   `client.issues(owner, repo).update(issue_number).state(IssueState::Closed).send().await`
   Returns: `octocrab::models::issues::Issue`
   `IssueState` enum is `octocrab::models::IssueState` with variants `Open` and `Closed`.
   Source: https://docs.rs/octocrab/0.38.0/octocrab/issues/struct.IssueHandler.html

4. **octocrab 0.38 — `IssueHandler::create_comment`**
   `client.issues(owner, repo).create_comment(issue_number, body).await`
   Returns: `octocrab::models::issues::Comment`
   Source: https://docs.rs/octocrab/0.38.0/octocrab/issues/struct.IssueHandler.html

5. **Tauri v1 — Async Command Pattern**
   `#[tauri::command] async fn name(param: Type, state: State<'_, Mutex<AppState>>) -> Result<T, String>`
   Frontend invocation: `await window.__TAURI__.tauri.invoke("name", { param: value })`
   Tauri auto-converts camelCase JS keys to snake_case Rust parameters.
   Source: https://tauri.app/v1/guides/features/command/

6. **GitHub OAuth Scopes for Issue Write Operations**
   The `repo` scope grants full read/write access to repository data including issues and comments.
   The `public_repo` scope suffices for public repositories only; `repo` is needed for private repos.
   The current app already requests `repo` scope in `auth.rs`: `const OAUTH_SCOPES: &str = "repo security_events";`
   Source: https://docs.github.com/en/developers/apps/building-oauth-apps/scopes-for-oauth-apps

7. **Error handling in Tauri — mapping `anyhow::Error` to `String`**
   The project uses `.map_err(|e| e.to_string())` to convert `anyhow::Error` to `String` for Tauri's `Result<T, String>`. The frontend receives this as a rejected promise whose reason is the error string.
   Source: Tauri v1 docs + existing codebase pattern (all commands in `main.rs`).

8. **UI/UX Pattern — Inline Actions in Detail Panels**
   GitHub's own web UI shows issue state-change and comment actions inline in the issue page below the existing content. The same convention applies here: actions appear at the bottom of the expanded detail panel, below the metadata grid, above the external link footer. This avoids the overhead of a separate modal for simple state-change actions.
   Source: github.com issue UI pattern analysis.

---

## 3. Proposed Solution

### 3.1 Design Decisions

**Action placement:** Actions are embedded directly in the existing issue detail panel (`buildIssueDetail`), below the metadata grid, above the "Open on GitHub" footer. No new modal overlay is required. This follows the lazy-loaded PR diff stats pattern already in the codebase.

**State change UX:** A single button toggles between "Close Issue" and "Reopen Issue" based on the current issue state. The button color matches the action: red for close, green for reopen. A confirmation dialog prevents accidental state changes.

**Comment UX:** An inline `<textarea>` with an "Add Comment" button. No modal. Status feedback (posting/success/error) appears inline via a `<span>` next to the button. After success, the textarea clears and the comment count on the issue is incremented locally.

**State synchronization after actions:**
- `close_issue` and `reopen_issue` return the full updated `models::Issue` from the backend. The frontend replaces `issues[idx]` with the returned value and surgically updates the state badge in the data row without triggering a full re-render (preserving the open detail panel).
- `add_issue_comment` returns `()`. The frontend increments `issues[idx].comments` by 1 locally.

**No new dependencies:** `octocrab` already supports `update()` and `create_comment()`. No new Cargo crates are needed.

**`buildIssueDetail` signature change:** Add `idx` parameter: `buildIssueDetail(issue, idx)`. All call sites updated.

**Max-height adjustment:** The detail panel `max-height` is currently `700px`. Adding the action section (approximately 120–150px) risks clipping with long bodies. The spec increases it to `900px`.

### 3.2 Architecture Diagram

```
JS Frontend                       Rust Backend (Tauri Command)     GitHub REST API
-----------                       ----------------------------     ---------------
handleIssueStateChange(idx)
  └──invoke("close_issue", {...})─▶ close_issue(owner,repo,num)
                                    └──issues.close_issue(...)────▶ PATCH /repos/.../issues/{n}
                                                                   ◀─── Updated Issue JSON
                                    └──map_issue(octocrab::Issue)
                                   ◀─── models::Issue (JSON)
  └── issues[idx] = updatedIssue
  └── updateStateBadgeInRow(idx)
  └── updateActionButton(idx)

handleAddIssueComment(idx)
  └──invoke("add_issue_comment",{})▶ add_issue_comment(owner,repo,num,body)
                                    └──issues.add_issue_comment(.)─▶ POST /repos/.../issues/{n}/comments
                                                                   ◀─── Comment JSON
                                    └──Ok(())
                                   ◀─── null (unit serialized as JSON null)
  └── issues[idx].comments += 1
  └── textarea.value = ""
  └── show success status
```

---

## 4. Rust Implementation Steps

### 4.1 `src-tauri/src/github/issues.rs`

**Step 1: Add `IssueState` import at the top.**

Add to existing imports:
```rust
use octocrab::models::IssueState;
```

**Step 2: Extract `map_issue` private helper.**

Add this private function immediately before `fetch_issues`:

```rust
/// Maps an octocrab issues API response to our domain `Issue` model.
fn map_issue(i: octocrab::models::issues::Issue) -> crate::models::Issue {
    crate::models::Issue {
        number: i.number,
        title: i.title,
        state: format!("{:?}", i.state),
        author: i.user.login.clone(),
        labels: i.labels.iter().map(|l| l.name.clone()).collect(),
        assignees: i.assignees.iter().map(|a| a.login.clone()).collect(),
        created_at: i.created_at,
        updated_at: i.updated_at,
        closed_at: i.closed_at,
        html_url: i.html_url.to_string(),
        body: i.body,
        comments: i.comments,
        milestone: i.milestone.as_ref().map(|m| m.title.clone()),
    }
}
```

**Step 3: Refactor the mapping in `fetch_issues` to use `map_issue`.**

Replace the inline `.map(|i| Issue { ... })` closure in `fetch_issues` to call `map_issue`:

```rust
    let issues = page
        .take_items()
        .into_iter()
        // GitHub's API returns PRs in the issues endpoint – filter them out
        .filter(|i| i.pull_request.is_none())
        .map(map_issue)
        .collect();
```

**Step 4: Add `close_issue` function.**

Append after `fetch_issues`:

```rust
/// Close an issue by setting its state to Closed.
/// Returns the updated `Issue` model.
pub async fn close_issue(
    client: &Octocrab,
    owner: &str,
    repo: &str,
    issue_number: u64,
) -> Result<crate::models::Issue> {
    let updated = client
        .issues(owner, repo)
        .update(issue_number)
        .state(IssueState::Closed)
        .send()
        .await
        .context("Failed to close issue")?;
    Ok(map_issue(updated))
}
```

**Step 5: Add `reopen_issue` function.**

Append after `close_issue`:

```rust
/// Reopen a closed issue by setting its state to Open.
/// Returns the updated `Issue` model.
pub async fn reopen_issue(
    client: &Octocrab,
    owner: &str,
    repo: &str,
    issue_number: u64,
) -> Result<crate::models::Issue> {
    let updated = client
        .issues(owner, repo)
        .update(issue_number)
        .state(IssueState::Open)
        .send()
        .await
        .context("Failed to reopen issue")?;
    Ok(map_issue(updated))
}
```

**Step 6: Add `add_issue_comment` function.**

Append after `reopen_issue`:

```rust
/// Post a new comment on an issue.
/// The `body` must be non-empty and at most 65,536 characters (validated by
/// the Tauri command layer before this function is called).
pub async fn add_issue_comment(
    client: &Octocrab,
    owner: &str,
    repo: &str,
    issue_number: u64,
    body: &str,
) -> Result<()> {
    client
        .issues(owner, repo)
        .create_comment(issue_number, body)
        .await
        .context("Failed to add comment")?;
    Ok(())
}
```

### 4.2 Final state of `issues.rs` imports section

```rust
use anyhow::{Context, Result};
use octocrab::models::IssueState;
use octocrab::params;
use octocrab::Octocrab;

use crate::models::{FilterParams, Issue, Repo};
```

### 4.3 Complete function signatures for reference

| Function | Parameters | Return |
|---|---|---|
| `map_issue` | `octocrab::models::issues::Issue` | `crate::models::Issue` |
| `close_issue` | `&Octocrab, &str, &str, u64` | `Result<crate::models::Issue>` |
| `reopen_issue` | `&Octocrab, &str, &str, u64` | `Result<crate::models::Issue>` |
| `add_issue_comment` | `&Octocrab, &str, &str, u64, &str` | `Result<()>` |

---

## 5. Frontend Implementation Steps

### 5.1 `src/main.js` — Update `buildIssueDetail` signature and call site

**Step 1: Add `idx` parameter to `buildIssueDetail`.**

Change the function signature from:
```javascript
function buildIssueDetail(issue) {
```
to:
```javascript
function buildIssueDetail(issue, idx) {
```

**Step 2: Update the call site in `renderIssues`.**

Currently:
```javascript
          <div class="detail-body">${buildIssueDetail(i)}</div>
```
Change to:
```javascript
          <div class="detail-body">${buildIssueDetail(i, idx)}</div>
```

**Step 3: Add the actions section to the `buildIssueDetail` return template.**

Insert a new `detail-actions` div between the closing `</div>` of `detail-meta-grid` and the `detail-footer` div. The full updated return value of `buildIssueDetail` is shown below (only the new section is added — all existing content is preserved):

```javascript
function buildIssueDetail(issue, idx) {
  const assignees = issue.assignees && issue.assignees.length
    ? issue.assignees.map(esc).join(", ")
    : "—";
  const labels = issue.labels && issue.labels.length
    ? labelBadges(issue.labels)
    : "—";
  const milestone = issue.milestone ? esc(issue.milestone) : "—";
  const comments = issue.comments != null ? issue.comments : "—";
  const closedDate = issue.closed_at ? shortDate(issue.closed_at) : null;

  const isOpen = issue.state.toLowerCase() === 'open';
  const actionBtnClass = isOpen ? 'btn-action btn-close-issue' : 'btn-action btn-reopen-issue';
  const actionBtnLabel = isOpen ? 'Close Issue' : 'Reopen Issue';

  return `
    <div class="detail-content">
      <div class="detail-header">
        <span class="detail-type-badge">Issue #${issue.number}</span>
        ${stateBadge(issue.state)}
        <button class="detail-close-btn" onclick="collapseAllRows()" title="Close">×</button>
      </div>
      <div class="detail-body-text markdown-body">${renderMarkdown(issue.body)}</div>
      <div class="detail-meta-grid">
        <div class="detail-meta-item">
          <span class="detail-meta-label">Author</span>
          <span>${esc(issue.author)}</span>
        </div>
        <div class="detail-meta-item">
          <span class="detail-meta-label">Assignees</span>
          <span>${assignees}</span>
        </div>
        <div class="detail-meta-item">
          <span class="detail-meta-label">Labels</span>
          <span>${labels}</span>
        </div>
        <div class="detail-meta-item">
          <span class="detail-meta-label">Milestone</span>
          <span>${milestone}</span>
        </div>
        <div class="detail-meta-item">
          <span class="detail-meta-label">Comments</span>
          <span>${comments}</span>
        </div>
        <div class="detail-meta-item">
          <span class="detail-meta-label">Created</span>
          <span>${shortDate(issue.created_at)}</span>
        </div>
        <div class="detail-meta-item">
          <span class="detail-meta-label">Updated</span>
          <span>${shortDate(issue.updated_at)}</span>
        </div>
        ${closedDate ? `
        <div class="detail-meta-item">
          <span class="detail-meta-label">Closed</span>
          <span>${closedDate}</span>
        </div>` : ""}
      </div>
      <div class="detail-actions">
        <div class="detail-action-group">
          <button
            class="${actionBtnClass}"
            id="issue-action-btn-${idx}"
            onclick="handleIssueStateChange(${idx})"
          >${actionBtnLabel}</button>
          <span class="issue-action-status" id="issue-action-status-${idx}"></span>
        </div>
        <div class="detail-comment-form">
          <textarea
            class="issue-comment-input"
            id="issue-comment-input-${idx}"
            placeholder="Leave a comment\u2026"
            rows="3"
          ></textarea>
          <div class="detail-comment-footer">
            <span class="issue-comment-status" id="issue-comment-status-${idx}"></span>
            <button class="btn-action btn-add-comment" onclick="handleAddIssueComment(${idx})">Add Comment</button>
          </div>
        </div>
      </div>
      <div class="detail-footer">
        <a href="${esc(issue.html_url)}" target="_blank" rel="noopener noreferrer" class="detail-open-link">Open on GitHub \u2197</a>
      </div>
    </div>`;
}
```

### 5.2 `src/main.js` — Add `handleIssueStateChange`

Add this function in the "Detail panel builders" section, after `buildIssueDetail`:

```javascript
async function handleIssueStateChange(idx) {
  if (!selectedRepo) return;
  const issue = issues[idx];
  const btn = document.getElementById(`issue-action-btn-${idx}`);
  const statusEl = document.getElementById(`issue-action-status-${idx}`);
  if (!btn || !statusEl) return;

  const isOpen = issue.state.toLowerCase() === 'open';
  const commandName = isOpen ? 'close_issue' : 'reopen_issue';
  const confirmMsg = isOpen
    ? `Close issue #${issue.number}: "${issue.title}"?`
    : `Reopen issue #${issue.number}: "${issue.title}"?`;

  if (!confirm(confirmMsg)) return;

  btn.disabled = true;
  statusEl.textContent = isOpen ? 'Closing\u2026' : 'Reopening\u2026';
  statusEl.className = 'issue-action-status';

  try {
    const updated = await invoke(commandName, {
      owner: selectedRepo.owner,
      repo: selectedRepo.name,
      issueNumber: issue.number,
    });

    // Update the issues array in place
    issues[idx] = updated;

    // Surgically update the state badge in the data row (no full re-render)
    const dataRow = document.querySelector(`#issues-table .data-row[data-idx="${idx}"]`);
    if (dataRow) dataRow.cells[2].innerHTML = stateBadge(updated.state);

    // Update the action button for the new state
    const newIsOpen = updated.state.toLowerCase() === 'open';
    btn.textContent = newIsOpen ? 'Close Issue' : 'Reopen Issue';
    btn.className = `btn-action ${newIsOpen ? 'btn-close-issue' : 'btn-reopen-issue'}`;
    btn.disabled = false;

    statusEl.textContent = isOpen ? '\u2713 Issue closed' : '\u2713 Issue reopened';
    statusEl.className = 'issue-action-status status-success';
    setTimeout(() => {
      statusEl.textContent = '';
      statusEl.className = 'issue-action-status';
    }, 3000);
  } catch (err) {
    statusEl.textContent = esc(String(err));
    statusEl.className = 'issue-action-status status-error';
    btn.disabled = false;
  }
}
```

### 5.3 `src/main.js` — Add `handleAddIssueComment`

Add this function immediately after `handleIssueStateChange`:

```javascript
async function handleAddIssueComment(idx) {
  if (!selectedRepo) return;
  const issue = issues[idx];
  const textarea = document.getElementById(`issue-comment-input-${idx}`);
  const statusEl = document.getElementById(`issue-comment-status-${idx}`);
  const detailRow = document.getElementById(`detail-issues-${idx}`);
  const btn = detailRow ? detailRow.querySelector('.btn-add-comment') : null;
  if (!textarea || !statusEl) return;

  const body = (textarea.value || '').trim();
  if (!body) {
    statusEl.textContent = 'Comment cannot be empty.';
    statusEl.className = 'issue-comment-status status-error';
    return;
  }

  if (btn) btn.disabled = true;
  statusEl.textContent = 'Posting\u2026';
  statusEl.className = 'issue-comment-status';

  try {
    await invoke('add_issue_comment', {
      owner: selectedRepo.owner,
      repo: selectedRepo.name,
      issueNumber: issue.number,
      body,
    });

    // Optimistically increment local comment count
    issues[idx] = { ...issues[idx], comments: (issues[idx].comments || 0) + 1 };

    textarea.value = '';
    statusEl.textContent = '\u2713 Comment posted';
    statusEl.className = 'issue-comment-status status-success';
    setTimeout(() => {
      statusEl.textContent = '';
      statusEl.className = 'issue-comment-status';
    }, 3000);
  } catch (err) {
    statusEl.textContent = esc(String(err));
    statusEl.className = 'issue-comment-status status-error';
  } finally {
    if (btn) btn.disabled = false;
  }
}
```

### 5.4 `src/styles.css` — Add new styles

Append at the end of the file:

```css
/* ── Issue action section ───────────────────── */

.detail-actions {
  display: flex;
  flex-direction: column;
  gap: 0.75rem;
  border-top: 1px solid var(--border);
  padding-top: 0.75rem;
}

.detail-action-group {
  display: flex;
  align-items: center;
  gap: 0.6rem;
  flex-wrap: wrap;
}

.btn-action {
  padding: 0.3rem 0.85rem;
  border-radius: var(--radius);
  border: 1px solid var(--border);
  cursor: pointer;
  font-size: 0.82rem;
  font-weight: 600;
  font-family: var(--font);
  transition: opacity 0.15s, background 0.15s;
  line-height: 1.5;
}

.btn-action:disabled {
  opacity: 0.4;
  cursor: default;
}

.btn-close-issue {
  background: rgba(248, 81, 73, 0.12);
  color: var(--red);
  border-color: rgba(248, 81, 73, 0.35);
}

.btn-close-issue:hover:not(:disabled) {
  background: rgba(248, 81, 73, 0.22);
}

.btn-reopen-issue {
  background: rgba(63, 185, 80, 0.12);
  color: var(--green);
  border-color: rgba(63, 185, 80, 0.35);
}

.btn-reopen-issue:hover:not(:disabled) {
  background: rgba(63, 185, 80, 0.22);
}

.btn-add-comment {
  background: var(--accent);
  color: #fff;
  border: none;
}

.btn-add-comment:hover:not(:disabled) {
  opacity: 0.85;
}

.issue-action-status,
.issue-comment-status {
  font-size: 0.78rem;
}

.issue-action-status.status-success,
.issue-comment-status.status-success {
  color: var(--green);
}

.issue-action-status.status-error,
.issue-comment-status.status-error {
  color: var(--red);
}

.detail-comment-form {
  display: flex;
  flex-direction: column;
  gap: 0.4rem;
}

.issue-comment-input {
  width: 100%;
  background: var(--bg);
  border: 1px solid var(--border);
  border-radius: var(--radius);
  color: var(--text);
  font-size: 0.84rem;
  font-family: var(--font);
  padding: 0.5rem 0.7rem;
  resize: vertical;
  min-height: 3.5rem;
}

.issue-comment-input:focus {
  outline: none;
  border-color: var(--accent);
}

.detail-comment-footer {
  display: flex;
  justify-content: space-between;
  align-items: center;
  gap: 0.5rem;
}

/* Widen max-height for issue detail rows to accommodate actions section */
#tab-issues .detail-row.expanded .detail-body {
  max-height: 900px;
}
```

**Note on max-height override:** The last rule uses `#tab-issues` as a scope prefix to override the general `.detail-row.expanded .detail-body { max-height: 700px; }` rule only for the issues panel, preserving the original height for pull requests and alerts. No existing styles are modified.

---

## 6. Tauri Command Registration

### 6.1 New Tauri commands in `src-tauri/src/main.rs`

Add the following three command functions to `main.rs`, after the existing `fetch_issues` command (around line 195, before `fetch_pulls`):

```rust
/// Close an open issue.
#[cfg(not(feature = "dev-mock"))]
#[tauri::command]
async fn close_issue(
    owner: String,
    repo: String,
    issue_number: u64,
    state: State<'_, Mutex<AppState>>,
) -> Result<models::Issue, String> {
    let client = {
        let app = state.lock().map_err(|e| e.to_string())?;
        app.client.clone().ok_or("Not authenticated")?
    };
    github::issues::close_issue(&client, &owner, &repo, issue_number)
        .await
        .map_err(|e| e.to_string())
}

/// Reopen a closed issue.
#[cfg(not(feature = "dev-mock"))]
#[tauri::command]
async fn reopen_issue(
    owner: String,
    repo: String,
    issue_number: u64,
    state: State<'_, Mutex<AppState>>,
) -> Result<models::Issue, String> {
    let client = {
        let app = state.lock().map_err(|e| e.to_string())?;
        app.client.clone().ok_or("Not authenticated")?
    };
    github::issues::reopen_issue(&client, &owner, &repo, issue_number)
        .await
        .map_err(|e| e.to_string())
}

/// Add a comment to an issue.
#[cfg(not(feature = "dev-mock"))]
#[tauri::command]
async fn add_issue_comment(
    owner: String,
    repo: String,
    issue_number: u64,
    body: String,
    state: State<'_, Mutex<AppState>>,
) -> Result<(), String> {
    let trimmed = body.trim().to_string();
    if trimmed.is_empty() {
        return Err("Comment body cannot be empty".to_string());
    }
    if trimmed.len() > 65_536 {
        return Err("Comment exceeds GitHub's maximum length of 65,536 characters".to_string());
    }
    let client = {
        let app = state.lock().map_err(|e| e.to_string())?;
        app.client.clone().ok_or("Not authenticated")?
    };
    github::issues::add_issue_comment(&client, &owner, &repo, issue_number, &trimmed)
        .await
        .map_err(|e| e.to_string())
}
```

### 6.2 Register commands in `generate_handler!`

In the `#[cfg(not(feature = "dev-mock"))]` `generate_handler!` block in `main()`, add the three new commands:

```rust
    #[cfg(not(feature = "dev-mock"))]
    let builder = builder.invoke_handler(tauri::generate_handler![
        get_dev_mode,
        start_device_flow,
        poll_device_flow,
        authenticate_with_pat,
        restore_session,
        list_accounts,
        add_account,
        switch_account,
        remove_account,
        logout,
        list_repos,
        fetch_issues,
        close_issue,        // NEW
        reopen_issue,       // NEW
        add_issue_comment,  // NEW
        fetch_pulls,
        fetch_security_alerts,
        get_pull_detail,
        export_data,
        get_tracked_repos,
        add_tracked_repo,
        remove_tracked_repo,
        list_all_repos,
        get_workflow_runs,
    ]);
```

---

## 7. Mock Stub Implementation

The `dev-mock` build must continue to compile without errors. Three mock stubs are required in `src-tauri/src/mock/mod.rs`.

### 7.1 Import update

The mock module already imports `Issue` from `crate::models`. No new imports are needed.

### 7.2 Mock `close_issue`

```rust
/// Mock: close an issue by returning it with state set to "closed".
#[tauri::command]
pub fn close_issue(
    _owner: String,
    _repo: String,
    issue_number: u64,
    _state: tauri::State<'_, Mutex<AppState>>,
) -> Result<Issue, String> {
    Ok(Issue {
        number: issue_number,
        title: format!("Mock issue #{}", issue_number),
        state: "Closed".to_string(),
        author: "octocat".to_string(),
        labels: vec![],
        assignees: vec![],
        created_at: dt("2025-11-01T09:15:00Z"),
        updated_at: dt("2026-03-07T12:00:00Z"),
        closed_at: Some(dt("2026-03-07T12:00:00Z")),
        html_url: format!("https://github.com/octocat/Hello-World/issues/{}", issue_number),
        body: None,
        comments: 0,
        milestone: None,
    })
}
```

### 7.3 Mock `reopen_issue`

```rust
/// Mock: reopen an issue by returning it with state set to "Open".
#[tauri::command]
pub fn reopen_issue(
    _owner: String,
    _repo: String,
    issue_number: u64,
    _state: tauri::State<'_, Mutex<AppState>>,
) -> Result<Issue, String> {
    Ok(Issue {
        number: issue_number,
        title: format!("Mock issue #{}", issue_number),
        state: "Open".to_string(),
        author: "octocat".to_string(),
        labels: vec![],
        assignees: vec![],
        created_at: dt("2025-11-01T09:15:00Z"),
        updated_at: dt("2026-03-07T12:01:00Z"),
        closed_at: None,
        html_url: format!("https://github.com/octocat/Hello-World/issues/{}", issue_number),
        body: None,
        comments: 0,
        milestone: None,
    })
}
```

### 7.4 Mock `add_issue_comment`

```rust
/// Mock: simulate posting a comment (no-op, always succeeds).
#[tauri::command]
pub fn add_issue_comment(
    _owner: String,
    _repo: String,
    _issue_number: u64,
    _body: String,
    _state: tauri::State<'_, Mutex<AppState>>,
) -> Result<(), String> {
    Ok(())
}
```

### 7.5 Register mock stubs in `generate_handler!`

In the `#[cfg(feature = "dev-mock")]` `generate_handler!` block in `main()`:

```rust
    #[cfg(feature = "dev-mock")]
    let builder = builder.invoke_handler(tauri::generate_handler![
        mock::get_dev_mode,
        mock::restore_session,
        mock::list_repos,
        mock::fetch_issues,
        mock::close_issue,        // NEW
        mock::reopen_issue,       // NEW
        mock::add_issue_comment,  // NEW
        mock::fetch_pulls,
        mock::fetch_security_alerts,
        start_device_flow,
        poll_device_flow,
        authenticate_with_pat,
        logout,
        export_data,
        mock::get_pull_detail,
        mock::get_tracked_repos,
        mock::add_tracked_repo,
        mock::remove_tracked_repo,
        mock::list_all_repos,
        mock::get_workflow_runs,
    ]);
```

---

## 8. Dependencies

**No new Cargo dependencies are required.**

All required functionality is available in the already-declared dependencies:

| Dependency | Current Version | Usage |
|---|---|---|
| `octocrab` | `0.38` | `IssueHandler::update()`, `IssueHandler::create_comment()`, `IssueState` enum |
| `anyhow` | `1` | `Context` trait for `.context(...)` error wrapping |
| `serde` | `1` | `Serialize`/`Deserialize` already on `Issue` model |
| `tokio` | `1` | Async runtime for Tauri commands (already configured) |

`Cargo.toml` requires no changes.

---

## 9. Security Considerations

### 9.1 OAuth Token Scope

The existing app already correctly requests `repo` scope:
- PAT sign-in: users are instructed to provide a token with `repo` and `security_events` scopes (visible in `index.html` login subtitle and help text).
- Device Flow: `const OAUTH_SCOPES: &str = "repo security_events"` in `auth.rs` includes `repo`.

The `repo` scope includes:
- `repo:status`, `repo_deployment`, `public_repo`, `repo:invite`, `security_events`
- Full read **and write** access to repositories including issues, comments, PRs.

No scope changes are needed.

### 9.2 Input Validation — Comment Body

Validation is performed at the Tauri command boundary (the correct place for system boundary validation):

1. **Empty check** (in `add_issue_comment` command): `if trimmed.is_empty() { return Err(...) }`
2. **Length check** (in `add_issue_comment` command): `if trimmed.len() > 65_536 { return Err(...) }` — matches GitHub API limit.
3. **Injection safety**: The `body` string is passed directly to octocrab's `create_comment()`, which serializes it as a JSON string field in the request body. There is no SQL, shell, or HTML injection risk. GitHub API receives it as a raw Markdown string stored to their database.

### 9.3 XSS Prevention in the Frontend

The comment body typed by the user is sent to GitHub via the Tauri backend. The body is NEVER rendered back into the DOM by this application (comments are not fetched back). There is no XSS risk from comment content.

Error messages from the backend (which could theoretically contain filenames, API error text, etc.) are already sanitized via the existing `esc()` helper before insertion into the DOM.

The action button labels and status messages are hardcoded strings — not user-provided input rendered into HTML.

### 9.4 Repository / Issue Owner Authorization

The GitHub API itself enforces authorization: the access token must have write access to the repository (collaborator or owner role) for state changes and comments. The API returns HTTP 403 Forbidden if the token lacks permission. Octocrab will surface this as an error through anyhow, which propagates as a `String` error to the frontend and is displayed in the `issue-action-status` / `issue-comment-status` span.

No additional application-level authorization check is required or appropriate.

### 9.5 Parameter Injection in Rust Commands

The `owner`, `repo`, and `issue_number` parameters passed to the new commands are received from the frontend and forwarded to `github::issues::*` functions. The octocrab API constructs the URL path using these values via Rust's type system (u64 for `issue_number` is inherently safe; `owner` and `repo` strings are URL-path-encoded by reqwest/octocrab internally). No custom URL construction occurs.

---

## 10. Risks and Mitigations

### 10.1 octocrab 0.38 API Surface — `update()` return type

**Risk:** The `update().send()` method in octocrab 0.38 may return `octocrab::models::issues::Issue` but the exact field types of `comments`, `created_at`, `updated_at` must match what `map_issue` expects.

**Evidence:** The existing `fetch_issues` in `issues.rs` already maps these same fields from the same type without `.unwrap_or_default()` calls, proving the types are non-optional at octocrab 0.38.

**Mitigation:** The `map_issue` helper uses the identical field access pattern as the existing `fetch_issues` mapping. If any field type mismatch exists, the Rust compiler will catch it at `cargo build` time.

### 10.2 `max-height` Overflow on Long Issue Bodies

**Risk:** Some issues may have very long Markdown bodies. The existing `.detail-body-text` has `max-height: 300px; overflow-y: auto` to limit body height. Even with the `900px` override for issues, a very dense meta grid plus action widgets might still overflow.

**Mitigation:** The `900px` override provides ~200px more space than the default 700px. The detail body text is already scrollable. If further issues arise, the height can be increased or `overflow-y: auto` added to `.detail-body` globally.

### 10.3 Stale `issues` array after state change

**Risk:** If the user performs an action and then the auto-refresh fires (e.g., they click the refresh button), the `expandedRow` state is reset to `null` (line in `refreshData`: `expandedRow = null`), requiring re-expansion to see the updated state.

**Mitigation:** This is acceptable behaviour. The state badge in the collapsed data row is surgically updated immediately after action success, so the user can see the new state without re-expanding. The comment on `expandedRow = null` in `refreshData` already documents this intent.

### 10.4 Mock `close_issue`/`reopen_issue` return stub issue vs. actual issue

**Risk:** The mock returns a minimal stub Issue, not the actual expanded mock data for the given issue number. After a mock action, `issues[idx]` in the frontend will be replaced by this stub, potentially losing rich fields like labels and assignees.

**Mitigation:** Acceptable for dev-mock builds. The stub correctly reflects the state change. A production test with a real GitHub token will exercise the real path. If more realistic mocks are desired, the mock stubs could look up the idx from a shared mock dataset — but this adds complexity not worth the gain for a dev-only mode.

### 10.5 `assign` Attribute on `AppState` mutex

**Risk:** Tauri commands that hold the mutex lock and then perform async operations (await) can deadlock. The existing pattern correctly solves this by cloning the client before releasing the lock.

**Mitigation:** All three new commands follow the same pattern as `fetch_issues`: the mutex lock is acquired, the client is cloned, the lock is immediately dropped, and then the async call is made with the cloned client. No deadlock risk.

---

## Summary of Files to Modify

| File | Change Type | Description |
|---|---|---|
| `src-tauri/src/github/issues.rs` | Modify | Add `IssueState` import, extract `map_issue` helper, add `close_issue`, `reopen_issue`, `add_issue_comment` functions |
| `src-tauri/src/main.rs` | Modify | Add 3 new Tauri command functions; register them in both `generate_handler!` blocks |
| `src-tauri/src/mock/mod.rs` | Modify | Add 3 mock stub functions |
| `src/main.js` | Modify | Update `buildIssueDetail` signature (add `idx`); update call site in `renderIssues`; add `handleIssueStateChange` and `handleAddIssueComment` functions |
| `src/styles.css` | Modify | Append new CSS for action section, buttons, textarea, status spans, and issues-scoped `max-height` override |
| `src/index.html` | **No change** | No new HTML elements needed outside of what is dynamically generated by `buildIssueDetail` |
| `src-tauri/Cargo.toml` | **No change** | No new dependencies |

**Total new lines of code (estimated):** ~180 Rust, ~120 JavaScript, ~90 CSS.
