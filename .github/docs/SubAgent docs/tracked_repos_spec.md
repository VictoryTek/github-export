# Tracked Repositories — Feature Specification

**Feature:** GitHub Desktop-style tracked repository panel  
**Project:** GitHub Export (Tauri v1 — Rust + HTML/CSS/JS)  
**Author:** Research Subagent  
**Date:** 2026-03-05  
**Status:** Ready for Implementation

---

## Table of Contents

1. [Current State Analysis](#1-current-state-analysis)
2. [Proposed Architecture](#2-proposed-architecture)
3. [Data Models](#3-data-models)
4. [Rust Implementation Plan](#4-rust-implementation-plan)
5. [JavaScript Implementation Plan](#5-javascript-implementation-plan)
6. [HTML Changes](#6-html-changes)
7. [CSS Changes](#7-css-changes)
8. [Mock Module Changes](#8-mock-module-changes)
9. [Step-by-Step Implementation Order](#9-step-by-step-implementation-order)
10. [File Paths and Function Names Summary](#10-file-paths-and-function-names-summary)
11. [JSON Schema for Persistence File](#11-json-schema-for-persistence-file)
12. [Risks and Mitigations](#12-risks-and-mitigations)
13. [Nice-to-Have: Remove Tracked Repo](#13-nice-to-have-remove-tracked-repo)

---

## 1. Current State Analysis

### 1.1 How Repositories Currently Work

**Boot flow:**
1. After login, `showApp(username)` in `src/main.js` calls `loadRepos()`.
2. `loadRepos()` calls `invoke("list_repos")`, which maps to the Rust command `list_repos` in `src-tauri/src/main.rs`.
3. The Rust command delegates to `github::issues::list_repos(&client)` in `src-tauri/src/github/issues.rs`.
4. `list_repos` calls `client.current().list_repos_for_authenticated_user().sort("updated").per_page(50).send()` — fetching **up to 50 repositories** in one page from the GitHub API.
5. The result (`Vec<Repo>`) is returned to the JS and stored in the global `repos` array.
6. `renderRepoList(repos)` populates `<ul id="repo-list">` — **all 50 repos are shown immediately**.
7. The `#repo-search` input filters the rendered list in real-time by `full_name`.

**Key observations:**
- There is **no concept of "tracked" or "pinned" repos** — all available repos are shown.
- There is **no user-curated list** — the sidebar is populated by the raw GitHub API result.
- There is **no persistent local state** for the sidebar (repos are re-fetched from GitHub every session).
- After switching accounts (`handleSwitchAccount`), `loadRepos()` is called again, replacing the sidebar.

### 1.2 Relevant Files

| File | Role |
|------|------|
| `src/index.html` | HTML structure — sidebar `<aside id="sidebar">` contains `<h3>Repositories</h3>`, `<input id="repo-search">`, `<ul id="repo-list">` |
| `src/main.js` | JS logic — `loadRepos()`, `renderRepoList()`, `selectRepo()`, `repoSearch` filter handler |
| `src/styles.css` | CSS — `#sidebar`, `#repo-list li`, `.modal-overlay`, `.modal-card` |
| `src-tauri/src/main.rs` | Rust entry point — `list_repos` Tauri command, `invoke_handler![]` registration |
| `src-tauri/src/github/issues.rs` | `pub async fn list_repos(client)` — fetches 50 repos via Octocrab |
| `src-tauri/src/models/mod.rs` | `Repo`, `AppState`, `Account`, `AccountInfo` structs |
| `src-tauri/src/mock/mod.rs` | Mock `list_repos` returning 3 hardcoded repos |

### 1.3 Existing `AppState` Structure

```rust
pub struct AppState {
    pub client: Option<Octocrab>,
    pub token: Option<String>,
    pub username: Option<String>,
    pub active_account_id: Option<String>,  // UUID of the active account
    pub accounts: Vec<Account>,
}
```

`AppState` holds in-memory runtime state only. Persistence is handled by the **OS keyring** (via the `keyring` crate) for account tokens and account index JSON. **No file-system persistence exists today.**

### 1.4 Existing Persistence Pattern (Keyring)

Accounts are persisted in the OS keyring under keys:
- `"github-export" / "accounts-index"` → JSON array of `Account` objects
- `"github-export" / "token-{account_id}"` → raw PAT/OAuth token string
- `"github-export" / "active-account-id"` → active account UUID string

The keyring is **not appropriate** for tracked repos (it is designed for secrets, not structured list data), so we will use a **JSON file in the app data directory** instead.

### 1.5 Existing Modal Pattern

The "Add Account" modal (`#add-account-modal`) uses `.modal-overlay` + `.modal-card` classes already defined in `styles.css`. This pattern will be **reused** for the "Add Repository" picker modal.

### 1.6 Limitation: `list_repos` fetches only 50 repos

The current `list_repos` call uses `.per_page(50)`. Users with many repositories would not see all of them. The implementation will bump this to 100 (the GitHub API maximum) for the picker command, and add a documented limitation note.

---

## 2. Proposed Architecture

### 2.1 High-Level Concept

```
┌─────────────────────────────────────────────────────────────────┐
│ SIDEBAR                                                         │
│  ┌────────────────────────────────────────────────────────────┐ │
│  │ @username                                               ▾  │ │
│  └────────────────────────────────────────────────────────────┘ │
│  Repositories                                                   │
│  ┌──────────────────────────────────────────────────────────┐   │
│  │ + Add Repository                                         │   │
│  └──────────────────────────────────────────────────────────┘   │
│  ┌──────────────────────────────────────────────────────────┐   │
│  │ Filter repos…                                            │   │
│  └──────────────────────────────────────────────────────────┘   │
│  (empty until repos are tracked)                                │
│   owner/repo-1                                          [×]     │
│   owner/repo-2                                          [×]     │
└─────────────────────────────────────────────────────────────────┘

When "+ Add Repository" is clicked:

┌─────────────────────────────────────────────────────────────────┐
│ MODAL: Add Repository                                   [×]     │
│  ┌──────────────────────────────────────────────────────────┐   │
│  │ Search repositories…                                     │   │
│  └──────────────────────────────────────────────────────────┘   │
│  ┌──────────────────────────────────────────────────────────┐   │
│  │ owner/repo-a                  (already added ✓)          │   │
│  │ owner/repo-b                                             │   │
│  │ owner/repo-c                                             │   │
│  │ …                                                        │   │
│  └──────────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────┘
```

### 2.2 Data Flow

```
Login / account switch
        │
        ▼
invoke("get_tracked_repos")  ──►  Rust reads tracked_repos.json from app data dir
        │                             (keyed by active_account_id)
        ▼
JS: trackedRepos = [TrackedRepo, ...]
        │
        ▼
renderTrackedRepoList(trackedRepos)  →  sidebar shows only tracked repos

User clicks "+ Add Repository"
        │
        ▼
invoke("list_all_repos")  ──►  Rust fetches up to 100 repos from GitHub API
        │
        ▼
JS: Show modal with repo list (searchable)
User selects a repo
        │
        ▼
invoke("add_tracked_repo", { fullName, owner, name })
        │
        ▼
Rust: appends to tracked_repos.json, returns updated Vec<TrackedRepo>
        │
        ▼
JS: trackedRepos updated, modal closed, sidebar re-rendered
```

### 2.3 Key Design Decisions

| Decision | Rationale |
|----------|-----------|
| Persist in JSON file (not keyring) | Keyring is for secrets; tracked repos are non-sensitive list data |
| File path: `<app_data_dir>/tracked_repos.json` | `tauri::api::path::app_data_dir()` gives a stable, per-app, per-user directory |
| Per-account namespacing within the JSON file | Multiple accounts may track different repos; keyed by `active_account_id` |
| Separate `list_all_repos` command (not modifying `list_repos`) | `list_repos` is currently unused after this change but kept for backward compat and mock mode; `list_all_repos` fetches 100 per page |
| Modal loads repos lazily (on first open) | Avoids an API call on every login; only called when user wants to add a repo |
| No `AppState` changes | Tracked repos are persisted to disk; no need to hold them in runtime state |

---

## 3. Data Models

### 3.1 New Struct: `TrackedRepo`

**File:** `src-tauri/src/models/mod.rs`

```rust
/// A user-curated tracked repository entry.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TrackedRepo {
    /// Fully-qualified repository name, e.g., "octocat/Hello-World".
    pub full_name: String,
    /// Repository owner login.
    pub owner: String,
    /// Repository name (without owner prefix).
    pub name: String,
}
```

This struct is small deliberately — it contains only what is needed to:
1. Display the repo in the sidebar (`full_name`)
2. Invoke `fetch_issues`, `fetch_pulls`, `fetch_security_alerts` (`owner` + `name`)

The full `Repo` struct (with `description`, `html_url`, etc.) is not stored to keep the persistence file minimal and avoid stale data.

### 3.2 Persistence File Structure

**File path:** `<app_data_dir>/tracked_repos.json`

Example with two accounts:
```json
{
  "a1b2c3d4-1234-5678-90ab-cdef01234567": [
    { "full_name": "octocat/Hello-World", "owner": "octocat", "name": "Hello-World" },
    { "full_name": "rust-lang/rust",      "owner": "rust-lang", "name": "rust" }
  ],
  "f9e8d7c6-9876-5432-10fe-dcba98765432": [
    { "full_name": "microsoft/vscode", "owner": "microsoft", "name": "vscode" }
  ]
}
```

Top-level keys are account UUIDs matching `AppState.active_account_id`. If the account ID is missing (e.g., dev-mock mode), use a fallback key `"default"`.

---

## 4. Rust Implementation Plan

### 4.1 New File: `src-tauri/src/storage.rs`

This module encapsulates all file-system persistence for tracked repos.

```rust
// src-tauri/src/storage.rs

use crate::models::TrackedRepo;
use anyhow::{Context, Result};
use serde_json;
use std::collections::HashMap;
use std::fs;
use tauri::AppHandle;

/// Returns the path to the tracked_repos.json file in the app data directory.
fn tracked_repos_path(app_handle: &AppHandle) -> Result<std::path::PathBuf> {
    let data_dir = tauri::api::path::app_data_dir(app_handle.config().as_ref())
        .context("Failed to resolve app data directory")?;
    fs::create_dir_all(&data_dir)
        .context("Failed to create app data directory")?;
    Ok(data_dir.join("tracked_repos.json"))
}

/// Load the full tracked repos map from disk. Returns an empty map if the file
/// does not exist or cannot be parsed.
fn load_all(app_handle: &AppHandle) -> HashMap<String, Vec<TrackedRepo>> {
    let path = match tracked_repos_path(app_handle) {
        Ok(p) => p,
        Err(_) => return HashMap::new(),
    };
    let json = match fs::read_to_string(&path) {
        Ok(s) => s,
        Err(_) => return HashMap::new(),
    };
    serde_json::from_str(&json).unwrap_or_default()
}

/// Persist the full tracked repos map to disk.
fn save_all(app_handle: &AppHandle, map: &HashMap<String, Vec<TrackedRepo>>) -> Result<()> {
    let path = tracked_repos_path(app_handle)?;
    let json = serde_json::to_string_pretty(map)
        .context("Failed to serialize tracked repos")?;
    fs::write(&path, json).context("Failed to write tracked repos file")
}

/// Load tracked repos for a specific account.
pub fn load_tracked_repos(app_handle: &AppHandle, account_id: &str) -> Vec<TrackedRepo> {
    let mut map = load_all(app_handle);
    map.remove(account_id).unwrap_or_default()
}

/// Save (replace) tracked repos for a specific account.
pub fn save_tracked_repos(
    app_handle: &AppHandle,
    account_id: &str,
    repos: &[TrackedRepo],
) -> Result<()> {
    let mut map = load_all(app_handle);
    map.insert(account_id.to_string(), repos.to_vec());
    save_all(app_handle, &map)
}
```

**Key points:**
- Uses `std::fs` (synchronous) — acceptable here because tracked repos file is tiny (< 10 KB in all realistic cases).
- `create_dir_all` ensures the directory exists before reading/writing.
- Errors during load return empty defaults (graceful degradation); errors during save are propagated as `Result`.
- No `unsafe` code, no secrets stored.

### 4.2 New Tauri Commands in `src-tauri/src/main.rs`

#### Command 1: `get_tracked_repos`

```rust
/// Return the list of tracked repositories for the currently active account.
#[cfg(not(feature = "dev-mock"))]
#[tauri::command]
fn get_tracked_repos(
    app_handle: tauri::AppHandle,
    state: State<'_, Mutex<AppState>>,
) -> Result<Vec<models::TrackedRepo>, String> {
    let account_id = {
        let app = state.lock().map_err(|e| e.to_string())?;
        app.active_account_id.clone().unwrap_or_else(|| "default".to_string())
    };
    Ok(storage::load_tracked_repos(&app_handle, &account_id))
}
```

#### Command 2: `add_tracked_repo`

```rust
/// Add a repository to the tracked list for the currently active account.
/// Returns the updated tracked list.
/// Returns an error if the repo is already tracked.
#[cfg(not(feature = "dev-mock"))]
#[tauri::command]
fn add_tracked_repo(
    full_name: String,
    owner: String,
    name: String,
    app_handle: tauri::AppHandle,
    state: State<'_, Mutex<AppState>>,
) -> Result<Vec<models::TrackedRepo>, String> {
    // Input validation: full_name must equal "owner/name" to prevent injection
    let expected = format!("{owner}/{name}");
    if full_name != expected {
        return Err("Invalid repo identifier: full_name does not match owner/name".to_string());
    }
    // Basic character validation — owner and name should only contain alphanumeric, hyphens, dots
    let valid_chars = |s: &str| s.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_' || c == '.');
    if !valid_chars(&owner) || !valid_chars(&name) {
        return Err("Invalid characters in repository owner or name".to_string());
    }

    let account_id = {
        let app = state.lock().map_err(|e| e.to_string())?;
        app.active_account_id.clone().unwrap_or_else(|| "default".to_string())
    };

    let mut repos = storage::load_tracked_repos(&app_handle, &account_id);

    // Idempotency: if already tracked, return current list without error
    if repos.iter().any(|r| r.full_name == full_name) {
        return Ok(repos);
    }

    repos.push(models::TrackedRepo { full_name, owner, name });
    storage::save_tracked_repos(&app_handle, &account_id, &repos)
        .map_err(|e| format!("Failed to save tracked repos: {e}"))?;

    Ok(repos)
}
```

#### Command 3: `remove_tracked_repo` (nice-to-have)

```rust
/// Remove a repository from the tracked list by full_name.
/// Returns the updated tracked list.
#[cfg(not(feature = "dev-mock"))]
#[tauri::command]
fn remove_tracked_repo(
    full_name: String,
    app_handle: tauri::AppHandle,
    state: State<'_, Mutex<AppState>>,
) -> Result<Vec<models::TrackedRepo>, String> {
    let account_id = {
        let app = state.lock().map_err(|e| e.to_string())?;
        app.active_account_id.clone().unwrap_or_else(|| "default".to_string())
    };

    let mut repos = storage::load_tracked_repos(&app_handle, &account_id);
    let original_len = repos.len();
    repos.retain(|r| r.full_name != full_name);

    if repos.len() < original_len {
        storage::save_tracked_repos(&app_handle, &account_id, &repos)
            .map_err(|e| format!("Failed to save tracked repos: {e}"))?;
    }

    Ok(repos)
}
```

#### Command 4: `list_all_repos` (for the picker modal)

```rust
/// List all repositories visible to the authenticated user (up to 100).
/// Used exclusively by the "Add Repository" picker modal.
#[cfg(not(feature = "dev-mock"))]
#[tauri::command]
async fn list_all_repos(state: State<'_, Mutex<AppState>>) -> Result<Vec<models::Repo>, String> {
    let client = {
        let app = state.lock().map_err(|e| e.to_string())?;
        app.client.clone().ok_or("Not authenticated")?
    };
    github::issues::list_all_repos(&client)
        .await
        .map_err(|e| e.to_string())
}
```

### 4.3 New Function in `src-tauri/src/github/issues.rs`

```rust
/// List up to 100 repositories visible to the authenticated user.
/// Used by the "Add Repository" picker modal.
pub async fn list_all_repos(client: &Octocrab) -> Result<Vec<Repo>> {
    let mut page = client
        .current()
        .list_repos_for_authenticated_user()
        .sort("updated")
        .per_page(100)
        .send()
        .await
        .context("Failed to list repositories")?;

    let repos = page
        .take_items()
        .into_iter()
        .map(|r| Repo {
            id: r.id.into_inner(),
            name: r.name.clone(),
            full_name: r.full_name.clone().unwrap_or_default(),
            owner: r.owner.as_ref().map(|o| o.login.clone()).unwrap_or_default(),
            description: r.description.clone(),
            private: r.private.unwrap_or(false),
            html_url: r.html_url.as_ref().map(|u| u.to_string()).unwrap_or_default(),
            open_issues_count: r.open_issues_count.unwrap_or(0),
        })
        .collect();

    Ok(repos)
}
```

### 4.4 Register New Module in `src-tauri/src/main.rs`

Add `mod storage;` at the top (alongside `mod github;`, `mod export;`, etc.).

Add the new commands to the `invoke_handler![]` in both `#[cfg(not(feature = "dev-mock"))]` and `#[cfg(feature = "dev-mock")]` branches.

Non-mock handler additions:
```rust
get_tracked_repos,
add_tracked_repo,
remove_tracked_repo,
list_all_repos,
```

### 4.5 Impact on Existing `list_repos` Command

The existing `list_repos` Tauri command **remains untouched**. After this feature is implemented, it won't be called from the JS during `showApp()` anymore (replaced by `get_tracked_repos`), but it remains registered and functional for backward compatibility and potential future use.

The existing `github::issues::list_repos` function (fetching 50 repos) also remains unchanged.

---

## 5. JavaScript Implementation Plan

### 5.1 New State Variables

Add to the top-level state section of `src/main.js`:

```javascript
let trackedRepos  = [];   // TrackedRepo[] — the user's curated list
let allRepos      = [];   // Repo[] — full GitHub repo list (loaded lazily for picker)
let pickerLoaded  = false; // whether allRepos has been fetched this session
```

### 5.2 Modify `showApp()`

Change the call from `loadRepos()` to `loadTrackedRepos()`:

**Current:**
```javascript
async function showApp(username) {
  usernameEl.textContent = `@${username}`;
  renderAccountSwitcher();
  loginScreen.classList.add("hidden");
  appScreen.classList.remove("hidden");
  await loadRepos();
}
```

**New:**
```javascript
async function showApp(username) {
  usernameEl.textContent = `@${username}`;
  renderAccountSwitcher();
  loginScreen.classList.add("hidden");
  appScreen.classList.remove("hidden");
  pickerLoaded = false;   // reset lazy-load flag on account switch
  allRepos = [];
  await loadTrackedRepos();
}
```

### 5.3 New Function: `loadTrackedRepos()`

```javascript
async function loadTrackedRepos() {
  try {
    trackedRepos = await invoke("get_tracked_repos");
  } catch (e) {
    console.error("get_tracked_repos failed:", e);
    trackedRepos = [];
  }
  renderTrackedRepoList(trackedRepos);
}
```

### 5.4 New Function: `renderTrackedRepoList(list)`

Replaces `renderRepoList()` for the sidebar. Also filters by the `#repo-search` input value:

```javascript
function renderTrackedRepoList(list) {
  const q = repoSearch.value.toLowerCase();
  const filtered = q
    ? list.filter(r => r.full_name.toLowerCase().includes(q))
    : list;

  repoList.innerHTML = "";

  if (filtered.length === 0 && !q) {
    // Empty state — show a helpful hint
    const li = document.createElement("li");
    li.className = "repo-list-empty";
    li.textContent = "No repositories tracked yet.";
    repoList.appendChild(li);
    return;
  }

  filtered.forEach((r) => {
    const li = document.createElement("li");
    li.className = "repo-list-item";

    const nameSpan = document.createElement("span");
    nameSpan.className = "repo-list-name";
    nameSpan.textContent = r.full_name;
    nameSpan.title = r.full_name;
    li.appendChild(nameSpan);

    // Remove button (nice-to-have)
    const removeBtn = document.createElement("button");
    removeBtn.className = "repo-remove-btn";
    removeBtn.title = `Remove ${r.full_name}`;
    removeBtn.textContent = "×";
    removeBtn.addEventListener("click", (e) => {
      e.stopPropagation();
      handleRemoveTrackedRepo(r.full_name);
    });
    li.appendChild(removeBtn);

    li.addEventListener("click", () => selectTrackedRepo(r));
    repoList.appendChild(li);
  });

  // Re-apply selected state if selectedRepo is in the list
  if (selectedRepo) {
    Array.from(repoList.children).forEach((li, idx) => {
      if (filtered[idx] && filtered[idx].full_name === `${selectedRepo.owner}/${selectedRepo.name}`) {
        li.classList.add("selected");
      }
    });
  }
}
```

### 5.5 Modify `repoSearch` Event Handler

The existing `repoSearch.addEventListener("input", ...)` currently filters the `repos` array into a call to `renderRepoList`. Update it to filter `trackedRepos`:

**Current:**
```javascript
repoSearch.addEventListener("input", () => {
  const q = repoSearch.value.toLowerCase();
  renderRepoList(repos.filter((r) => r.full_name.toLowerCase().includes(q)));
});
```

**New:**
```javascript
repoSearch.addEventListener("input", () => {
  renderTrackedRepoList(trackedRepos);
});
```

(Filtering is handled inside `renderTrackedRepoList` itself.)

### 5.6 New Function: `selectTrackedRepo(repo)`

Similar to the existing `selectRepo()` but works with `TrackedRepo` (which has the same `owner` and `name` fields):

```javascript
function selectTrackedRepo(repo) {
  selectedRepo = { owner: repo.owner, name: repo.name };
  Array.from(repoList.children).forEach(li => li.classList.remove("selected"));
  // Find the matching li by full_name text
  Array.from(repoList.children).forEach(li => {
    const span = li.querySelector(".repo-list-name");
    if (span && span.textContent === repo.full_name) {
      li.classList.add("selected");
    }
  });
  refreshData();
}
```

### 5.7 New Function: `openAddRepoModal()`

```javascript
async function openAddRepoModal() {
  const modal = document.getElementById("add-repo-modal");
  const searchInput = document.getElementById("add-repo-search");
  const listEl = document.getElementById("add-repo-list");
  const errorEl = document.getElementById("add-repo-error");

  modal.classList.remove("hidden");
  searchInput.value = "";
  errorEl.classList.add("hidden");

  // Lazy-load the full repo list (once per session or after account switch)
  if (!pickerLoaded) {
    listEl.innerHTML = '<li class="add-repo-loading"><span class="spinner-small"></span> Loading repositories…</li>';
    try {
      allRepos = await invoke("list_all_repos");
      pickerLoaded = true;
    } catch (e) {
      listEl.innerHTML = `<li class="add-repo-error-item">Failed to load repositories: ${esc(String(e))}</li>`;
      return;
    }
  }

  renderPickerList(allRepos, searchInput.value);
  searchInput.focus();
}
```

### 5.8 New Function: `renderPickerList(repos, query)`

```javascript
function renderPickerList(repos, query) {
  const listEl = document.getElementById("add-repo-list");
  const q = (query || "").toLowerCase();
  const filtered = q
    ? repos.filter(r => r.full_name.toLowerCase().includes(q))
    : repos;

  const trackedSet = new Set(trackedRepos.map(r => r.full_name));

  listEl.innerHTML = "";

  if (filtered.length === 0) {
    listEl.innerHTML = '<li class="add-repo-empty">No repositories found.</li>';
    return;
  }

  filtered.forEach(r => {
    const li = document.createElement("li");
    li.className = "add-repo-item";
    const alreadyTracked = trackedSet.has(r.full_name);
    if (alreadyTracked) li.classList.add("add-repo-item-tracked");

    li.innerHTML = `
      <span class="add-repo-item-name">${esc(r.full_name)}</span>
      ${r.description ? `<span class="add-repo-item-desc">${esc(r.description)}</span>` : ""}
      ${alreadyTracked ? '<span class="add-repo-item-check" aria-label="Already tracked">✓</span>' : ""}
    `;

    if (!alreadyTracked) {
      li.addEventListener("click", () => handleAddTrackedRepo(r));
    }

    listEl.appendChild(li);
  });
}
```

### 5.9 New Function: `handleAddTrackedRepo(repo)`

```javascript
async function handleAddTrackedRepo(repo) {
  const errorEl = document.getElementById("add-repo-error");
  errorEl.classList.add("hidden");
  try {
    trackedRepos = await invoke("add_tracked_repo", {
      fullName: repo.full_name,
      owner: repo.owner,
      name: repo.name,
    });
    // Close modal and refresh sidebar
    document.getElementById("add-repo-modal").classList.add("hidden");
    renderTrackedRepoList(trackedRepos);
    // Auto-select the newly added repo
    const added = trackedRepos.find(r => r.full_name === repo.full_name);
    if (added) selectTrackedRepo(added);
  } catch (e) {
    errorEl.textContent = `Failed to add repository: ${esc(String(e))}`;
    errorEl.classList.remove("hidden");
  }
}
```

### 5.10 New Function: `handleRemoveTrackedRepo(fullName)`

```javascript
async function handleRemoveTrackedRepo(fullName) {
  try {
    trackedRepos = await invoke("remove_tracked_repo", { fullName });
    // If the removed repo was selected, clear the selection
    if (selectedRepo && `${selectedRepo.owner}/${selectedRepo.name}` === fullName) {
      selectedRepo = null;
      issues = []; pulls = []; alerts = [];
      placeholder.classList.remove("hidden");
    }
    renderTrackedRepoList(trackedRepos);
  } catch (e) {
    alert(`Failed to remove repository: ${e}`);
  }
}
```

### 5.11 Event Handler: "Add Repository" Button

```javascript
document.getElementById("add-repo-btn").addEventListener("click", openAddRepoModal);
```

### 5.12 Event Handler: Modal Close Button + Overlay Click

```javascript
document.getElementById("add-repo-close-btn").addEventListener("click", () => {
  document.getElementById("add-repo-modal").classList.add("hidden");
});

// Close on overlay click (not modal card click)
document.getElementById("add-repo-modal").addEventListener("click", (e) => {
  if (e.target === document.getElementById("add-repo-modal")) {
    document.getElementById("add-repo-modal").classList.add("hidden");
  }
});
```

### 5.13 Event Handler: Picker Search Input

```javascript
document.getElementById("add-repo-search").addEventListener("input", (e) => {
  renderPickerList(allRepos, e.target.value);
});
```

### 5.14 Account Switch — Reset on `handleSwitchAccount`

The existing `handleSwitchAccount` already resets `repos = []` and calls `loadRepos()`. Update it to:
1. Set `pickerLoaded = false` and `allRepos = []`
2. Call `loadTrackedRepos()` instead of `loadRepos()`

### 5.15 Keep `loadRepos()` and `renderRepoList()` intact

The existing `loadRepos()` and `renderRepoList()` functions should be **retained** (not deleted) to avoid breaking mock mode, which relies on `list_repos`. The mock's `showApp` path will still call `loadRepos()` since mock mode doesn't implement `get_tracked_repos` (or it returns an empty list).

---

## 6. HTML Changes

### 6.1 Sidebar — Add "Add Repository" Button

**File:** `src/index.html`

Locate the sidebar section:
```html
<aside id="sidebar">
  <div class="sidebar-header">
    ...
  </div>
  <h3>Repositories</h3>
  <input id="repo-search" type="text" placeholder="Filter repos…" />
  <ul id="repo-list"></ul>
</aside>
```

Insert the "Add Repository" button **between** `<h3>Repositories</h3>` and `<input id="repo-search">`:

```html
<aside id="sidebar">
  <div class="sidebar-header">
    ...
  </div>
  <h3>Repositories</h3>
  <button id="add-repo-btn" class="btn-add-repo">+ Add Repository</button>
  <input id="repo-search" type="text" placeholder="Filter repos…" />
  <ul id="repo-list"></ul>
</aside>
```

### 6.2 New Modal: "Add Repository" Picker

Add the following modal **after** the `#add-account-modal` div and **before** the `<script>` tags at the bottom of `<body>`:

```html
<!-- Add Repository modal overlay -->
<div id="add-repo-modal" class="modal-overlay hidden" role="dialog" aria-modal="true" aria-labelledby="add-repo-title">
  <div class="modal-card modal-card-large">
    <div class="modal-header">
      <h2 id="add-repo-title" class="modal-title">Add Repository</h2>
      <button id="add-repo-close-btn" class="modal-close-btn" aria-label="Close">×</button>
    </div>
    <p class="modal-subtitle">Select a repository to track in your sidebar.</p>
    <input id="add-repo-search" type="text" placeholder="Search repositories…" class="pat-input" autocomplete="off" spellcheck="false" />
    <div id="add-repo-error" class="login-error hidden"></div>
    <ul id="add-repo-list" class="add-repo-list"></ul>
  </div>
</div>
```

---

## 7. CSS Changes

**File:** `src/styles.css`

### 7.1 "Add Repository" Button

```css
/* ── Add Repository button ───────────────────── */
.btn-add-repo {
  display: block;
  width: 100%;
  padding: 0.4rem 0.6rem;
  margin-bottom: 0.5rem;
  background: transparent;
  border: 1px dashed var(--border);
  border-radius: var(--radius);
  color: var(--accent);
  font-size: 0.85rem;
  text-align: left;
  cursor: pointer;
  transition: background 0.15s, border-color 0.15s;
}

.btn-add-repo:hover {
  background: rgba(88, 166, 255, 0.08);
  border-color: var(--accent);
}
```

### 7.2 Repo List Item with Remove Button

The current `#repo-list li` style handles basic items. We need to add styles for the new `repo-list-item` structure (name + remove button):

```css
/* ── Repo list items (tracked repos) ────────── */
.repo-list-item {
  display: flex;
  align-items: center;
  padding: 0.45rem 0.6rem;
  border-radius: var(--radius);
  cursor: pointer;
  font-size: 0.9rem;
}

.repo-list-item:hover { background: var(--border); }
.repo-list-item.selected { background: var(--accent); color: #fff; }
.repo-list-item.selected .repo-remove-btn { color: rgba(255,255,255,0.7); }

.repo-list-name {
  flex: 1;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.repo-remove-btn {
  flex-shrink: 0;
  background: none;
  border: none;
  color: var(--text-muted);
  cursor: pointer;
  font-size: 1rem;
  line-height: 1;
  padding: 0 2px;
  border-radius: 3px;
  opacity: 0;
  transition: opacity 0.1s, color 0.1s;
}

.repo-list-item:hover .repo-remove-btn { opacity: 1; }
.repo-remove-btn:hover { color: var(--red); }

.repo-list-empty {
  padding: 0.6rem;
  font-size: 0.85rem;
  color: var(--text-muted);
  list-style: none;
  font-style: italic;
}
```

### 7.3 "Add Repository" Modal — Large Modal Card Variant

The existing `.modal-card` is `width: 400px`. The repo picker needs to be taller with a scrollable list:

```css
/* ── Large modal card (for Add Repository picker) ── */
.modal-card-large {
  width: 480px;
  max-height: 75vh;
  display: flex;
  flex-direction: column;
}

.modal-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 4px;
}

.modal-close-btn {
  background: none;
  border: none;
  color: var(--text-muted);
  font-size: 1.4rem;
  cursor: pointer;
  line-height: 1;
  padding: 0 4px;
  border-radius: 3px;
  transition: color 0.1s;
}

.modal-close-btn:hover { color: var(--text); }

/* ── Add Repository picker list ─────────────── */
.add-repo-list {
  list-style: none;
  overflow-y: auto;
  flex: 1;
  margin-top: 8px;
  border: 1px solid var(--border);
  border-radius: var(--radius);
}

.add-repo-item {
  display: flex;
  flex-direction: column;
  padding: 0.55rem 0.75rem;
  cursor: pointer;
  border-bottom: 1px solid var(--border);
  gap: 2px;
}

.add-repo-item:last-child { border-bottom: none; }
.add-repo-item:hover { background: rgba(88, 166, 255, 0.08); }

.add-repo-item-tracked {
  cursor: default;
  opacity: 0.55;
}

.add-repo-item-tracked:hover { background: none; }

.add-repo-item-name {
  font-size: 0.9rem;
  font-weight: 500;
  color: var(--text);
}

.add-repo-item-desc {
  font-size: 0.78rem;
  color: var(--text-muted);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.add-repo-item-check {
  font-size: 0.75rem;
  color: var(--green);
  font-weight: 700;
  margin-left: auto;
  align-self: flex-start;
  padding-top: 2px;
}

.add-repo-loading,
.add-repo-empty,
.add-repo-error-item {
  padding: 1rem;
  text-align: center;
  color: var(--text-muted);
  font-size: 0.88rem;
  list-style: none;
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 8px;
}

.add-repo-error-item { color: var(--red); }
```

---

## 8. Mock Module Changes

**File:** `src-tauri/src/mock/mod.rs`

The mock module needs implementations of the three new commands so the `dev-mock` build compiles. Since mock mode runs in memory with no real persistence, these use in-memory state via a simple approach.

Add a `tracked_repos` field to an in-memory state, OR use a simple `Vec` wrapped in a `Mutex` stored as managed state. The cleanest approach for mock mode is to add `tracked_repos` to the mock `AppState` via a separate managed state, but since `AppState` is shared, we should add the field conditionally.

**Recommended approach for mock:** Add `tracked_repos: Vec<TrackedRepo>` to `AppState` under `#[cfg(feature = "dev-mock")]` cfg attributes. This keeps the production `AppState` clean.

Alternative (simpler, no `AppState` change): Use a `Mutex<Vec<TrackedRepo>>` registered as a separate managed state just for mock mode. However, this requires changes to `main.rs` to register this state in mock builds.

**Simplest approach:** For mock mode, implement in-memory tracked repos directly in `AppState` with a `#[cfg(feature = "dev-mock")]` conditional field, OR use `thread_local!` storage in the mock module.

The spec recommends adding to `AppState`:

```rust
// In src-tauri/src/models/mod.rs
#[derive(Default)]
pub struct AppState {
    pub client: Option<Octocrab>,
    pub token: Option<String>,
    pub username: Option<String>,
    pub active_account_id: Option<String>,
    pub accounts: Vec<Account>,
    #[cfg(feature = "dev-mock")]
    pub tracked_repos: Vec<TrackedRepo>,
}
```

Mock commands in `src-tauri/src/mock/mod.rs`:

```rust
#[tauri::command]
pub fn get_tracked_repos(
    state: tauri::State<'_, Mutex<AppState>>,
) -> Result<Vec<crate::models::TrackedRepo>, String> {
    let app = state.lock().map_err(|e| e.to_string())?;
    Ok(app.tracked_repos.clone())
}

#[tauri::command]
pub fn add_tracked_repo(
    full_name: String,
    owner: String,
    name: String,
    state: tauri::State<'_, Mutex<AppState>>,
) -> Result<Vec<crate::models::TrackedRepo>, String> {
    let mut app = state.lock().map_err(|e| e.to_string())?;
    if !app.tracked_repos.iter().any(|r| r.full_name == full_name) {
        app.tracked_repos.push(crate::models::TrackedRepo { full_name, owner, name });
    }
    Ok(app.tracked_repos.clone())
}

#[tauri::command]
pub fn remove_tracked_repo(
    full_name: String,
    state: tauri::State<'_, Mutex<AppState>>,
) -> Result<Vec<crate::models::TrackedRepo>, String> {
    let mut app = state.lock().map_err(|e| e.to_string())?;
    app.tracked_repos.retain(|r| r.full_name != full_name);
    Ok(app.tracked_repos.clone())
}

#[tauri::command]
pub fn list_all_repos(
    _state: tauri::State<'_, Mutex<AppState>>,
) -> Result<Vec<crate::models::Repo>, String> {
    // Return the same mock repos as list_repos
    list_repos(_state)
}
```

Register in `main.rs` mock handler:
```rust
mock::get_tracked_repos,
mock::add_tracked_repo,
mock::remove_tracked_repo,
mock::list_all_repos,
```

---

## 9. Step-by-Step Implementation Order

### Phase 1: Models (Rust)

1. **`src-tauri/src/models/mod.rs`**
   - Add `TrackedRepo` struct (with `Serialize`, `Deserialize`, `Clone`, `Debug`, `PartialEq`)
   - Add `#[cfg(feature = "dev-mock")] pub tracked_repos: Vec<TrackedRepo>` to `AppState`

### Phase 2: Storage Module (Rust)

2. **Create `src-tauri/src/storage.rs`**
   - Implement `tracked_repos_path()`, `load_all()`, `save_all()`, `load_tracked_repos()`, `save_tracked_repos()`

### Phase 3: GitHub Fetch Function (Rust)

3. **`src-tauri/src/github/issues.rs`**
   - Add `pub async fn list_all_repos(client: &Octocrab) -> Result<Vec<Repo>>`

### Phase 4: New Tauri Commands (Rust)

4. **`src-tauri/src/main.rs`**
   - Add `mod storage;`
   - Add `use models::TrackedRepo;` (if needed for type annotation)
   - Implement `get_tracked_repos`, `add_tracked_repo`, `remove_tracked_repo`, `list_all_repos` commands
   - Register all 4 in the non-mock `invoke_handler![]`

### Phase 5: Mock Module (Rust)

5. **`src-tauri/src/mock/mod.rs`**
   - Add `get_tracked_repos`, `add_tracked_repo`, `remove_tracked_repo`, `list_all_repos` mock commands
   - Register in `main.rs` mock handler

### Phase 6: HTML (Frontend)

6. **`src/index.html`**
   - Add `<button id="add-repo-btn" class="btn-add-repo">+ Add Repository</button>` in sidebar
   - Add `#add-repo-modal` overlay at bottom of `<body>`

### Phase 7: CSS (Frontend)

7. **`src/styles.css`**
   - Add `.btn-add-repo` styles
   - Add `.repo-list-item`, `.repo-list-name`, `.repo-remove-btn`, `.repo-list-empty` styles
   - Add `.modal-card-large`, `.modal-header`, `.modal-close-btn` styles
   - Add `.add-repo-list`, `.add-repo-item`, `.add-repo-item-*` styles

### Phase 8: JavaScript (Frontend)

8. **`src/main.js`**
   - Add `trackedRepos`, `allRepos`, `pickerLoaded` state variables
   - Add `loadTrackedRepos()`, `renderTrackedRepoList()`, `selectTrackedRepo()` functions
   - Add `openAddRepoModal()`, `renderPickerList()`, `handleAddTrackedRepo()`, `handleRemoveTrackedRepo()` functions
   - Modify `showApp()` to call `loadTrackedRepos()` instead of `loadRepos()`
   - Modify `repoSearch` input handler to call `renderTrackedRepoList(trackedRepos)`
   - Modify `handleSwitchAccount()` to reset `pickerLoaded`/`allRepos` and call `loadTrackedRepos()`
   - Add event listeners for `#add-repo-btn`, `#add-repo-close-btn`, `#add-repo-modal` (overlay click), `#add-repo-search`

---

## 10. File Paths and Function Names Summary

### New Files

| File | Purpose |
|------|---------|
| `src-tauri/src/storage.rs` | File-system persistence helpers for tracked repos |

### Modified Files

| File | Changes |
|------|---------|
| `src-tauri/src/models/mod.rs` | Add `TrackedRepo` struct; add `tracked_repos` field to `AppState` (mock-only) |
| `src-tauri/src/main.rs` | Add `mod storage;`; add 4 new Tauri commands; register commands |
| `src-tauri/src/github/issues.rs` | Add `list_all_repos()` function |
| `src-tauri/src/mock/mod.rs` | Add 4 mock command implementations |
| `src/index.html` | Add `#add-repo-btn` button; add `#add-repo-modal` HTML |
| `src/styles.css` | Add ~80 lines of new CSS for button and modal |
| `src/main.js` | Add ~120 lines of new JS logic; modify `showApp()`, `repoSearch` handler, `handleSwitchAccount()` |

### New Rust Functions

| Function | Location |
|----------|---------|
| `storage::tracked_repos_path()` | `src-tauri/src/storage.rs` |
| `storage::load_all()` | `src-tauri/src/storage.rs` |
| `storage::save_all()` | `src-tauri/src/storage.rs` |
| `storage::load_tracked_repos()` | `src-tauri/src/storage.rs` |
| `storage::save_tracked_repos()` | `src-tauri/src/storage.rs` |
| `github::issues::list_all_repos()` | `src-tauri/src/github/issues.rs` |
| `get_tracked_repos` (Tauri cmd) | `src-tauri/src/main.rs` |
| `add_tracked_repo` (Tauri cmd) | `src-tauri/src/main.rs` |
| `remove_tracked_repo` (Tauri cmd) | `src-tauri/src/main.rs` |
| `list_all_repos` (Tauri cmd) | `src-tauri/src/main.rs` |
| `mock::get_tracked_repos` | `src-tauri/src/mock/mod.rs` |
| `mock::add_tracked_repo` | `src-tauri/src/mock/mod.rs` |
| `mock::remove_tracked_repo` | `src-tauri/src/mock/mod.rs` |
| `mock::list_all_repos` | `src-tauri/src/mock/mod.rs` |

### New JS Functions

| Function | Location |
|----------|---------|
| `loadTrackedRepos()` | `src/main.js` |
| `renderTrackedRepoList(list)` | `src/main.js` |
| `selectTrackedRepo(repo)` | `src/main.js` |
| `openAddRepoModal()` | `src/main.js` |
| `renderPickerList(repos, query)` | `src/main.js` |
| `handleAddTrackedRepo(repo)` | `src/main.js` |
| `handleRemoveTrackedRepo(fullName)` | `src/main.js` |

---

## 11. JSON Schema for Persistence File

**File:** `<app_data_dir>/tracked_repos.json`

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "type": "object",
  "description": "Map of account_id (UUID string) to array of tracked repos",
  "additionalProperties": {
    "type": "array",
    "items": {
      "type": "object",
      "required": ["full_name", "owner", "name"],
      "properties": {
        "full_name": {
          "type": "string",
          "description": "owner/repo format, e.g. octocat/Hello-World",
          "pattern": "^[\\w.-]+/[\\w.-]+$"
        },
        "owner": {
          "type": "string",
          "description": "GitHub user or organization login",
          "pattern": "^[\\w.-]+$"
        },
        "name": {
          "type": "string",
          "description": "Repository name without owner prefix",
          "pattern": "^[\\w.-]+$"
        }
      },
      "additionalProperties": false
    }
  }
}
```

**Example file content:**

```json
{
  "a1b2c3d4-1234-5678-90ab-cdef01234567": [
    {
      "full_name": "octocat/Hello-World",
      "owner": "octocat",
      "name": "Hello-World"
    },
    {
      "full_name": "rust-lang/rust",
      "owner": "rust-lang",
      "name": "rust"
    }
  ]
}
```

---

## 12. Risks and Mitigations

| Risk | Severity | Mitigation |
|------|----------|-----------|
| **`app_data_dir` returns `None`** on some platforms | Medium | Wrap in `.context(...)`, return empty list as graceful fallback. Log the error with `eprintln!`. |
| **Corrupted JSON file** prevents app from loading repos | Low | `serde_json::from_str().unwrap_or_default()` — returns empty map, allowing normal operation. |
| **GitHub API `list_all_repos` returns max 100** — users with >100 repos won't see all of them in the picker | Medium | Document this limitation clearly in the modal UI ("Showing up to 100 most recently updated repositories"). A future enhancement can add pagination or a server-side search. |
| **Stale repo data** — tracked repo is deleted or renamed on GitHub | Low | The tracked entry remains in the file. When the user selects it, the API calls (`fetch_issues`, etc.) will fail with a clear error. The user can remove it manually via the ×button. A future enhancement could validate tracked repos on startup. |
| **Injection via `owner`/`name` fields** passed to `add_tracked_repo` | Low (mitigated) | The command validates that `full_name == format!("{owner}/{name}")` and enforces `[a-zA-Z0-9._-]+` character set on both fields before persisting. |
| **Concurrency: multiple windows / rapid add calls** | Very Low | `std::fs::read` + `std::fs::write` are used synchronously; Tauri commands share the same thread pool. The `storage::save_all` function performs a read-modify-write, which could theoretically race in very unusual multi-window scenarios. For v1, this is acceptable — the app is designed for single-window use. |
| **Dev-mock mode**: `AppState` gains a `tracked_repos` field | Low | Gated with `#[cfg(feature = "dev-mock")]` — zero overhead in production builds. |
| **`tauri::api::path::app_data_dir` API availability** | None | Confirmed available in Tauri v1 — used by many Tauri v1 apps. |
| **`tauri.conf.json` allowlist changes needed for file I/O** | None | The Tauri `allowlist` only governs JS-side APIs (`tauri.fs`). Rust commands using `std::fs` directly have no allowlist restrictions. No `tauri.conf.json` changes are needed. |
| **Remove button accidentally triggered during repo click** | Low | The remove button's click handler calls `e.stopPropagation()` to prevent it from bubbling up to the parent `<li>` click handler. |

---

## 13. Nice-to-Have: Remove Tracked Repo

The remove feature is fully specified above and should be implemented as part of this feature (not deferred), because:

1. The UI (`×` button on each sidebar item) is simple to add.
2. The Rust command (`remove_tracked_repo`) is ~10 lines.
3. The JS handler (`handleRemoveTrackedRepo`) is ~10 lines.
4. Without it, the only way to "reset" the sidebar is to delete the data file manually.

The remove button is **hidden by default** and only appears on hover (`opacity: 0` → `opacity: 1`), matching GitHub Desktop's UX pattern. It turns red on hover to signal destructive intent.

---

## Summary

This spec describes a full "tracked repositories" feature that transforms the sidebar from an auto-populated API dump into a user-curated, persistent list — matching the GitHub Desktop interaction model.

**Scope:**
- 1 new Rust file (`storage.rs`)
- ~50 lines of new Rust (models + storage + 4 commands)
- ~120 lines of new JS (7 new functions + wiring)
- ~25 lines of new HTML (1 button + 1 modal)
- ~80 lines of new CSS (button + modal + list items)
- 4 mock command stubs

**No breaking changes** to existing commands (`list_repos`, `fetch_issues`, etc.) or the login/auth flow.
