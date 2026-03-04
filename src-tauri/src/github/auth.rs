use anyhow::{Context, Result};
use octocrab::Octocrab;
use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tauri::Manager;
use crate::models::AppState;

// Replace with your GitHub OAuth App Client ID from https://github.com/settings/developers
const GITHUB_CLIENT_ID: &str = "YOUR_OAUTH_APP_CLIENT_ID";
const DEVICE_CODE_URL: &str = "https://github.com/login/device/code";
const ACCESS_TOKEN_URL: &str = "https://github.com/login/oauth/access_token";
const OAUTH_SCOPES: &str = "repo security_events";

const KEYRING_SERVICE: &str = "github-export";
const KEYRING_USER: &str = "github-token";

// ── Structs ────────────────────────────────────────────────────────────────

/// Data returned to the frontend when the device flow is initiated.
#[derive(Debug, Serialize, Deserialize)]
pub struct DeviceFlowStart {
    pub user_code: String,
    pub verification_uri: String,
    pub device_code: String,
    pub expires_in: u64,
    pub interval: u64,
}

// ── Helper functions (used by Tauri commands in main.rs) ───────────────────

/// Build an authenticated Octocrab client from a token (PAT or OAuth).
pub async fn authenticate_with_token(token: &str) -> Result<Octocrab> {
    let client = Octocrab::builder()
        .personal_token(token.to_string())
        .build()
        .context("Failed to build GitHub client")?;
    Ok(client)
}

/// Persist the token in the OS credential store (Keychain / Credential Manager / Secret Service).
pub fn store_token(token: &str) -> Result<()> {
    let entry = keyring::Entry::new(KEYRING_SERVICE, KEYRING_USER)
        .context("Failed to create keyring entry")?;
    entry
        .set_password(token)
        .context("Failed to store token in keyring")?;
    Ok(())
}

/// Load a previously stored token from the OS credential store.
pub fn load_token() -> Result<String> {
    let entry = keyring::Entry::new(KEYRING_SERVICE, KEYRING_USER)
        .context("Failed to access keyring entry")?;
    let token = entry
        .get_password()
        .context("No stored token found")?;
    Ok(token)
}

/// Remove the stored token from the OS credential store.
pub fn delete_token() -> Result<()> {
    let entry = keyring::Entry::new(KEYRING_SERVICE, KEYRING_USER)
        .context("Failed to access keyring entry")?;
    entry
        .delete_credential()
        .context("Failed to delete stored token")?;
    Ok(())
}

// ── Tauri commands ─────────────────────────────────────────────────────────

/// Start the GitHub OAuth Device Flow.
///
/// POSTs to GitHub to obtain device/user codes, opens the browser to the
/// verification URI, and returns the data needed by the frontend to display
/// the user code and begin polling.
#[tauri::command]
pub async fn start_device_flow(app_handle: tauri::AppHandle) -> Result<DeviceFlowStart, String> {
    let client = reqwest::Client::new();

    let resp: serde_json::Value = client
        .post(DEVICE_CODE_URL)
        .header("Accept", "application/json")
        .form(&[("client_id", GITHUB_CLIENT_ID), ("scope", OAUTH_SCOPES)])
        .send()
        .await
        .map_err(|e| format!("Failed to reach GitHub device code endpoint: {e}"))?
        .json()
        .await
        .map_err(|e| format!("Failed to parse device code response: {e}"))?;

    let device_code = resp["device_code"]
        .as_str()
        .ok_or("Missing device_code in response")?
        .to_string();
    let user_code = resp["user_code"]
        .as_str()
        .ok_or("Missing user_code in response")?
        .to_string();
    let verification_uri = resp["verification_uri"]
        .as_str()
        .ok_or("Missing verification_uri in response")?
        .to_string();
    let expires_in = resp["expires_in"].as_u64().unwrap_or(900);
    let interval = resp["interval"].as_u64().unwrap_or(5);

    // Open the browser automatically so the user can enter the code
    tauri::api::shell::open(&app_handle.shell_scope(), &verification_uri, None)
        .map_err(|e| format!("Failed to open browser: {e}"))?;

    Ok(DeviceFlowStart {
        user_code,
        verification_uri,
        device_code,
        expires_in,
        interval,
    })
}

/// Poll GitHub for the OAuth access token using the device code.
///
/// Sleeps for `interval` seconds between attempts (increasing by 5 s on
/// `slow_down`). Returns the authenticated GitHub username on success, or an
/// error string if the user denies access, the code expires, or the total
/// `expires_in` timeout is reached.
#[tauri::command]
pub async fn poll_device_flow(
    device_code: String,
    expires_in: u64,
    interval: u64,
    state: tauri::State<'_, Mutex<AppState>>,
) -> Result<String, String> {
    let client = reqwest::Client::new();
    let deadline = Instant::now() + Duration::from_secs(expires_in);
    let mut current_interval = interval;

    loop {
        // Sleep first — GitHub requires waiting at least `interval` seconds
        // before the first poll attempt as well.
        tokio::time::sleep(Duration::from_secs(current_interval)).await;

        if Instant::now() >= deadline {
            return Err("Authorization timed out. Please try again.".to_string());
        }

        let resp: serde_json::Value = client
            .post(ACCESS_TOKEN_URL)
            .header("Accept", "application/json")
            .form(&[
                ("client_id", GITHUB_CLIENT_ID),
                ("device_code", device_code.as_str()),
                ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
            ])
            .send()
            .await
            .map_err(|e| format!("Failed to poll for access token: {e}"))?
            .json()
            .await
            .map_err(|e| format!("Failed to parse access token response: {e}"))?;

        // Success path
        if let Some(token) = resp["access_token"].as_str() {
            let token = token.to_string();

            // Persist token so future sessions can skip the flow
            if let Err(e) = store_token(&token) {
                eprintln!("Warning: could not store token in keyring: {e}");
            }

            // Build Octocrab client and resolve the authenticated username
            let octocrab_client = authenticate_with_token(&token)
                .await
                .map_err(|e| e.to_string())?;

            let user = octocrab_client
                .current()
                .user()
                .await
                .map_err(|e| format!("Failed to fetch GitHub user: {e}"))?;

            let username = user.login.clone();

            // Commit authenticated state
            let mut app = state.lock().map_err(|e| e.to_string())?;
            app.client = Some(octocrab_client);
            app.token = Some(token);
            app.username = Some(username.clone());

            return Ok(username);
        }

        // Error / waiting paths
        match resp["error"].as_str() {
            Some("authorization_pending") => {
                // User hasn't acted yet — keep polling at current interval
            }
            Some("slow_down") => {
                // GitHub asked us to back off
                current_interval += 5;
            }
            Some("expired_token") => {
                return Err("The authorization code expired. Please sign in again.".to_string());
            }
            Some("access_denied") => {
                return Err("Authorization was denied. Please try again.".to_string());
            }
            Some(other) => {
                return Err(format!("Authorization error: {other}"));
            }
            None => {
                return Err(
                    "Unexpected response from GitHub authorization server.".to_string(),
                );
            }
        }
    }
}
