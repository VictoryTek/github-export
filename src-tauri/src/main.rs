// Prevents additional console window on Windows in release builds
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
// In dev-mock builds the real GitHub functions are intentionally unused.
#![cfg_attr(feature = "dev-mock", allow(dead_code, unused_imports))]

mod export;
mod github;
#[cfg(feature = "dev-mock")]
mod mock;
mod models;
mod storage;

use github::auth::{
    add_account, authenticate_with_pat, delete_active_account_id, list_accounts, poll_device_flow,
    remove_account, start_device_flow, switch_account,
};
// restore_session is mocked in dev-mock builds, so only import it in non-mock builds.
#[cfg(not(feature = "dev-mock"))]
use github::auth::restore_session;
#[cfg(not(feature = "dev-mock"))]
use models::FilterParams;
use models::{AppState, ExportFormat};
use std::sync::Mutex;
use tauri::State;

// ──────────────────────────────────────────────
// Dev mode flag (non-mock build always returns false)
// ──────────────────────────────────────────────

#[cfg(not(feature = "dev-mock"))]
#[tauri::command]
fn get_dev_mode() -> bool {
    false
}

// ──────────────────────────────────────────────
// Tauri commands exposed to the frontend
// ──────────────────────────────────────────────

/// Disconnect the current session without removing stored accounts.
#[tauri::command]
fn logout(state: State<'_, Mutex<AppState>>) -> Result<(), String> {
    let mut app = state.lock().map_err(|e| e.to_string())?;
    app.client = None;
    app.token = None;
    app.username = None;
    app.active_account_id = None;
    let _ = delete_active_account_id();
    Ok(())
}

/// List repositories visible to the authenticated user.
#[cfg(not(feature = "dev-mock"))]
#[tauri::command]
async fn list_repos(state: State<'_, Mutex<AppState>>) -> Result<Vec<models::Repo>, String> {
    let client = {
        let app = state.lock().map_err(|e| e.to_string())?;
        app.client.clone().ok_or("Not authenticated")?
    };
    github::issues::list_repos(&client)
        .await
        .map_err(|e| e.to_string())
}

/// Return the tracked repository list for the currently active account.
#[cfg(not(feature = "dev-mock"))]
#[tauri::command]
fn get_tracked_repos(
    app_handle: tauri::AppHandle,
    state: State<'_, Mutex<AppState>>,
) -> Result<Vec<models::TrackedRepo>, String> {
    let account_id = {
        let app = state.lock().map_err(|e| e.to_string())?;
        app.active_account_id
            .clone()
            .unwrap_or_else(|| "default".to_string())
    };
    Ok(storage::load_tracked_repos(&app_handle, &account_id))
}

/// Add a repository to the tracked list for the currently active account.
/// Returns the updated tracked list. Idempotent — adding an already-tracked
/// repo simply returns the current list without error.
#[cfg(not(feature = "dev-mock"))]
#[tauri::command]
fn add_tracked_repo(
    full_name: String,
    owner: String,
    name: String,
    app_handle: tauri::AppHandle,
    state: State<'_, Mutex<AppState>>,
) -> Result<Vec<models::TrackedRepo>, String> {
    // Validate that full_name == "owner/name" to prevent injection
    let expected = format!("{owner}/{name}");
    if full_name != expected {
        return Err(
            "Invalid repo identifier: full_name does not match owner/name".to_string(),
        );
    }
    let valid_chars =
        |s: &str| s.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_' || c == '.');
    if !valid_chars(&owner) || !valid_chars(&name) {
        return Err("Invalid characters in repository owner or name".to_string());
    }

    let account_id = {
        let app = state.lock().map_err(|e| e.to_string())?;
        app.active_account_id
            .clone()
            .unwrap_or_else(|| "default".to_string())
    };

    let mut repos = storage::load_tracked_repos(&app_handle, &account_id);
    if repos.iter().any(|r| r.full_name == full_name) {
        return Ok(repos);
    }

    repos.push(models::TrackedRepo {
        full_name,
        owner,
        name,
    });
    storage::save_tracked_repos(&app_handle, &account_id, &repos)
        .map_err(|e| format!("Failed to save tracked repos: {e}"))?;

    Ok(repos)
}

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
        app.active_account_id
            .clone()
            .unwrap_or_else(|| "default".to_string())
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

/// Fetch issues for a given owner/repo with optional filters.
#[cfg(not(feature = "dev-mock"))]
#[tauri::command]
async fn fetch_issues(
    owner: String,
    repo: String,
    filters: Option<FilterParams>,
    state: State<'_, Mutex<AppState>>,
) -> Result<Vec<models::Issue>, String> {
    let client = {
        let app = state.lock().map_err(|e| e.to_string())?;
        app.client.clone().ok_or("Not authenticated")?
    };
    github::issues::fetch_issues(&client, &owner, &repo, filters.as_ref())
        .await
        .map_err(|e| e.to_string())
}

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
    let title = title.trim().to_string();
    if title.is_empty() {
        return Err("Issue title cannot be empty".to_string());
    }
    if title.len() > 256 {
        return Err("Issue title exceeds maximum length of 256 characters".to_string());
    }
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

/// Fetch pull requests for a given owner/repo with optional filters.
#[cfg(not(feature = "dev-mock"))]
#[tauri::command]
async fn fetch_pulls(
    owner: String,
    repo: String,
    filters: Option<FilterParams>,
    state: State<'_, Mutex<AppState>>,
) -> Result<Vec<models::PullRequest>, String> {
    let client = {
        let app = state.lock().map_err(|e| e.to_string())?;
        app.client.clone().ok_or("Not authenticated")?
    };
    github::pulls::fetch_pulls(&client, &owner, &repo, filters.as_ref())
        .await
        .map_err(|e| e.to_string())
}

/// Fetch Dependabot / code-scanning security alerts.
#[cfg(not(feature = "dev-mock"))]
#[tauri::command]
async fn fetch_security_alerts(
    owner: String,
    repo: String,
    state: Option<String>,
    app_state: State<'_, Mutex<AppState>>,
) -> Result<Vec<models::SecurityAlert>, String> {
    let client = {
        let app = app_state.lock().map_err(|e| e.to_string())?;
        app.client.clone().ok_or("Not authenticated")?
    };
    github::security::fetch_alerts(&client, &owner, &repo, state.as_deref())
        .await
        .map_err(|e| e.to_string())
}

/// Fetch detailed diff statistics for a single pull request.
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

/// Export items (issues, PRs, alerts, and workflow runs) to CSV or PDF.
#[tauri::command]
async fn export_data(
    format: ExportFormat,
    issues: Vec<models::Issue>,
    pulls: Vec<models::PullRequest>,
    alerts: Vec<models::SecurityAlert>,
    workflow_runs: Vec<models::WorkflowRun>,
    file_path: String,
) -> Result<String, String> {
    match format {
        ExportFormat::Csv => {
            export::csv_export::export_to_csv(&issues, &pulls, &alerts, &workflow_runs, &file_path)
                .map_err(|e| e.to_string())?;
        }
        ExportFormat::Pdf => {
            export::pdf_export::export_to_pdf(&issues, &pulls, &alerts, &workflow_runs, &file_path)
                .map_err(|e| e.to_string())?;
        }
    }
    Ok(format!("Exported to {file_path}"))
}

/// Fetch the most recent GitHub Actions workflow runs for a repository.
#[cfg(not(feature = "dev-mock"))]
#[tauri::command]
async fn get_workflow_runs(
    owner: String,
    repo: String,
    state: State<'_, Mutex<AppState>>,
) -> Result<Vec<models::WorkflowRun>, String> {
    let token = {
        let app = state.lock().map_err(|e| e.to_string())?;
        app.token.clone().ok_or("Not authenticated")?
    };
    github::actions::fetch_workflow_runs(&token, &owner, &repo)
        .await
        .map_err(|e| e.to_string())
}

// ──────────────────────────────────────────────
// Application entry‐point
// ──────────────────────────────────────────────

fn main() {
    let builder = tauri::Builder::default().manage(Mutex::new(AppState::default()));

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
        close_issue,
        reopen_issue,
        add_issue_comment,
        create_issue,
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

    #[cfg(feature = "dev-mock")]
    let builder = builder.invoke_handler(tauri::generate_handler![
        mock::get_dev_mode,
        mock::restore_session,
        mock::list_repos,
        mock::fetch_issues,
        mock::close_issue,
        mock::reopen_issue,
        mock::add_issue_comment,
        mock::create_issue,
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

    builder
        .run(tauri::generate_context!())
        .expect("error while running GitHub Export");
}
