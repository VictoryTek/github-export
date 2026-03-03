// Prevents additional console window on Windows in release builds
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod export;
mod github;
mod models;

use models::{AppState, ExportFormat, FilterParams};
use std::sync::Mutex;
use tauri::State;

// ──────────────────────────────────────────────
// Tauri commands exposed to the frontend
// ──────────────────────────────────────────────

/// Authenticate with GitHub using a personal access token.
#[tauri::command]
async fn authenticate(
    token: String,
    state: State<'_, Mutex<AppState>>,
) -> Result<String, String> {
    let client = github::auth::authenticate_with_token(&token)
        .await
        .map_err(|e| e.to_string())?;

    let user = client
        .current()
        .user()
        .await
        .map_err(|e| format!("Failed to fetch user: {e}"))?;

    let username = user.login.clone();

    let mut app = state.lock().map_err(|e| e.to_string())?;
    app.client = Some(client);
    app.token = Some(token);
    app.username = Some(username.clone());

    // Persist token in OS keyring for future sessions
    if let Err(e) = github::auth::store_token(&app.token.as_ref().unwrap()) {
        eprintln!("Warning: could not store token in keyring: {e}");
    }

    Ok(username)
}

/// Try to restore a previously saved token from the OS keyring.
#[tauri::command]
async fn restore_session(
    state: State<'_, Mutex<AppState>>,
) -> Result<Option<String>, String> {
    match github::auth::load_token() {
        Ok(token) => {
            let client = github::auth::authenticate_with_token(&token)
                .await
                .map_err(|e| e.to_string())?;

            let user = client
                .current()
                .user()
                .await
                .map_err(|e| format!("Failed to fetch user: {e}"))?;

            let username = user.login.clone();

            let mut app = state.lock().map_err(|e| e.to_string())?;
            app.client = Some(client);
            app.token = Some(token);
            app.username = Some(username.clone());

            Ok(Some(username))
        }
        Err(_) => Ok(None),
    }
}

/// Logout – clear state and remove stored token.
#[tauri::command]
fn logout(state: State<'_, Mutex<AppState>>) -> Result<(), String> {
    let mut app = state.lock().map_err(|e| e.to_string())?;
    app.client = None;
    app.token = None;
    app.username = None;
    let _ = github::auth::delete_token();
    Ok(())
}

/// List repositories visible to the authenticated user.
#[tauri::command]
async fn list_repos(
    state: State<'_, Mutex<AppState>>,
) -> Result<Vec<models::Repo>, String> {
    let client = {
        let app = state.lock().map_err(|e| e.to_string())?;
        app.client.clone().ok_or("Not authenticated")?
    };
    github::issues::list_repos(&client)
        .await
        .map_err(|e| e.to_string())
}

/// Fetch issues for a given owner/repo with optional filters.
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

/// Fetch pull requests for a given owner/repo with optional filters.
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
#[tauri::command]
async fn fetch_security_alerts(
    owner: String,
    repo: String,
    state: State<'_, Mutex<AppState>>,
) -> Result<Vec<models::SecurityAlert>, String> {
    let client = {
        let app = state.lock().map_err(|e| e.to_string())?;
        app.client.clone().ok_or("Not authenticated")?
    };
    github::security::fetch_alerts(&client, &owner, &repo)
        .await
        .map_err(|e| e.to_string())
}

/// Export items (issues, PRs, or alerts) to CSV or PDF.
#[tauri::command]
async fn export_data(
    format: ExportFormat,
    issues: Vec<models::Issue>,
    pulls: Vec<models::PullRequest>,
    alerts: Vec<models::SecurityAlert>,
    file_path: String,
) -> Result<String, String> {
    match format {
        ExportFormat::Csv => {
            export::csv_export::export_to_csv(&issues, &pulls, &alerts, &file_path)
                .map_err(|e| e.to_string())?;
        }
        ExportFormat::Pdf => {
            export::pdf_export::export_to_pdf(&issues, &pulls, &alerts, &file_path)
                .map_err(|e| e.to_string())?;
        }
    }
    Ok(format!("Exported to {file_path}"))
}

// ──────────────────────────────────────────────
// Application entry‐point
// ──────────────────────────────────────────────

fn main() {
    tauri::Builder::default()
        .manage(Mutex::new(AppState::default()))
        .invoke_handler(tauri::generate_handler![
            authenticate,
            restore_session,
            logout,
            list_repos,
            fetch_issues,
            fetch_pulls,
            fetch_security_alerts,
            export_data,
        ])
        .run(tauri::generate_context!())
        .expect("error while running GitHub Export");
}
