# Expandable Detail View — Feature Specification

**Project:** GitHub Export (Tauri v1 Desktop App)  
**Feature:** Inline expandable detail rows for Issues, Pull Requests, and Security Alerts  
**Spec Author:** Research Subagent  
**Date:** 2026-03-05  

---

## Table of Contents

1. [Current State Analysis](#1-current-state-analysis)
2. [Proposed Solution Architecture](#2-proposed-solution-architecture)
3. [Backend Changes — Models](#3-backend-changes--models)
4. [Backend Changes — Data Mappers](#4-backend-changes--data-mappers)
5. [New Tauri Command — get_pull_detail](#5-new-tauri-command--get_pull_detail)
6. [Frontend — HTML Changes](#6-frontend--html-changes)
7. [Frontend — JavaScript Implementation](#7-frontend--javascript-implementation)
8. [Frontend — CSS Animations and Styles](#8-frontend--css-animations-and-styles)
9. [Markdown Rendering Approach](#9-markdown-rendering-approach)
10. [Mock Module Updates](#10-mock-module-updates)
11. [Step-by-Step Implementation](#11-step-by-step-implementation)
12. [Dependencies](#12-dependencies)
13. [Risks and Mitigations](#13-risks-and-mitigations)

---

## 1. Current State Analysis

### 1.1 Data Models (`src-tauri/src/models/mod.rs`)

#### `Issue` — fields currently present:
| Field | Type | Notes |
|---|---|---|
| `number` | `u64` | Displayed in table |
| `title` | `String` | Displayed in table |
| `state` | `String` | Displayed in table |
| `author` | `String` | Displayed in table |
| `labels` | `Vec<String>` | Displayed in table |
| `assignees` | `Vec<String>` | **Fetched, not displayed** |
| `created_at` | `DateTime<Utc>` | Displayed in table |
| `updated_at` | `DateTime<Utc>` | **Fetched, not displayed** |
| `closed_at` | `Option<DateTime<Utc>>` | **Fetched, not displayed** |
| `html_url` | `String` | Used in table link |
| `body` | `Option<String>` | **Fetched, not displayed** |
| ~~`comments`~~ | — | **MISSING** — available from list API |
| ~~`milestone`~~ | — | **MISSING** — available from list API |

#### `PullRequest` — fields currently present:
| Field | Type | Notes |
|---|---|---|
| `number` | `u64` | Displayed |
| `title` | `String` | Displayed |
| `state` | `String` | Displayed |
| `author` | `String` | Displayed |
| `labels` | `Vec<String>` | **Fetched, not displayed** |
| `reviewers` | `Vec<String>` | **Fetched, not displayed** (requested_reviewers) |
| `head_branch` | `String` | Displayed |
| `base_branch` | `String` | Displayed |
| `created_at` | `DateTime<Utc>` | **Fetched, not displayed** |
| `updated_at` | `DateTime<Utc>` | **Fetched, not displayed** |
| `merged_at` | `Option<DateTime<Utc>>` | **Fetched, not displayed** |
| `closed_at` | `Option<DateTime<Utc>>` | **Fetched, not displayed** |
| `html_url` | `String` | Used in table link |
| `draft` | `bool` | Displayed |
| `body` | `Option<String>` | **Fetched, not displayed** |
| ~~`assignees`~~ | — | **MISSING** — available from list API |
| ~~`additions`~~ | — | **MISSING** — individual PR API only |
| ~~`deletions`~~ | — | **MISSING** — individual PR API only |
| ~~`changed_files`~~ | — | **MISSING** — individual PR API only |
| ~~`mergeable`~~ | — | **MISSING** — individual PR API only |

#### `SecurityAlert` — fields currently present:
| Field | Type | Notes |
|---|---|---|
| `id` | `u64` | Displayed |
| `severity` | `String` | Displayed |
| `summary` | `String` | Displayed |
| `description` | `String` | **Fetched, not displayed** |
| `package_name` | `Option<String>` | Displayed |
| `vulnerable_version_range` | `Option<String>` | Displayed |
| `patched_version` | `Option<String>` | Displayed |
| `state` | `String` | **Fetched, not displayed** |
| `html_url` | `String` | Used as link |
| `created_at` | `DateTime<Utc>` | **Fetched, not displayed** |
| `alert_type` | `String` | Displayed |
| `tool_name` | `Option<String>` | Displayed (code scanning) |
| `location_path` | `Option<String>` | **Fetched, not displayed** |
| ~~`cve_id`~~ | — | **MISSING** — in `security_advisory` from API |
| ~~`cvss_score`~~ | — | **MISSING** — in `security_advisory.cvss` from API |
| ~~`cwes`~~ | — | **MISSING** — in `security_advisory.cwes` from API |
| ~~`dismissed_reason`~~ | — | **MISSING** — top-level field on Dependabot alert |
| ~~`dismissed_comment`~~ | — | **MISSING** — top-level field on Dependabot alert |

### 1.2 Existing Tauri Commands

| Command | Parameters | Returns |
|---|---|---|
| `fetch_issues` | owner, repo, filters | `Vec<Issue>` |
| `fetch_pulls` | owner, repo, filters | `Vec<PullRequest>` |
| `fetch_security_alerts` | owner, repo, state | `Vec<SecurityAlert>` |
| `export_data` | format, issues, pulls, alerts, file_path | `String` |

### 1.3 Frontend Rendering Pattern

All three render functions (`renderIssues`, `renderPulls`, `renderAlerts`) follow this pattern:

```javascript
function renderIssues() {
  const tbody = $("#issues-table tbody");
  tbody.innerHTML = issues
    .map((i, idx) => `<tr data-idx="${idx}">
        <td>${i.number}</td>
        ...
      </tr>`)
    .join("");
}
```

- Data is loaded once into module-level arrays (`issues`, `pulls`, `alerts`)
- Rendering uses `innerHTML` with template literals
- `esc()` is used for all user-supplied text values (XSS prevention)
- Tables are inside `.tab-panel` divs; only the active panel is visible
- No virtual DOM, no framework — direct DOM manipulation

### 1.4 HTML Structure

Three separate `<table>` elements:
- `#issues-table` inside `#tab-issues`
- `#pulls-table` inside `#tab-pulls`
- `#alerts-table` inside `#tab-alerts`

Each table has a `<thead>` with column headers and a `<tbody>` that is fully replaced on each render.

### 1.5 Tauri / IPC Pattern

```javascript
// JS calls Rust via:
const result = await invoke("command_name", { param1: value1 });

// Rust exposes via:
#[tauri::command]
async fn command_name(param1: String, state: State<'_, Mutex<AppState>>) -> Result<T, String>
```

### 1.6 Content Security Policy

`tauri.conf.json` sets `"csp": null` — **no CSP restrictions**. Self-hosted JS libraries can be loaded without CSP configuration changes.

---

## 2. Proposed Solution Architecture

### 2.1 UX Design

- **Click-to-expand**: Clicking any data row expands an inline detail panel directly below it
- **Click-to-collapse**: Clicking the same row again collapses the panel; a close button (×) also collapses
- **One-at-a-time**: Only one row expanded at a time across all tabs (collapsing previous row on new expand)
- **Smooth animation**: CSS `max-height` + `opacity` transition, 300ms ease
- **Immediate display**: Detail panel renders instantly from already-loaded in-memory data
- **Lazy-loaded PR stats**: For PRs only, `additions`/`deletions`/`changed_files`/`mergeable` are fetched asynchronously via `get_pull_detail` when the row is first expanded; a mini-spinner is shown in those slots

### 2.2 Data Strategy

| Data Type | Strategy |
|---|---|
| Issue detail | **No new API call** — all needed data already in `issues[]` array; add `comments` + `milestone` to model so they're populated during `fetch_issues` |
| PR detail (basic) | **No new API call** — body, branches, draft, labels, reviewers in `pulls[]` array; add `assignees` to model |
| PR detail (stats) | **Lazy fetch** — `get_pull_detail` invoked on expand; `additions`, `deletions`, `changed_files`, `mergeable` come from individual PR endpoint |
| Security alert detail | **No new API call** — extend `SecurityAlert` model with CVE/CVSS/CWEs/dismissed fields; all sourced from existing Dependabot list API response |

### 2.3 Rendering Architecture

Detail rows are inserted into the `<tbody>` alongside data rows. Each data row gets a companion hidden detail row immediately below it:

```
<tbody>
  <tr class="data-row" data-idx="0">...</tr>          <!-- Issue #42 -->
  <tr class="detail-row" id="detail-issue-0">          <!-- Detail panel -->
    <td colspan="6">
      <div class="detail-body">...</div>
    </td>
  </tr>
  <tr class="data-row" data-idx="1">...</tr>          <!-- Issue #57 -->
  <tr class="detail-row" id="detail-issue-1">
    <td colspan="6">
      <div class="detail-body">...</div>
    </td>
  </tr>
</tbody>
```

The detail rows are rendered as part of the initial `renderIssues()`/`renderPulls()`/`renderAlerts()` calls, but with `max-height: 0` (collapsed). Clicking a data row toggles the expanded class on the corresponding detail row.

### 2.4 State Management

```javascript
// Add to top-level state:
let expandedRow = null; // { type: "issues"|"pulls"|"alerts", idx: number } | null
```

---

## 3. Backend Changes — Models

**File:** `src-tauri/src/models/mod.rs`

### 3.1 Updated `Issue` struct

Add two fields:
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
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
    // NEW:
    pub comments: u32,
    pub milestone: Option<String>,
}
```

### 3.2 Updated `PullRequest` struct

Add one field:
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PullRequest {
    pub number: u64,
    pub title: String,
    pub state: String,
    pub author: String,
    pub labels: Vec<String>,
    pub assignees: Vec<String>,  // NEW
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
```

### 3.3 Updated `SecurityAlert` struct

Add five fields:
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
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
    pub alert_type: String,
    pub tool_name: Option<String>,
    pub location_path: Option<String>,
    // NEW:
    pub cve_id: Option<String>,
    pub cvss_score: Option<f64>,
    pub cwes: Vec<String>,
    pub dismissed_reason: Option<String>,
    pub dismissed_comment: Option<String>,
}
```

### 3.4 New `PullDetail` struct

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PullDetail {
    pub number: u64,
    pub additions: u64,
    pub deletions: u64,
    pub changed_files: u64,
    pub mergeable: Option<bool>,
    pub mergeable_state: Option<String>,
}
```

---

## 4. Backend Changes — Data Mappers

### 4.1 `src-tauri/src/github/issues.rs`

In `fetch_issues()`, the octocrab `Issue` type includes `comments: u32` and `milestone: Option<Milestone>`. Map them:

```rust
.map(|i| Issue {
    // ... existing fields ...
    body: i.body,
    // NEW mappings:
    comments: i.comments,
    milestone: i.milestone.as_ref().map(|m| m.title.clone()),
})
```

### 4.2 `src-tauri/src/github/pulls.rs`

In `fetch_pulls()`, the octocrab PR list response includes `assignees: Option<Vec<User>>`. Map it:

```rust
.map(|pr| PullRequest {
    // ... existing fields ...
    body: pr.body,
    // NEW mapping:
    assignees: pr
        .assignees
        .unwrap_or_default()
        .iter()
        .map(|a| a.login.clone())
        .collect(),
})
```

### 4.3 `src-tauri/src/github/security.rs`

Extend `RawAdvisory` and `RawDependabotAlert` to capture advisory extended fields:

```rust
#[derive(Debug, Deserialize)]
struct RawCvss {
    score: Option<f64>,
}

#[derive(Debug, Deserialize)]
struct RawCwe {
    cwe_id: Option<String>,
}

// Extended RawAdvisory:
#[derive(Debug, Deserialize)]
struct RawAdvisory {
    summary: Option<String>,
    description: Option<String>,
    severity: Option<String>,
    // NEW:
    cve_id: Option<String>,
    cvss: Option<RawCvss>,
    cwes: Option<Vec<RawCwe>>,
}

// Extended RawDependabotAlert:
#[derive(Debug, Deserialize)]
struct RawDependabotAlert {
    number: u64,
    state: String,
    html_url: String,
    created_at: String,
    security_advisory: Option<RawAdvisory>,
    security_vulnerability: Option<RawVulnerability>,
    // NEW:
    dismissed_reason: Option<String>,
    dismissed_comment: Option<String>,
}
```

In the mapping closure, populate the new `SecurityAlert` fields:

```rust
SecurityAlert {
    // ... existing fields ...
    // NEW:
    cve_id: advisory.and_then(|ad| ad.cve_id.clone()),
    cvss_score: advisory.and_then(|ad| ad.cvss.as_ref().and_then(|c| c.score)),
    cwes: advisory
        .and_then(|ad| ad.cwes.as_ref())
        .map(|cwes| cwes.iter().filter_map(|c| c.cwe_id.clone()).collect())
        .unwrap_or_default(),
    dismissed_reason: a.dismissed_reason,
    dismissed_comment: a.dismissed_comment,
}
```

> **Note on borrow ordering**: `advisory` above is `a.security_advisory.as_ref()`. To use it multiple times in the struct literal, assign it to a local variable before the mapping closure, analogous to how `label_vec` is currently pre-computed in `fetch_issues`.

---

## 5. New Tauri Command — `get_pull_detail`

### 5.1 Create `src-tauri/src/github/detail.rs`

```rust
use anyhow::{Context, Result};
use octocrab::Octocrab;

use crate::models::PullDetail;

/// Fetches detailed statistics for a single pull request.
/// The list endpoint does not return additions/deletions/changed_files/mergeable;
/// those require an individual GET to /repos/{owner}/{repo}/pulls/{pull_number}.
pub async fn fetch_pull_detail(
    client: &Octocrab,
    owner: &str,
    repo: &str,
    pull_number: u64,
) -> Result<PullDetail> {
    let pr = client
        .pulls(owner, repo)
        .get(pull_number)
        .await
        .context("Failed to fetch pull request detail")?;

    Ok(PullDetail {
        number: pull_number,
        additions: pr.additions.unwrap_or(0),
        deletions: pr.deletions.unwrap_or(0),
        changed_files: pr.changed_files.unwrap_or(0),
        mergeable: pr.mergeable,
        mergeable_state: pr.mergeable_state.map(|s| format!("{:?}", s)),
    })
}
```

### 5.2 Register the module in `src-tauri/src/github/mod.rs`

Add `pub mod detail;` to the existing module declarations.

### 5.3 Add Tauri command in `src-tauri/src/main.rs`

```rust
#[cfg(not(feature = "dev-mock"))]
#[tauri::command]
async fn get_pull_detail(
    owner: String,
    repo: String,
    pull_number: u64,
    state: State<'_, Mutex<AppState>>,
) -> Result<models::PullDetail, String> {
    let client = {
        let app = state.lock().map_err(|e| e.to_string())?;
        app.client.clone().ok_or("Not authenticated")?
    };
    github::detail::fetch_pull_detail(&client, &owner, &repo, pull_number)
        .await
        .map_err(|e| e.to_string())
}
```

Register in `tauri::generate_handler![]`:
```rust
// In the non-dev-mock handler list, add:
get_pull_detail,
```

### 5.4 Add `github/mod.rs` declaration

In `src-tauri/src/github/mod.rs`, add:
```rust
pub mod detail;
```

---

## 6. Frontend — HTML Changes

**File:** `src/index.html`

### 6.1 Add marked.js and DOMPurify scripts

Before the closing `</body>` tag (before `main.js`):
```html
<script src="lib/marked.min.js"></script>
<script src="lib/purify.min.js"></script>
<script src="main.js"></script>
```

### 6.2 Table headers — no structural changes needed

The `<table>` structures in `index.html` do not need changes. The tbody content (including detail rows) is fully managed by JavaScript render functions.

---

## 7. Frontend — JavaScript Implementation

**File:** `src/main.js`

### 7.1 State variable addition

```javascript
// Add to the "── State ──" section alongside existing state variables:
let expandedRow = null; // { type: "issues"|"pulls"|"alerts", idx: number } | null
```

### 7.2 Markdown rendering helper

```javascript
// ── Markdown rendering ──────────────────────────
function renderMarkdown(text) {
  if (!text) return '<em class="detail-no-body">No description provided.</em>';
  // DOMPurify sanitizes the HTML output of marked to prevent XSS.
  // marked.parse converts GitHub-flavored markdown to HTML.
  const rawHtml = marked.parse(text, { breaks: true, gfm: true });
  return DOMPurify.sanitize(rawHtml);
}
```

### 7.3 Detail panel HTML builders

#### Issue detail builder

```javascript
function buildIssueDetail(issue) {
  const assignees = issue.assignees.length
    ? issue.assignees.map(esc).join(", ")
    : "—";
  const labels = issue.labels.length
    ? labelBadges(issue.labels)
    : "—";
  const milestone = issue.milestone ? esc(issue.milestone) : "—";
  const comments = issue.comments ?? "—";
  const updatedDate = shortDate(issue.updated_at);
  const closedDate = issue.closed_at ? shortDate(issue.closed_at) : null;

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
          <span>${updatedDate}</span>
        </div>
        ${closedDate ? `
        <div class="detail-meta-item">
          <span class="detail-meta-label">Closed</span>
          <span>${closedDate}</span>
        </div>` : ""}
      </div>
      <div class="detail-footer">
        <a href="${issue.html_url}" target="_blank" class="detail-open-link">Open on GitHub ↗</a>
      </div>
    </div>`;
}
```

#### Pull request detail builder

```javascript
function buildPullDetail(pull) {
  const assignees = pull.assignees.length
    ? pull.assignees.map(esc).join(", ")
    : "—";
  const reviewers = pull.reviewers.length
    ? pull.reviewers.map(esc).join(", ")
    : "—";
  const labels = pull.labels.length ? labelBadges(pull.labels) : "—";
  const mergedDate = pull.merged_at ? shortDate(pull.merged_at) : null;
  const closedDate = pull.closed_at && !pull.merged_at ? shortDate(pull.closed_at) : null;

  // Stats are loaded lazily; rendered with a placeholder initially
  const statsId = `pull-stats-${pull.number}`;

  return `
    <div class="detail-content">
      <div class="detail-header">
        <span class="detail-type-badge">PR #${pull.number}</span>
        ${stateBadge(pull.state)}
        ${pull.draft ? '<span class="badge badge-draft">draft</span>' : ""}
        <button class="detail-close-btn" onclick="collapseAllRows()" title="Close">×</button>
      </div>
      <div class="detail-body-text markdown-body">${renderMarkdown(pull.body)}</div>
      <div class="detail-meta-grid">
        <div class="detail-meta-item">
          <span class="detail-meta-label">Author</span>
          <span>${esc(pull.author)}</span>
        </div>
        <div class="detail-meta-item">
          <span class="detail-meta-label">Assignees</span>
          <span>${assignees}</span>
        </div>
        <div class="detail-meta-item">
          <span class="detail-meta-label">Reviewers</span>
          <span>${reviewers}</span>
        </div>
        <div class="detail-meta-item">
          <span class="detail-meta-label">Labels</span>
          <span>${labels}</span>
        </div>
        <div class="detail-meta-item">
          <span class="detail-meta-label">Head → Base</span>
          <span>${esc(pull.head_branch)} → ${esc(pull.base_branch)}</span>
        </div>
        <div class="detail-meta-item">
          <span class="detail-meta-label">Created</span>
          <span>${shortDate(pull.created_at)}</span>
        </div>
        <div class="detail-meta-item">
          <span class="detail-meta-label">Updated</span>
          <span>${shortDate(pull.updated_at)}</span>
        </div>
        ${mergedDate ? `
        <div class="detail-meta-item">
          <span class="detail-meta-label">Merged</span>
          <span>${mergedDate}</span>
        </div>` : ""}
        ${closedDate ? `
        <div class="detail-meta-item">
          <span class="detail-meta-label">Closed</span>
          <span>${closedDate}</span>
        </div>` : ""}
      </div>
      <div id="${statsId}" class="detail-pr-stats">
        <div class="detail-pr-stats-loading">
          <span class="spinner-small"></span> Loading diff stats…
        </div>
      </div>
      <div class="detail-footer">
        <a href="${pull.html_url}" target="_blank" class="detail-open-link">Open on GitHub ↗</a>
      </div>
    </div>`;
}
```

#### Security alert detail builder

```javascript
function buildAlertDetail(alert) {
  const normalizedSev = normalizeSeverity(alert.severity, alert.alert_type);
  const sevClass = normalizedSev === "critical" ? "severity-critical"
                 : normalizedSev === "high"     ? "severity-high"
                 : normalizedSev === "medium"   ? "severity-medium"
                 :                               "severity-low";

  const cvss = alert.cvss_score != null
    ? `<span class="detail-cvss-score">${alert.cvss_score.toFixed(1)}</span>`
    : "—";
  const cve = alert.cve_id ? esc(alert.cve_id) : "—";
  const cwes = alert.cwes.length ? alert.cwes.map(esc).join(", ") : "—";
  const location = alert.location_path ? esc(alert.location_path) : null;
  const tool = alert.tool_name ? esc(alert.tool_name) : null;
  const typeLabel = alert.alert_type === "code_scanning"
    ? `Code Scanning${tool ? ` (${tool})` : ""}`
    : "Dependabot";
  const dismissedReason = alert.dismissed_reason ? esc(alert.dismissed_reason) : null;
  const dismissedComment = alert.dismissed_comment ? esc(alert.dismissed_comment) : null;

  return `
    <div class="detail-content">
      <div class="detail-header">
        <span class="detail-type-badge">${typeLabel} #${alert.id}</span>
        <span class="badge badge-severity ${sevClass}">${esc(alert.severity)}</span>
        <button class="detail-close-btn" onclick="collapseAllRows()" title="Close">×</button>
      </div>
      <div class="detail-body-text detail-advisory-description">${esc(alert.description) || '<em class="detail-no-body">No advisory description available.</em>'}</div>
      <div class="detail-meta-grid">
        <div class="detail-meta-item">
          <span class="detail-meta-label">CVE ID</span>
          <span>${cve}</span>
        </div>
        <div class="detail-meta-item">
          <span class="detail-meta-label">CVSS Score</span>
          <span>${cvss}</span>
        </div>
        <div class="detail-meta-item">
          <span class="detail-meta-label">CWEs</span>
          <span>${cwes}</span>
        </div>
        <div class="detail-meta-item">
          <span class="detail-meta-label">Package</span>
          <span>${esc(alert.package_name || "—")}</span>
        </div>
        <div class="detail-meta-item">
          <span class="detail-meta-label">Vulnerable</span>
          <span>${esc(alert.vulnerable_version_range || "—")}</span>
        </div>
        <div class="detail-meta-item">
          <span class="detail-meta-label">Patched</span>
          <span>${esc(alert.patched_version || "—")}</span>
        </div>
        <div class="detail-meta-item">
          <span class="detail-meta-label">State</span>
          <span>${esc(alert.state)}</span>
        </div>
        <div class="detail-meta-item">
          <span class="detail-meta-label">Created</span>
          <span>${shortDate(alert.created_at)}</span>
        </div>
        ${location ? `
        <div class="detail-meta-item detail-meta-full">
          <span class="detail-meta-label">Location</span>
          <code>${location}</code>
        </div>` : ""}
        ${dismissedReason ? `
        <div class="detail-meta-item">
          <span class="detail-meta-label">Dismissed Reason</span>
          <span>${dismissedReason}</span>
        </div>` : ""}
        ${dismissedComment ? `
        <div class="detail-meta-item detail-meta-full">
          <span class="detail-meta-label">Dismissed Comment</span>
          <span>${dismissedComment}</span>
        </div>` : ""}
      </div>
      <div class="detail-footer">
        <a href="${alert.html_url}" target="_blank" class="detail-open-link">Open on GitHub ↗</a>
      </div>
    </div>`;
}
```

### 7.4 Toggle and collapse logic

```javascript
// ── Expandable row logic ────────────────────────
function collapseAllRows() {
  document.querySelectorAll(".detail-row.expanded").forEach((row) => {
    row.classList.remove("expanded");
  });
  expandedRow = null;
}

async function toggleDetailRow(type, idx) {
  const rowId = `detail-${type}-${idx}`;
  const detailRow = document.getElementById(rowId);
  if (!detailRow) return;

  const isExpanded = detailRow.classList.contains("expanded");

  // Collapse any currently open row
  collapseAllRows();

  if (!isExpanded) {
    // Expand the clicked row
    detailRow.classList.add("expanded");
    expandedRow = { type, idx };

    // For PRs: lazily fetch diff stats
    if (type === "pulls" && selectedRepo) {
      const pull = pulls[idx];
      const statsEl = document.getElementById(`pull-stats-${pull.number}`);
      if (statsEl && statsEl.querySelector(".detail-pr-stats-loading")) {
        try {
          const detail = await invoke("get_pull_detail", {
            owner: selectedRepo.owner,
            repo: selectedRepo.name,
            pullNumber: pull.number,
          });
          statsEl.innerHTML = `
            <div class="detail-pr-stats-row">
              <span class="stat-additions">+${detail.additions} additions</span>
              <span class="stat-deletions">−${detail.deletions} deletions</span>
              <span class="stat-files">${detail.changed_files} file${detail.changed_files !== 1 ? "s" : ""} changed</span>
              ${detail.mergeable != null ? `<span class="stat-mergeable ${detail.mergeable ? "mergeable-yes" : "mergeable-no"}">
                ${detail.mergeable ? "✓ Mergeable" : "✗ Not mergeable"}
              </span>` : ""}
            </div>`;
        } catch (e) {
          statsEl.innerHTML = `<span class="detail-stats-error">Could not load diff stats: ${esc(String(e))}</span>`;
        }
      }
    }
  }
}
```

### 7.5 Updated render functions

#### Updated `renderIssues()`

```javascript
function renderIssues() {
  const tbody = $("#issues-table tbody");
  if (issuesError) {
    tbody.innerHTML = `<tr><td colspan="6" class="fetch-error">Failed to load issues: ${esc(issuesError)}</td></tr>`;
    return;
  }
  tbody.innerHTML = issues
    .map((i, idx) => `
      <tr class="data-row clickable-row" data-idx="${idx}" onclick="toggleDetailRow('issues', ${idx})">
        <td>${i.number}</td>
        <td>${esc(i.title)}</td>
        <td>${stateBadge(i.state)}</td>
        <td>${esc(i.author)}</td>
        <td>${labelBadges(i.labels)}</td>
        <td>${shortDate(i.created_at)}</td>
      </tr>
      <tr class="detail-row" id="detail-issues-${idx}">
        <td colspan="6">
          <div class="detail-body">${buildIssueDetail(i)}</div>
        </td>
      </tr>`)
    .join("");
}
```

> **Note:** The existing `<a href="...">` link on the title column is **removed** in the data row — clicking any part of the row now expands the detail panel, and the "Open on GitHub ↗" link is inside the detail panel. This avoids click event conflicts.

#### Updated `renderPulls()`

```javascript
function renderPulls() {
  const tbody = $("#pulls-table tbody");
  if (pullsError) {
    tbody.innerHTML = `<tr><td colspan="6" class="fetch-error">Failed to load pull requests: ${esc(pullsError)}</td></tr>`;
    return;
  }
  tbody.innerHTML = pulls
    .map((p, idx) => `
      <tr class="data-row clickable-row" data-idx="${idx}" onclick="toggleDetailRow('pulls', ${idx})">
        <td>${p.number}</td>
        <td>${esc(p.title)}</td>
        <td>${stateBadge(p.state)}</td>
        <td>${esc(p.author)}</td>
        <td>${esc(p.head_branch)} → ${esc(p.base_branch)}</td>
        <td>${p.draft ? "✓" : ""}</td>
      </tr>
      <tr class="detail-row" id="detail-pulls-${idx}">
        <td colspan="6">
          <div class="detail-body">${buildPullDetail(p)}</div>
        </td>
      </tr>`)
    .join("");
}
```

#### Updated `renderAlerts()`

The complex error/empty handling in `renderAlerts()` is unchanged; only the happy-path map is updated:

```javascript
// Inside renderAlerts(), replace the final tbody.innerHTML = alerts.map(...).join("") block:
tbody.innerHTML = alerts
  .map((a, idx) => {
    const normalizedSev = normalizeSeverity(a.severity, a.alert_type);
    const cls = normalizedSev === "critical" ? "severity-critical"
              : normalizedSev === "high"     ? "severity-high"
              : normalizedSev === "medium"   ? "severity-medium"
              :                               "severity-low";
    const typeLabel = a.alert_type === "code_scanning"
      ? `Code Scanning${a.tool_name ? ` (${esc(a.tool_name)})` : ""}`
      : "Dependabot";
    return `
      <tr class="data-row clickable-row" data-idx="${idx}" onclick="toggleDetailRow('alerts', ${idx})">
        <td>${a.id}</td>
        <td>${typeLabel}</td>
        <td class="${cls}">${esc(a.severity)}</td>
        <td>${esc(a.summary)}</td>
        <td>${esc(a.package_name || "—")}</td>
        <td>${esc(a.vulnerable_version_range || "—")}</td>
        <td>${esc(a.patched_version || "—")}</td>
      </tr>
      <tr class="detail-row" id="detail-alerts-${idx}">
        <td colspan="7">
          <div class="detail-body">${buildAlertDetail(a)}</div>
        </td>
      </tr>`;
  })
  .join("");
```

### 7.6 Collapse on data refresh

In `refreshData()`, after calling the three render functions, add:
```javascript
expandedRow = null; // Reset expanded state on data refresh
```

---

## 8. Frontend — CSS Animations and Styles

**File:** `src/styles.css`

Append the following ruleset blocks:

### 8.1 Clickable rows

```css
/* ── Expandable detail rows ──────────────────── */
.clickable-row { cursor: pointer; user-select: none; }
.clickable-row:hover { background: rgba(88,166,255,0.08); }
.clickable-row.row-expanded { background: rgba(88,166,255,0.12); }
```

### 8.2 Detail row and animation

```css
.detail-row td {
  padding: 0;
  border-bottom: none;
  /* No hover highlight on detail rows */
  background: var(--surface);
}

.detail-row:not(.expanded) td { border-bottom: 1px solid var(--border); }
.detail-row.expanded td { border-bottom: 2px solid var(--accent); }

.detail-body {
  max-height: 0;
  overflow: hidden;
  opacity: 0;
  transition: max-height 0.32s ease, opacity 0.25s ease, padding 0.32s ease;
  padding: 0 0.8rem;
  background: #0d1117;
}

.detail-row.expanded .detail-body {
  max-height: 700px;
  opacity: 1;
  padding: 1.25rem 0.8rem;
}
```

### 8.3 Detail content layout

```css
.detail-content {
  display: flex;
  flex-direction: column;
  gap: 1rem;
}

.detail-header {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  flex-wrap: wrap;
}

.detail-type-badge {
  font-size: 0.78rem;
  font-weight: 700;
  color: var(--text-muted);
  text-transform: uppercase;
  letter-spacing: 0.05em;
}

.detail-close-btn {
  margin-left: auto;
  background: none;
  border: 1px solid var(--border);
  color: var(--text-muted);
  border-radius: 50%;
  width: 24px;
  height: 24px;
  font-size: 1rem;
  line-height: 1;
  cursor: pointer;
  transition: color 0.15s, border-color 0.15s;
  display: flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
}

.detail-close-btn:hover {
  color: var(--red);
  border-color: var(--red);
}
```

### 8.4 Markdown body styles

```css
.detail-body-text {
  font-size: 0.875rem;
  line-height: 1.7;
  color: var(--text);
  border-left: 3px solid var(--border);
  padding-left: 0.8rem;
  max-height: 300px;
  overflow-y: auto;
}

.detail-no-body {
  color: var(--text-muted);
  font-style: italic;
}

/* Markdown body content styles */
.markdown-body h1, .markdown-body h2, .markdown-body h3 {
  font-size: 1em;
  font-weight: 700;
  margin: 0.75em 0 0.35em;
  color: var(--text);
}

.markdown-body p { margin: 0 0 0.6em; }
.markdown-body ul, .markdown-body ol { padding-left: 1.4em; margin: 0.4em 0; }
.markdown-body li { margin: 0.2em 0; }

.markdown-body code {
  background: rgba(255,255,255,0.07);
  padding: 1px 5px;
  border-radius: 3px;
  font-size: 0.85em;
}

.markdown-body pre {
  background: rgba(255,255,255,0.05);
  padding: 0.75rem;
  border-radius: var(--radius);
  overflow-x: auto;
  margin: 0.5em 0;
}

.markdown-body pre code { background: none; padding: 0; }

.markdown-body blockquote {
  border-left: 3px solid var(--border);
  padding-left: 0.8em;
  color: var(--text-muted);
  margin: 0.5em 0;
}

.markdown-body a { color: var(--accent); }
.markdown-body hr { border: none; border-top: 1px solid var(--border); margin: 0.75em 0; }

.detail-advisory-description {
  white-space: pre-wrap;
  word-break: break-word;
}
```

### 8.5 Metadata grid

```css
.detail-meta-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(200px, 1fr));
  gap: 0.5rem 1rem;
  font-size: 0.82rem;
}

.detail-meta-item {
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.detail-meta-full {
  grid-column: 1 / -1;
}

.detail-meta-label {
  color: var(--text-muted);
  font-size: 0.73rem;
  text-transform: uppercase;
  letter-spacing: 0.04em;
  font-weight: 600;
}
```

### 8.6 PR diff stats

```css
.detail-pr-stats {
  font-size: 0.84rem;
}

.detail-pr-stats-loading {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  color: var(--text-muted);
}

.spinner-small {
  width: 12px;
  height: 12px;
  border: 2px solid var(--border);
  border-top-color: var(--accent);
  border-radius: 50%;
  animation: spin 0.8s linear infinite;
  display: inline-block;
  flex-shrink: 0;
}

.detail-pr-stats-row {
  display: flex;
  gap: 1rem;
  flex-wrap: wrap;
  align-items: center;
}

.stat-additions { color: var(--green); font-weight: 600; }
.stat-deletions { color: var(--red); font-weight: 600; }
.stat-files { color: var(--text-muted); }
.mergeable-yes { color: var(--green); }
.mergeable-no  { color: var(--red); }
.detail-stats-error { color: var(--red); font-size: 0.8rem; }
.detail-cvss-score  { font-weight: 700; }
```

### 8.7 Footer

```css
.detail-footer {
  text-align: right;
}

.detail-open-link {
  font-size: 0.82rem;
  color: var(--accent);
  text-decoration: none;
  font-weight: 500;
}

.detail-open-link:hover { text-decoration: underline; }
```

### 8.8 Draft badge

```css
.badge-draft { background: rgba(139,148,158,0.2); color: var(--text-muted); }
```

---

## 9. Markdown Rendering Approach

### 9.1 Library choice

**marked.js v9+ + DOMPurify v3**

| Criterion | Rationale |
|---|---|
| No bundler | Both libraries ship as self-contained UMD bundles; a single `<script>` tag is enough |
| Offline operation | Self-hosted in `src/lib/` — no CDN dependency; works with Tauri's `custom-protocol` |
| GitHub Flavored Markdown | marked.js supports GFM (tables, task lists, strikethrough) out of the box |
| Security | DOMPurify strips dangerous HTML from marked's output before `innerHTML` assignment |
| Bundle size | marked.min.js ≈ 47 KB, purify.min.js ≈ 45 KB — acceptable for a desktop app |
| License | Both MIT licensed |

### 9.2 Integration method

1. Download `marked.min.js` from `https://cdn.jsdelivr.net/npm/marked/marked.min.js` and save to `src/lib/marked.min.js`
2. Download `purify.min.js` from `https://cdn.jsdelivr.net/npm/dompurify/dist/purify.min.js` and save to `src/lib/purify.min.js`
3. Reference both with local `<script>` tags in `index.html` before `main.js`

### 9.3 Security notes

- DOMPurify's default configuration strips all event handlers (`onclick`, `onerror`, etc.), `<script>` tags, and dangerous attributes
- Even with `csp: null`, this provides runtime XSS prevention within the Tauri webview
- Do NOT use `marked`'s deprecated `sanitize: true` option (removed in v5) — only use DOMPurify
- All other user-supplied text (author, labels, titles) continues to use the existing `esc()` function

---

## 10. Mock Module Updates

**File:** `src-tauri/src/mock/mod.rs`

### 10.1 Updated mock issues

Add `comments` and `milestone` to each mock `Issue`:
```rust
// Issue #42:
comments: 7,
milestone: Some("v2.0".to_string()),

// Issue #57:
comments: 3,
milestone: None,

// Issues #63, #71, #78:
comments: 1,
milestone: None,
```

### 10.2 Updated mock PRs

Add `assignees` to each mock `PullRequest`:
```rust
// PR #101:
assignees: vec!["hubot".to_string()],

// PR #115:
assignees: vec!["octocat".to_string()],

// PR #122:
assignees: vec![],
```

### 10.3 Updated mock security alerts

Add extended fields to each mock `SecurityAlert`:
```rust
// Alert #1 (lodash):
cve_id: Some("CVE-2021-23337".to_string()),
cvss_score: Some(8.1),
cwes: vec!["CWE-78".to_string()],
dismissed_reason: None,
dismissed_comment: None,

// Alert #2 (follow-redirects):
cve_id: Some("CVE-2024-28849".to_string()),
cvss_score: Some(6.5),
cwes: vec!["CWE-601".to_string()],
dismissed_reason: None,
dismissed_comment: None,

// Alert #3 (CodeQL SQL injection):
cve_id: None,
cvss_score: None,
cwes: vec!["CWE-89".to_string()],
dismissed_reason: None,
dismissed_comment: None,
```

### 10.4 Mock `get_pull_detail` command

```rust
#[tauri::command]
pub fn get_pull_detail(
    _owner: String,
    _repo: String,
    pull_number: u64,
    _state: tauri::State<'_, Mutex<AppState>>,
) -> Result<crate::models::PullDetail, String> {
    // Return mock stats based on pull number
    let (additions, deletions, changed_files, mergeable) = match pull_number {
        101 => (320, 85, 12, Some(false)), // merged, so not mergeable
        115 => (47, 9, 3, Some(true)),
        122 => (128, 44, 8, Some(false)), // draft, conflicts
        _   => (10, 2, 1, Some(true)),
    };
    Ok(crate::models::PullDetail {
        number: pull_number,
        additions,
        deletions,
        changed_files,
        mergeable,
        mergeable_state: if mergeable == Some(true) {
            Some("clean".to_string())
        } else {
            Some("dirty".to_string())
        },
    })
}
```

Also add `get_pull_detail` to the dev-mock handler list in `main.rs`:
```rust
#[cfg(feature = "dev-mock")]
let builder = builder.invoke_handler(tauri::generate_handler![
    // ... existing handlers ...
    mock::get_pull_detail,
]);
```

---

## 11. Step-by-Step Implementation

### Phase A — Rust Backend

**Step A1:** Update `src-tauri/src/models/mod.rs`
- Add `comments: u32` and `milestone: Option<String>` to `Issue`
- Add `assignees: Vec<String>` to `PullRequest`
- Add `cve_id`, `cvss_score`, `cwes`, `dismissed_reason`, `dismissed_comment` to `SecurityAlert`
- Add new `PullDetail` struct

**Step A2:** Update `src-tauri/src/github/issues.rs`
- Map `i.comments` and `i.milestone.as_ref().map(|m| m.title.clone())` in the Issue mapper

**Step A3:** Update `src-tauri/src/github/pulls.rs`
- Map `pr.assignees.unwrap_or_default()` in the PR mapper

**Step A4:** Update `src-tauri/src/github/security.rs`
- Extend `RawAdvisory` with `cve_id`, `cvss: Option<RawCvss>`, `cwes: Option<Vec<RawCwe>>`
- Add `RawCvss { score: Option<f64> }` and `RawCwe { cwe_id: Option<String> }` structs
- Extend `RawDependabotAlert` with `dismissed_reason: Option<String>`, `dismissed_comment: Option<String>`
- Map new fields in the Dependabot alert mapping closure

**Step A5:** Create `src-tauri/src/github/detail.rs`
- Implement `fetch_pull_detail(client, owner, repo, pull_number) -> Result<PullDetail>`

**Step A6:** Update `src-tauri/src/github/mod.rs`
- Add `pub mod detail;`

**Step A7:** Update `src-tauri/src/main.rs`
- Add `use github::detail` import (or use full path)
- Add `get_pull_detail` command function
- Register `get_pull_detail` in `tauri::generate_handler![]` (both real and dev-mock variants)

**Step A8:** Update `src-tauri/src/mock/mod.rs`
- Add `comments` and `milestone` to all mock issues
- Add `assignees` to all mock PRs
- Add `cve_id`, `cvss_score`, `cwes`, `dismissed_reason`, `dismissed_comment` to all mock security alerts
- Add `get_pull_detail` mock command

### Phase B — Frontend Libraries

**Step B1:** Download and save `marked.min.js`
- Fetch from: `https://cdn.jsdelivr.net/npm/marked/marked.min.js`
- Save to: `src/lib/marked.min.js`

**Step B2:** Download and save `purify.min.js`
- Fetch from: `https://cdn.jsdelivr.net/npm/dompurify/dist/purify.min.js`
- Save to: `src/lib/purify.min.js`

### Phase C — Frontend HTML

**Step C1:** Update `src/index.html`
- Add `<script src="lib/marked.min.js"></script>` before `main.js` script tag
- Add `<script src="lib/purify.min.js"></script>` before `main.js` script tag

### Phase D — Frontend JavaScript

**Step D1:** Update `src/main.js` — State
- Add `let expandedRow = null;` to state section

**Step D2:** Update `src/main.js` — Markdown helper
- Add `renderMarkdown(text)` function using `marked.parse()` + `DOMPurify.sanitize()`

**Step D3:** Update `src/main.js` — Detail builders
- Add `buildIssueDetail(issue)` function
- Add `buildPullDetail(pull)` function
- Add `buildAlertDetail(alert)` function

**Step D4:** Update `src/main.js` — Toggle logic
- Add `collapseAllRows()` function
- Add `toggleDetailRow(type, idx)` async function

**Step D5:** Update `src/main.js` — Render functions
- Update `renderIssues()` to include data-row + detail-row pairs; remove `<a>` link from title column body
- Update `renderPulls()` to include data-row + detail-row pairs; remove `<a>` link from title column body
- Update `renderAlerts()` to include data-row + detail-row pairs; remove `<a>` link from summary column body

**Step D6:** Update `src/main.js` — refreshData
- Add `expandedRow = null;` (or call `collapseAllRows()`) after render calls

### Phase E — Frontend CSS

**Step E1:** Update `src/styles.css`
- Append all new CSS ruleset blocks specified in Section 8

---

## 12. Dependencies

### New Rust Crates
**None.** All required Rust functionality (octocrab PR detail fetch, Serde, Chrono) is already present in `Cargo.toml`.

### New JavaScript Libraries (self-hosted)
| Library | Version | File | Purpose |
|---|---|---|---|
| marked.js | v9+ (latest stable) | `src/lib/marked.min.js` | Markdown → HTML conversion |
| DOMPurify | v3+ (latest stable) | `src/lib/purify.min.js` | HTML sanitization |

### New Files Created
| File | Purpose |
|---|---|
| `src-tauri/src/github/detail.rs` | PR detail fetch implementation |
| `src/lib/marked.min.js` | Self-hosted marked.js library |
| `src/lib/purify.min.js` | Self-hosted DOMPurify library |

### Files Modified
| File | Change Summary |
|---|---|
| `src-tauri/src/models/mod.rs` | Add fields to Issue, PullRequest, SecurityAlert; add PullDetail struct |
| `src-tauri/src/github/issues.rs` | Map comments and milestone |
| `src-tauri/src/github/pulls.rs` | Map assignees |
| `src-tauri/src/github/security.rs` | Extend raw structs; map CVE/CVSS/CWEs/dismissed fields |
| `src-tauri/src/github/mod.rs` | Add `pub mod detail;` |
| `src-tauri/src/main.rs` | Add get_pull_detail command and register it |
| `src-tauri/src/mock/mod.rs` | Update all mock data; add get_pull_detail mock |
| `src/index.html` | Add marked.js + DOMPurify script tags |
| `src/main.js` | Add detail view logic (state, helpers, builders, toggle, updated renders) |
| `src/styles.css` | Add all detail view styles |

---

## 13. Risks and Mitigations

| Risk | Severity | Mitigation |
|---|---|---|
| **PR lazy fetch race condition**: User rapidly clicks multiple PR rows | Low | `collapseAllRows()` cancels visual state; only the latest expand resolves meaningfully. The stale fetch result targets a stale DOM element ID and either finds the element and updates it (harmless) or silently fails. |
| **Large markdown bodies**: A PR/issue body with 10K+ characters could cause layout overflow | Medium | `detail-body-text` uses `max-height: 300px; overflow-y: auto` to scroll long bodies. |
| **max-height animation stutter**: Content shorter than 700px animates faster; taller content gets clipped | Low | Acceptable trade-off for vanilla CSS. If needed, implement `requestAnimationFrame`-based height measurement in `toggleDetailRow` to set `max-height` to `scrollHeight + "px"`. |
| **XSS via markdown bodies**: Issue/PR bodies are user-supplied and may contain malicious HTML | High | DOMPurify sanitizes all marked.js HTML output before `innerHTML` assignment. Non-markdown fields continue to use `esc()`. |
| **octocrab PR assignees type mismatch**: `pr.assignees` may differ between octocrab versions | Low | The `assignees` field on PR list responses was present in octocrab 0.38 (already in use). Verify during implementation with `cargo check`. |
| **Dependabot API field names**: `dismissed_reason` / `dismissed_comment` field names may differ from actual API | Medium | Verify against GitHub REST API docs at https://docs.github.com/en/rest/dependabot/alerts. Fields use `#[serde(rename = "...")]` if names differ. Test with actual API response. |
| **Code scanning alerts have no advisories**: Extension fields are Dependabot-specific | Low | All new SecurityAlert fields are `Option<_>` or `Vec<_>`; code scanning alerts will simply have `None` / empty values, which the UI renders as "—". |
| **Offline / no internet during detail expand**: `get_pull_detail` requires GitHub API | Low | Show error message in stats slot; all other detail data (body, branches, meta) is already in-memory and displays immediately. |
| **`marked` or `DOMPurify` global not available**: Script load order issue | Low | Both libraries must be loaded before `main.js` in `index.html`. Since Tauri serves local files synchronously, load order is guaranteed. Add a startup guard if needed: `if (typeof marked === "undefined") { console.error("marked.js not loaded"); }` |
| **`data-idx` index mismatch after refresh**: Expand state could point to wrong item | Low | `collapseAllRows()` is called in `refreshData()`, resetting `expandedRow = null` before re-rendering. |

---

*End of Specification*
