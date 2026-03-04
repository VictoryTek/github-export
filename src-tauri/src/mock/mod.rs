// src-tauri/src/mock/mod.rs
//
// Mock implementations of every GitHub-calling Tauri command.
// This module is compiled **only** when the `dev-mock` feature is active
// (gated by `#[cfg(feature = "dev-mock")] mod mock;` in main.rs).
// Zero code ships in production builds.
//
// Activate via: npm run dev:mock

use crate::models::{AppState, FilterParams, Issue, PullRequest, Repo, SecurityAlert};
use chrono::DateTime;
use std::sync::Mutex;

// ── Helper ─────────────────────────────────────────────────────────────────

fn dt(rfc3339: &str) -> DateTime<chrono::Utc> {
    DateTime::parse_from_rfc3339(rfc3339)
        .expect("hard-coded datetime must be valid RFC-3339")
        .with_timezone(&chrono::Utc)
}

// ── Mock Tauri Commands ────────────────────────────────────────────────────

/// Auto-login as "octocat" — bypasses the GitHub OAuth flow entirely.
#[tauri::command]
pub fn restore_session(
    state: tauri::State<'_, Mutex<AppState>>,
) -> Result<Option<String>, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    s.token = Some("mock-token-dev".to_string());
    s.username = Some("octocat".to_string());
    Ok(Some("octocat".to_string()))
}

/// Returns three realistic fake repositories.
#[tauri::command]
pub fn list_repos(
    _state: tauri::State<'_, Mutex<AppState>>,
) -> Result<Vec<Repo>, String> {
    Ok(vec![
        Repo {
            id: 1_296_269,
            name: "Hello-World".to_string(),
            full_name: "octocat/Hello-World".to_string(),
            owner: "octocat".to_string(),
            description: Some("My first repository on GitHub!".to_string()),
            private: false,
            html_url: "https://github.com/octocat/Hello-World".to_string(),
            open_issues_count: 5,
        },
        Repo {
            id: 1_300_192,
            name: "Spoon-Knife".to_string(),
            full_name: "octocat/Spoon-Knife".to_string(),
            owner: "octocat".to_string(),
            description: Some(
                "This repo is for demonstration purposes only.".to_string(),
            ),
            private: false,
            html_url: "https://github.com/octocat/Spoon-Knife".to_string(),
            open_issues_count: 1_843,
        },
        Repo {
            id: 1_364_490,
            name: "linguist".to_string(),
            full_name: "octocat/linguist".to_string(),
            owner: "octocat".to_string(),
            description: Some(
                "Language Savant. If your repository's language is wrong, send us a pull request!"
                    .to_string(),
            ),
            private: false,
            html_url: "https://github.com/octocat/linguist".to_string(),
            open_issues_count: 12,
        },
    ])
}

/// Returns five realistic fake issues.
#[tauri::command]
pub fn fetch_issues(
    _owner: String,
    _repo: String,
    _filters: Option<FilterParams>,
    _state: tauri::State<'_, Mutex<AppState>>,
) -> Result<Vec<Issue>, String> {
    Ok(vec![
        Issue {
            number: 42,
            title: "Fix null pointer dereference in auth module".to_string(),
            state: "open".to_string(),
            author: "monalisa".to_string(),
            labels: vec!["bug".to_string(), "priority: high".to_string()],
            assignees: vec!["octocat".to_string()],
            created_at: dt("2025-11-01T09:15:00Z"),
            updated_at: dt("2025-11-15T14:32:00Z"),
            closed_at: None,
            html_url: "https://github.com/octocat/Hello-World/issues/42".to_string(),
            body: Some(
                "Observed a null pointer dereference when the auth token is expired."
                    .to_string(),
            ),
        },
        Issue {
            number: 57,
            title: "Add dark mode support".to_string(),
            state: "open".to_string(),
            author: "hubot".to_string(),
            labels: vec!["enhancement".to_string()],
            assignees: vec![],
            created_at: dt("2025-12-03T11:00:00Z"),
            updated_at: dt("2025-12-10T08:45:00Z"),
            closed_at: None,
            html_url: "https://github.com/octocat/Hello-World/issues/57".to_string(),
            body: Some("Users have requested a dark mode toggle in the settings panel.".to_string()),
        },
        Issue {
            number: 63,
            title: "Pagination breaks when repository has > 1000 issues".to_string(),
            state: "open".to_string(),
            author: "defunkt".to_string(),
            labels: vec!["bug".to_string(), "good first issue".to_string()],
            assignees: vec!["monalisa".to_string()],
            created_at: dt("2026-01-07T16:20:00Z"),
            updated_at: dt("2026-01-09T10:00:00Z"),
            closed_at: None,
            html_url: "https://github.com/octocat/Hello-World/issues/63".to_string(),
            body: Some("The cursor-based pagination implementation stops at page 10.".to_string()),
        },
        Issue {
            number: 71,
            title: "Improve error messages for rate-limit responses".to_string(),
            state: "closed".to_string(),
            author: "octocat".to_string(),
            labels: vec!["enhancement".to_string(), "documentation".to_string()],
            assignees: vec![],
            created_at: dt("2026-01-20T08:05:00Z"),
            updated_at: dt("2026-02-01T12:30:00Z"),
            closed_at: Some(dt("2026-02-01T12:30:00Z")),
            html_url: "https://github.com/octocat/Hello-World/issues/71".to_string(),
            body: Some(
                "Rate-limit errors currently surface raw HTTP 429 text to the user.".to_string(),
            ),
        },
        Issue {
            number: 78,
            title: "CSV export omits assignees column".to_string(),
            state: "open".to_string(),
            author: "torvalds".to_string(),
            labels: vec!["bug".to_string()],
            assignees: vec!["octocat".to_string()],
            created_at: dt("2026-02-14T13:45:00Z"),
            updated_at: dt("2026-02-14T17:00:00Z"),
            closed_at: None,
            html_url: "https://github.com/octocat/Hello-World/issues/78".to_string(),
            body: Some("When exporting issues to CSV the assignees column is empty for all rows.".to_string()),
        },
    ])
}

/// Returns three realistic fake pull requests.
#[tauri::command]
pub fn fetch_pulls(
    _owner: String,
    _repo: String,
    _filters: Option<FilterParams>,
    _state: tauri::State<'_, Mutex<AppState>>,
) -> Result<Vec<PullRequest>, String> {
    Ok(vec![
        PullRequest {
            number: 101,
            title: "feat: implement OAuth device flow".to_string(),
            state: "merged".to_string(),
            author: "octocat".to_string(),
            labels: vec!["feature".to_string()],
            reviewers: vec!["monalisa".to_string(), "hubot".to_string()],
            head_branch: "feat/oauth-device-flow".to_string(),
            base_branch: "main".to_string(),
            created_at: dt("2025-10-10T10:00:00Z"),
            updated_at: dt("2025-10-20T14:00:00Z"),
            merged_at: Some(dt("2025-10-20T14:00:00Z")),
            closed_at: Some(dt("2025-10-20T14:00:00Z")),
            html_url: "https://github.com/octocat/Hello-World/pull/101".to_string(),
            draft: false,
            body: Some("Implements the GitHub OAuth Device Flow as per RFC 8628.".to_string()),
        },
        PullRequest {
            number: 115,
            title: "fix: resolve memory leak in parser".to_string(),
            state: "open".to_string(),
            author: "defunkt".to_string(),
            labels: vec!["bug".to_string(), "priority: high".to_string()],
            reviewers: vec!["octocat".to_string()],
            head_branch: "fix/parser-memory-leak".to_string(),
            base_branch: "main".to_string(),
            created_at: dt("2026-02-01T09:30:00Z"),
            updated_at: dt("2026-02-20T11:15:00Z"),
            merged_at: None,
            closed_at: None,
            html_url: "https://github.com/octocat/Hello-World/pull/115".to_string(),
            draft: false,
            body: Some(
                "The Markdown parser was retaining references to stale AST nodes. This PR frees them on drop.".to_string(),
            ),
        },
        PullRequest {
            number: 122,
            title: "chore: upgrade octocrab to 0.38 and fix breaking API changes".to_string(),
            state: "open".to_string(),
            author: "monalisa".to_string(),
            labels: vec!["dependencies".to_string()],
            reviewers: vec![],
            head_branch: "chore/octocrab-0.38".to_string(),
            base_branch: "main".to_string(),
            created_at: dt("2026-02-28T15:00:00Z"),
            updated_at: dt("2026-03-01T08:00:00Z"),
            merged_at: None,
            closed_at: None,
            html_url: "https://github.com/octocat/Hello-World/pull/122".to_string(),
            draft: true,
            body: Some("Draft: still working through the API surface changes in the new version.".to_string()),
        },
    ])
}

/// Returns two realistic fake Dependabot security alerts.
#[tauri::command]
pub fn fetch_security_alerts(
    _owner: String,
    _repo: String,
    _state: tauri::State<'_, Mutex<AppState>>,
) -> Result<Vec<SecurityAlert>, String> {
    Ok(vec![
        SecurityAlert {
            id: 1,
            severity: "high".to_string(),
            summary: "lodash vulnerable to prototype pollution (CVE-2021-23337)".to_string(),
            description: "Versions of lodash prior to 4.17.21 are vulnerable to \
                          command injection via the template and bindAll functions."
                .to_string(),
            package_name: Some("lodash".to_string()),
            vulnerable_version_range: Some("< 4.17.21".to_string()),
            patched_version: Some("4.17.21".to_string()),
            state: "open".to_string(),
            html_url: "https://github.com/octocat/Hello-World/security/dependabot/1"
                .to_string(),
            created_at: dt("2025-09-15T12:00:00Z"),
        },
        SecurityAlert {
            id: 2,
            severity: "critical".to_string(),
            summary: "follow-redirects improperly handles URLs (CVE-2024-28849)".to_string(),
            description: "follow-redirects before 1.15.6 allows a bypass of the \
                          no-auth redirect protection via a specially crafted URL."
                .to_string(),
            package_name: Some("follow-redirects".to_string()),
            vulnerable_version_range: Some("< 1.15.6".to_string()),
            patched_version: Some("1.15.6".to_string()),
            state: "open".to_string(),
            html_url: "https://github.com/octocat/Hello-World/security/dependabot/2"
                .to_string(),
            created_at: dt("2026-01-10T08:30:00Z"),
        },
    ])
}

/// Always returns `true` — the frontend uses this to display the dev-mode banner.
#[tauri::command]
pub fn get_dev_mode() -> bool {
    true
}
