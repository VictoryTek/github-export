# Specification: Create New Issue Feature

**Feature Name:** `create_issue`  
**Project:** GitHub Export (Tauri v1 desktop app)  
**Date:** 2026-03-07  
**Status:** Ready for Implementation

---

## 1. Current State Analysis

### 1.1 Toolbar Structure (`src/index.html`, lines 112–127)

The `#toolbar` div currently contains, in order:

```html
<div id="toolbar">
  <select id="state-filter">…</select>
  <select id="sort-filter">…</select>
  <input id="search-input" type="text" placeholder="Search…" />
  <button id="refresh-btn" title="Refresh">&#8635;</button>
  <div class="spacer"></div>
  <button id="export-csv-btn" disabled>Export CSV</button>
  <button id="export-pdf-btn" disabled>Export PDF</button>
</div>
```

The `.spacer` element has `flex: 1` which pushes everything after it to the right edge. Export buttons begin at the right side of the toolbar.

**Insertion point for the new button:** Immediately after `<div class="spacer"></div>` and before `<button id="export-csv-btn"`. This groups the "New Issue" button with the right-side export actions, but distinguishes it as a creation action.

### 1.2 Existing Modal Patterns (`src/index.html`, `src/styles.css`)

Two modal dialogs already exist in the codebase — both follow a consistent pattern:

**Pattern:**
```html
<div id="XXX-modal" class="modal-overlay hidden" role="dialog" aria-modal="true" aria-labelledby="XXX-title">
  <div class="modal-card">
    <h2 id="XXX-title" class="modal-title">Title</h2>
    <p class="modal-subtitle">Optional subtitle</p>
    <!-- inputs -->
    <div id="XXX-error" class="login-error hidden"></div>
    <div class="modal-actions">
      <button id="XXX-submit-btn" class="btn-github-signin">Submit</button>
      <button id="XXX-cancel-btn" class="btn-cancel">Cancel</button>
    </div>
  </div>
</div>
```

**Established CSS classes (no changes needed to existing styles):**
- `.modal-overlay` — fixed full-screen backdrop, `rgba(0,0,0,0.6)` bg, flex centering, `z-index: 200`
- `.modal-card` — `width: 400px`, `max-width: 90vw`, Surface bg, 10px border-radius, 24px padding
- `.modal-title` — 1.1rem, no margin
- `.modal-subtitle` — muted text, 13px
- `.modal-actions` — flex row, `gap: 8px`, `margin-top: 6px`
- `.btn-github-signin` — filled green button, full-width, used for primary submit action
- `.btn-cancel` — transparent border button, hover turns red
- `.login-error` — full-width red error box (reused for in-modal errors)
- `.pat-input` — styled text input with monospace font, full width

**Key JS patterns for modals (from `add-account-modal`):**
- Open: remove `hidden` class, clear fields, focus first input
- Close: add `hidden` class
- Backdrop click: check `e.target === modalEl` to close on overlay click
- Error display: set `.textContent` and remove `.hidden` on the error element

### 1.3 State Variables (`src/main.js`, lines ~25–38)

Relevant state:
- `selectedRepo` — `{ owner: string, name: string } | null` — currently selected repo
- `issues` — `Issue[]` — in-memory issues list for the current repo/filters
- `activeTab` — `string` — currently active tab (`"issues"`, `"pulls"`, `"alerts"`)

### 1.4 Issues List Refresh Pattern

After `close_issue` and `reopen_issue` mutations, the app **surgically updates** `issues[idx]` in place and calls `renderIssues()` — it does NOT call `refreshData()` (which re-fetches all data).

For a newly created issue, the same pattern applies: prepend the returned `Issue` to the `issues` array and call `renderIssues()`. This avoids a full re-fetch and immediately shows the result.

### 1.5 Existing Rust Issue Functions (`src-tauri/src/github/issues.rs`)

Present functions:
- `list_repos` / `list_all_repos`
- `fetch_issues` — list issues with filters
- `close_issue` — sets state to `Closed` via `issues().update().state().send()`
- `reopen_issue` — sets state to `Open` via `issues().update().state().send()`
- `add_issue_comment` — posts a comment via `issues().create_comment().await`

All use the `octocrab` crate (version `0.38`) and the shared `map_issue` helper.

### 1.6 Tauri Command Registration (`src-tauri/src/main.rs`, lines ~352–398)

Commands are registered in two `invoke_handler!` macro calls:
- `#[cfg(not(feature = "dev-mock"))]` block — real implementations
- `#[cfg(feature = "dev-mock")]` block — `mock::*` prefix

The `create_issue` command must be added to **both** registration blocks.

### 1.7 Mock Pattern (`src-tauri/src/mock/mod.rs`)

Existing write-action mocks (`close_issue`, `reopen_issue`, `add_issue_comment`) return hardcoded `Issue` structs or `Ok(())`. The `create_issue` mock must return a fake `Issue` with:
- `number: 999` (distinctive mock issue number)
- `state: "Open"`
- `title` / `body` from the inputs
- `author: "octocat"`
- All other fields set to sensible defaults

---

## 2. Proposed Solution

### 2.1 Design Decision: Modal Dialog

**Choice:** Modal overlay dialog (consistent with existing patterns).

**Rationale:**
- Both existing create-style actions (Add Account, Add Repository) use modal dialogs.
- A modal avoids cluttering the issues table view or requiring inline state management.
- It provides a clear "focused" creation context that can be dismissed.

### 2.2 UI Layout

**"+ New Issue" button** — placed in the toolbar, after `<div class="spacer"></div>` and before `<button id="export-csv-btn">`.

Behavior:
- **Starts disabled** (like Export buttons) — enabled when `selectedRepo !== null`
- **Visible only when issues tab is active** — hidden on other tabs (`hidden` class toggled in the tab-switch handler and `refreshData`)
- Styled with green border/text matching the `.btn-reopen-issue` design language

**Create Issue modal** (`#create-issue-modal`):
- Title: "Create New Issue"
- Subtitle: "Creating in `<owner>/<repo>`"
- Fields:
  1. Title input (text, required, maxlength=256, placeholder "Issue title")
  2. Body textarea (optional, maxlength=65536, rows=6, placeholder "Describe the issue…")
- Error area: `#create-issue-error` reusing `.login-error` class
- Actions: "Create Issue" (green, `#create-issue-submit-btn`) + "Cancel" (`#create-issue-cancel-btn`)

---

## 3. Rust Implementation Steps

### 3.1 New function in `src-tauri/src/github/issues.rs`

Add after `add_issue_comment`:

```rust
/// Create a new issue in the specified repository.
pub async fn create_issue(
    client: &Octocrab,
    owner: &str,
    repo: &str,
    title: &str,
    body: Option<&str>,
) -> Result<crate::models::Issue> {
    let mut builder = client
        .issues(owner, repo)
        .create(title);

    if let Some(b) = body {
        builder = builder.body(b);
    }

    let issue = builder
        .send()
        .await
        .context("Failed to create issue")?;

    Ok(map_issue(issue))
}
```

**Note on octocrab 0.38 API:** The `IssuesHandler::create(title)` method returns a `CreateIssueBuilder`. Chaining `.body(text)` sets the optional body. `.send().await` executes the `POST /repos/{owner}/{repo}/issues` request. The return type is `octocrab::models::issues::Issue`, which is passed through the existing `map_issue` helper.

### 3.2 New Tauri command in `src-tauri/src/main.rs`

Add after the `add_issue_comment` command (around line 247):

```rust
/// Create a new issue in the specified repository.
#[cfg(not(feature = "dev-mock"))]
#[tauri::command]
async fn create_issue(
    owner: String,
    repo: String,
    title: String,
    body: Option<String>,
    state: State<'_, Mutex<AppState>>,
) -> Result<models::Issue, String> {
    // Validate title
    let title = title.trim().to_string();
    if title.is_empty() {
        return Err("Issue title cannot be empty".to_string());
    }
    if title.len() > 256 {
        return Err("Issue title exceeds maximum length of 256 characters".to_string());
    }
    // Validate optional body
    let body = body.map(|b| b.trim().to_string()).filter(|b| !b.is_empty());
    if let Some(ref b) = body {
        if b.len() > 65_536 {
            return Err("Issue body exceeds GitHub's maximum length of 65,536 characters".to_string());
        }
    }
    let client = {
        let app = state.lock().map_err(|e| e.to_string())?;
        app.client.clone().ok_or("Not authenticated")?
    };
    github::issues::create_issue(&client, &owner, &repo, &title, body.as_deref())
        .await
        .map_err(|e| e.to_string())
}
```

### 3.3 Register in the `invoke_handler!` macro (`src-tauri/src/main.rs`)

**In the `#[cfg(not(feature = "dev-mock"))]` block**, add `create_issue` to the handler list after `add_issue_comment`:

```rust
add_issue_comment,
create_issue,          // ← add this line
fetch_pulls,
```

**In the `#[cfg(feature = "dev-mock")]` block**, add `mock::create_issue` after `mock::add_issue_comment`:

```rust
mock::add_issue_comment,
mock::create_issue,    // ← add this line
mock::fetch_pulls,
```

---

## 4. Mock Stub

### 4.1 Add to `src-tauri/src/mock/mod.rs`

Add after `add_issue_comment` (around line 525):

```rust
/// Mock: simulate creating a new issue. Returns a fake Issue with number 999.
#[tauri::command]
pub fn create_issue(
    _owner: String,
    _repo: String,
    title: String,
    body: Option<String>,
    _state: tauri::State<'_, Mutex<AppState>>,
) -> Result<Issue, String> {
    let title = title.trim().to_string();
    if title.is_empty() {
        return Err("Issue title cannot be empty".to_string());
    }
    if title.len() > 256 {
        return Err("Issue title exceeds maximum length of 256 characters".to_string());
    }
    Ok(Issue {
        number: 999,
        title,
        state: "Open".to_string(),
        author: "octocat".to_string(),
        labels: vec![],
        assignees: vec![],
        created_at: dt("2026-03-07T12:00:00Z"),
        updated_at: dt("2026-03-07T12:00:00Z"),
        closed_at: None,
        html_url: "https://github.com/octocat/Hello-World/issues/999".to_string(),
        body,
        comments: 0,
        milestone: None,
    })
}
```

---

## 5. Frontend Implementation Steps

### 5.1 HTML Changes (`src/index.html`)

#### 5.1.1 Add "New Issue" button to toolbar

**Insertion point:** After `<div class="spacer"></div>` and before `<button id="export-csv-btn"`.

**Replace:**
```html
        <div class="spacer"></div>
        <button id="export-csv-btn" disabled>Export CSV</button>
```

**With:**
```html
        <div class="spacer"></div>
        <button id="new-issue-btn" class="btn-new-issue hidden" disabled>+ New Issue</button>
        <button id="export-csv-btn" disabled>Export CSV</button>
```

Note: starts with `hidden` and `disabled`; hidden because the issues tab isn't "active" until a repo is selected and the tab is shown.

#### 5.1.2 Add Create Issue modal

**Insertion point:** After the closing `</div>` of `#add-repo-modal` and before `<script src="vendor/marked.min.js">`.

**Add:**
```html
  <!-- Create Issue modal overlay -->
  <div id="create-issue-modal" class="modal-overlay hidden" role="dialog" aria-modal="true" aria-labelledby="create-issue-title">
    <div class="modal-card modal-card-create-issue">
      <div class="modal-header">
        <h2 id="create-issue-title" class="modal-title">Create New Issue</h2>
        <button id="create-issue-close-btn" class="modal-close-btn" aria-label="Close">×</button>
      </div>
      <p id="create-issue-subtitle" class="modal-subtitle"></p>
      <input
        id="create-issue-title-input"
        type="text"
        placeholder="Issue title"
        class="pat-input"
        maxlength="256"
        autocomplete="off"
        spellcheck="true"
      />
      <textarea
        id="create-issue-body-input"
        placeholder="Describe the issue… (optional)"
        class="issue-create-body-input"
        rows="6"
        maxlength="65536"
        spellcheck="true"
      ></textarea>
      <div id="create-issue-error" class="login-error hidden"></div>
      <div class="modal-actions">
        <button id="create-issue-submit-btn" class="btn-github-signin">Create Issue</button>
        <button id="create-issue-cancel-btn" class="btn-cancel">Cancel</button>
      </div>
    </div>
  </div>
```

### 5.2 CSS Changes (`src/styles.css`)

Add after the existing `/* Widen max-height for issue detail rows… */` block at the very end of the file:

```css
/* ── Create Issue button (toolbar) ──────────── */

.btn-new-issue {
  background: rgba(63, 185, 80, 0.12);
  color: var(--green) !important;
  border: 1px solid rgba(63, 185, 80, 0.35) !important;
  font-weight: 600;
}

.btn-new-issue:hover:not(:disabled) {
  background: rgba(63, 185, 80, 0.22) !important;
  opacity: 1 !important;
}

/* ── Create Issue modal ──────────────────────── */

.modal-card-create-issue {
  width: 520px;
}

.issue-create-body-input {
  width: 100%;
  background: var(--bg);
  border: 1px solid var(--border);
  border-radius: var(--radius);
  color: var(--text);
  font-size: 0.84rem;
  font-family: var(--font);
  padding: 0.5rem 0.7rem;
  resize: vertical;
  min-height: 7rem;
}

.issue-create-body-input:focus {
  outline: none;
  border-color: var(--accent);
}
```

**Why `!important` on button styles:** The toolbar `#toolbar button` rule has high specificity (`padding`, `background: var(--accent)`, `color: #fff`). Using `.btn-new-issue` class alone would be overridden. The `!important` on the override properties ensures the green theme applies without changing the cascade ordering.

### 5.3 JavaScript Changes (`src/main.js`)

#### 5.3.1 Add DOM reference (near the top with other DOM refs)

After `const exportPdf = …` and before `const placeholder = …`:

```javascript
const newIssueBtn  = $("#new-issue-btn");
```

#### 5.3.2 Enable/disable "New Issue" button alongside exports

The `refreshData()` function enables `exportCsv.disabled = false` and `exportPdf.disabled = false` at the end (around line 660). Add `newIssueBtn.disabled = false;` alongside:

```javascript
  loading.classList.add("hidden");
  exportCsv.disabled = false;
  exportPdf.disabled = false;
  newIssueBtn.disabled = false;
```

#### 5.3.3 Show/hide "New Issue" button by active tab

In the tab switch event handler (around line 510–517), where `activeTab` is set and panels are toggled, add show/hide logic:

```javascript
$$(".tab").forEach((btn) => {
  btn.addEventListener("click", () => {
    $$(".tab").forEach((b) => b.classList.remove("active"));
    $$(".tab-panel").forEach((p) => p.classList.remove("active"));
    btn.classList.add("active");
    activeTab = btn.dataset.tab;
    $(`#tab-${activeTab}`).classList.add("active");
    // Show the New Issue button only on the issues tab
    if (activeTab === "issues") {
      newIssueBtn.classList.remove("hidden");
    } else {
      newIssueBtn.classList.add("hidden");
    }
  });
});
```

Also in `showApp()` (around line 183), when the app first loads (defaults to "issues" tab), show the button:

```javascript
async function showApp(username) {
  usernameEl.textContent = `@${username}`;
  renderAccountSwitcher();
  loginScreen.classList.add("hidden");
  appScreen.classList.remove("hidden");
  newIssueBtn.classList.remove("hidden");  // ← add this
  pickerLoaded = false;
  allRepos = [];
  await loadTrackedRepos();
}
```

Also reset in `handleSwitchAccount()` when `selectedRepo` is cleared and the tab might change — ensure the button stays hidden until re-enabled by `refreshData()`. Add `newIssueBtn.disabled = true;` alongside where `selectedRepo = null` is set. Also, if switching account resets to issues tab, ensure `newIssueBtn.classList.remove("hidden")`.

#### 5.3.4 Create Issue modal — open/close event handlers

Add a dedicated section after the existing "Add Repository button & modal events" block:

```javascript
// ── Create Issue modal ───────────────────────────

function openCreateIssueModal() {
  if (!selectedRepo) return;
  const modal = document.getElementById("create-issue-modal");
  const subtitleEl = document.getElementById("create-issue-subtitle");
  const titleInput = document.getElementById("create-issue-title-input");
  const bodyInput = document.getElementById("create-issue-body-input");
  const errorEl = document.getElementById("create-issue-error");
  const submitBtn = document.getElementById("create-issue-submit-btn");

  subtitleEl.textContent = `Creating in ${selectedRepo.owner}/${selectedRepo.name}`;
  titleInput.value = "";
  bodyInput.value = "";
  errorEl.classList.add("hidden");
  submitBtn.disabled = false;

  modal.classList.remove("hidden");
  titleInput.focus();
}

function closeCreateIssueModal() {
  document.getElementById("create-issue-modal").classList.add("hidden");
}

newIssueBtn.addEventListener("click", openCreateIssueModal);

document.getElementById("create-issue-cancel-btn").addEventListener("click", closeCreateIssueModal);

document.getElementById("create-issue-close-btn").addEventListener("click", closeCreateIssueModal);

// Close on backdrop click
document.getElementById("create-issue-modal").addEventListener("click", (e) => {
  if (e.target === document.getElementById("create-issue-modal")) {
    closeCreateIssueModal();
  }
});

// Close on Escape key
document.addEventListener("keydown", (e) => {
  if (e.key === "Escape") {
    const modal = document.getElementById("create-issue-modal");
    if (!modal.classList.contains("hidden")) {
      closeCreateIssueModal();
    }
  }
});

document.getElementById("create-issue-submit-btn").addEventListener("click", async () => {
  const titleInput = document.getElementById("create-issue-title-input");
  const bodyInput  = document.getElementById("create-issue-body-input");
  const errorEl    = document.getElementById("create-issue-error");
  const submitBtn  = document.getElementById("create-issue-submit-btn");

  const title = titleInput.value.trim();
  const body  = bodyInput.value.trim() || null;

  // Client-side validation
  if (!title) {
    errorEl.textContent = "Issue title is required.";
    errorEl.classList.remove("hidden");
    titleInput.focus();
    return;
  }
  if (title.length > 256) {
    errorEl.textContent = "Issue title must be 256 characters or fewer.";
    errorEl.classList.remove("hidden");
    titleInput.focus();
    return;
  }
  if (body && body.length > 65536) {
    errorEl.textContent = "Issue body must be 65,536 characters or fewer.";
    errorEl.classList.remove("hidden");
    bodyInput.focus();
    return;
  }

  errorEl.classList.add("hidden");
  submitBtn.disabled = true;
  submitBtn.textContent = "Creating…";

  try {
    const newIssue = await invoke("create_issue", {
      owner: selectedRepo.owner,
      repo: selectedRepo.name,
      title,
      body,
    });

    // Prepend the new issue to the local list and re-render
    issues.unshift(newIssue);
    renderIssues();
    updateTabBadges(issues.length, pulls.length, alerts.length);

    closeCreateIssueModal();
  } catch (err) {
    errorEl.textContent = esc(String(err));
    errorEl.classList.remove("hidden");
    submitBtn.disabled = false;
    submitBtn.textContent = "Create Issue";
  }
});
```

---

## 6. Security Considerations

### 6.1 Input Validation (Defense in Depth)

Validation occurs at **two layers**:

| Layer | What is validated |
|-------|-------------------|
| Frontend JS | Title non-empty, title ≤ 256 chars, body ≤ 65,536 chars (before invoke) |
| Rust backend | Title trimmed + non-empty, title ≤ 256 chars, body trimmed + ≤ 65,536 chars (before API call) |

The Rust layer is the authoritative gate; the JS layer provides immediate user feedback.

### 6.2 HTML Injection Prevention

All user-supplied strings are rendered via either:
- `esc()` — the existing XSS-safe HTML-escaping helper used throughout `main.js`
- Direct DOM assignment via `.textContent` (safe by design)

The new issue's `title` and `body` are passed to `renderIssues()` → `buildIssueDetail()`, which already uses `esc(i.title)` and `renderMarkdown(i.body)` (DOMPurify-sanitized Markdown). No new XSS risk is introduced.

### 6.3 GitHub API Error Handling

The Rust backend propagates GitHub API errors (including `422 Unprocessable Entity` for validation failures, `403 Forbidden` for permission issues, `404 Not Found`) as `String` errors through the `Result<Issue, String>` command signature. The frontend displays these errors verbatim in the `#create-issue-error` element via `esc(String(err))` — XSS-safe.

### 6.4 Authentication Guard

The Rust `create_issue` command checks `app.client.clone().ok_or("Not authenticated")?` — the same pattern as all other commands. A call without authentication returns an error immediately.

### 6.5 No New Permissions Required

The GitHub API `POST /repos/{owner}/{repo}/issues` endpoint requires only the `repo` scope, which is already required at login. No new scope is needed.

---

## 7. Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|------------|
| octocrab `create()` builder API changed in 0.38 | Low | Build failure | Verify against octocrab 0.38 crate source; the `IssuesHandler::create(title).body(body).send()` chain is stable in this version |
| GitHub API returns 403 (no write access to repo) | Medium | User sees error | Error propagated and displayed in modal error box; no crash |
| GitHub API returns 422 (validation failed) | Low | User sees error | Same error propagation; message will include GitHub's validation details |
| Body text contains special characters that break serialization | Low | Request error | serde_json handles all Unicode correctly; no risk |
| "New Issue" button visible on Pulls/Alerts tabs if tab-switch logic missed | Low | UX issue | The tab-switch handler adds/removes `hidden`; also `hidden` by default in HTML |
| Mock number 999 clashes with real issue list | Low | Minor UX oddity | In mock mode only; no real data; acceptable for dev testing |
| Double-click on submit button fires two requests | Low | Duplicate issue | `submitBtn.disabled = true` on first click prevents this |

---

## 8. Implementation Checklist

### Rust (`src-tauri/src/github/issues.rs`)
- [ ] Add `create_issue(client, owner, repo, title, body)` function after `add_issue_comment`

### Rust (`src-tauri/src/main.rs`)
- [ ] Add `create_issue` Tauri command after `add_issue_comment` Tauri command
- [ ] Register `create_issue` in the `#[cfg(not(feature = "dev-mock"))]` invoke handler list
- [ ] Register `mock::create_issue` in the `#[cfg(feature = "dev-mock")]` invoke handler list

### Rust (`src-tauri/src/mock/mod.rs`)
- [ ] Add `mock::create_issue` function after `add_issue_comment` mock

### HTML (`src/index.html`)
- [ ] Add `#new-issue-btn` button to toolbar (after `.spacer`, before `#export-csv-btn`)
- [ ] Add `#create-issue-modal` modal HTML (after `#add-repo-modal`, before `<script>` tags)

### CSS (`src/styles.css`)
- [ ] Add `.btn-new-issue` styles (green-themed toolbar button)
- [ ] Add `.modal-card-create-issue` width override
- [ ] Add `.issue-create-body-input` textarea styles

### JavaScript (`src/main.js`)
- [ ] Add `newIssueBtn` DOM reference constant
- [ ] Enable `newIssueBtn.disabled = false` in `refreshData()`
- [ ] Show `newIssueBtn` in `showApp()` (default tab is issues)
- [ ] Toggle `newIssueBtn` visibility in the tab-switch click handler
- [ ] Add Create Issue modal open/close/submit event handlers
- [ ] Prepend new issue to `issues[]` and call `renderIssues()` + `updateTabBadges()` on success

---

## 9. Research Sources

1. **GitHub REST API — Create an Issue**  
   `POST /repos/{owner}/{repo}/issues` — title (required), body (optional), labels (optional, not in scope). Returns full issue object. 422 on validation failures, 403 on insufficient permissions.  
   https://docs.github.com/en/rest/issues/issues#create-an-issue

2. **GitHub Issue title/body limits**  
   Title: maximum 256 characters (documented in GitHub's GraphQL and REST validation). Body: maximum 65,536 characters (same limit as comments, enforced by GitHub's API).  
   https://docs.github.com/en/graphql/reference/input-objects#createissueinput

3. **octocrab 0.38 IssuesHandler::create API**  
   `client.issues(owner, repo).create(title).body(text).send().await` — `create()` takes `impl Into<String>` for title and returns `CreateIssueBuilder`. `.body()` accepts `impl Into<String>`. `.send()` issues the POST.  
   https://docs.rs/octocrab/0.38.0/octocrab/issues/struct.IssueHandler.html

4. **Tauri v1 Async Command Pattern**  
   `#[tauri::command]` on `async fn` functions — Tokio executor handles async. State accessed with `state.lock()` before the await point to avoid holding MutexGuard across await. Consistent with all existing async commands in `main.rs`.  
   https://tauri.app/v1/guides/features/command/

5. **Tauri IPC invoke() from JavaScript**  
   `window.__TAURI__.tauri.invoke('create_issue', { owner, repo, title, body })` — camelCase for argument keys (Tauri automatically converts snake_case Rust param names to camelCase for JS).  
   https://tauri.app/v1/api/js/tauri/#invoke

6. **DOMPurify + marked.js for Markdown rendering (security)**  
   Already used in `renderMarkdown()` — passes raw GitHub Markdown through `marked.parse()` then `DOMPurify.sanitize()`. The new issue body shown in `buildIssueDetail()` uses this same path. No new sanitization code required.  
   https://github.com/cure53/DOMPurify

7. **OWASP Input Validation / XSS Prevention**  
   All user-generated strings displayed in the DOM go through either `esc()` (for plain text) or `DOMPurify.sanitize()` (for Markdown HTML). Server-side validation in Rust prevents oversized inputs from reaching the GitHub API.  
   https://cheatsheetseries.owasp.org/cheatsheets/Cross_Site_Scripting_Prevention_Cheat_Sheet.html

8. **UX patterns — Create dialogs in desktop/GitHub apps**  
   GitHub.com uses a modal/overlay for "New Issue" only on project boards; the main issue list uses a full page. For this desktop utility, a compact modal is preferred to maintain single-page app flow — consistent with the app's existing Add Account and Add Repository modals.
