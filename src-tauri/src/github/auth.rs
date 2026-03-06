use crate::models::{Account, AccountInfo, AppState, RestoreResult};
use anyhow::{Context, Result};
use octocrab::Octocrab;
use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tauri::Manager;
use uuid::Uuid;

/// The GitHub OAuth App Client ID.
///
/// To obtain this:
/// 1. Go to <https://github.com/settings/developers> → "OAuth Apps" → your app
///    (or "New OAuth App" to create one)
/// 2. Copy the "Client ID" value (format: Ov23xxxxxxxxxxxxxxxx)
/// 3. **IMPORTANT**: On the same settings page, scroll to "Device Flow" and
///    click "Enable Device Flow" — this is required for this app to work.
///    It is disabled by default on all new OAuth Apps.
///
/// The Client ID is not a secret (RFC 8628 §3.4 — public clients).
const GITHUB_CLIENT_ID: &str = "Ov23lit0Ok09PHqufOw7";
const DEVICE_CODE_URL: &str = "https://github.com/login/device/code";
const ACCESS_TOKEN_URL: &str = "https://github.com/login/oauth/access_token";
const OAUTH_SCOPES: &str = "repo security_events";

const KEYRING_SERVICE: &str = "github-export";
const KEYRING_USER: &str = "github-token"; // legacy single-account key

// Multi-account keyring keys
const KEYRING_ACCOUNTS_INDEX: &str = "accounts-index";
const KEYRING_ACTIVE_ACCOUNT: &str = "active-account-id";

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

/// Load a previously stored token from the OS credential store.
pub fn load_token() -> Result<String> {
    let entry = keyring::Entry::new(KEYRING_SERVICE, KEYRING_USER)
        .context("Failed to access keyring entry")?;
    let token = entry.get_password().context("No stored token found")?;
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
// ── Multi-account keyring helpers ─────────────────────────────────────────

/// Load the account index from the keyring. Returns empty `Vec` if absent.
fn load_account_index() -> Result<Vec<Account>> {
    let entry = keyring::Entry::new(KEYRING_SERVICE, KEYRING_ACCOUNTS_INDEX)
        .context("Failed to create keyring entry for accounts index")?;
    match entry.get_password() {
        Ok(json) => serde_json::from_str(&json).context("Failed to parse accounts index JSON"),
        Err(_) => Ok(Vec::new()),
    }
}

/// Persist the account index to the keyring.
fn save_account_index(accounts: &[Account]) -> Result<()> {
    let entry = keyring::Entry::new(KEYRING_SERVICE, KEYRING_ACCOUNTS_INDEX)
        .context("Failed to create keyring entry for accounts index")?;
    let json = serde_json::to_string(accounts).context("Failed to serialize accounts index")?;
    entry
        .set_password(&json)
        .context("Failed to save accounts index to keyring")
}

/// Load a token for a specific account ID from the keyring.
fn load_account_token(account_id: &str) -> Result<String> {
    let key = format!("token-{account_id}");
    let entry = keyring::Entry::new(KEYRING_SERVICE, &key)
        .context("Failed to create keyring entry for account token")?;
    entry.get_password().context("No token found for account")
}

/// Store a token for a specific account ID in the keyring.
fn store_account_token(account_id: &str, token: &str) -> Result<()> {
    let key = format!("token-{account_id}");
    let entry = keyring::Entry::new(KEYRING_SERVICE, &key)
        .context("Failed to create keyring entry for account token")?;
    entry
        .set_password(token)
        .context("Failed to store account token in keyring")
}

/// Delete a token for a specific account ID from the keyring.
/// Silently ignores not-found errors.
fn delete_account_token(account_id: &str) -> Result<()> {
    let key = format!("token-{account_id}");
    let entry = keyring::Entry::new(KEYRING_SERVICE, &key)
        .context("Failed to create keyring entry for account token")?;
    let _ = entry.delete_credential();
    Ok(())
}

/// Load the persisted active-account-id from the keyring.
fn load_active_account_id() -> Result<Option<String>> {
    let entry = keyring::Entry::new(KEYRING_SERVICE, KEYRING_ACTIVE_ACCOUNT)
        .context("Failed to create keyring entry for active account ID")?;
    match entry.get_password() {
        Ok(id) => Ok(Some(id)),
        Err(_) => Ok(None),
    }
}

/// Persist the active-account-id to the keyring.
fn save_active_account_id(account_id: &str) -> Result<()> {
    let entry = keyring::Entry::new(KEYRING_SERVICE, KEYRING_ACTIVE_ACCOUNT)
        .context("Failed to create keyring entry for active account ID")?;
    entry
        .set_password(account_id)
        .context("Failed to save active account ID to keyring")
}

/// Delete the active-account-id from the keyring.
pub fn delete_active_account_id() -> Result<()> {
    let entry = keyring::Entry::new(KEYRING_SERVICE, KEYRING_ACTIVE_ACCOUNT)
        .context("Failed to create keyring entry for active account ID")?;
    let _ = entry.delete_credential();
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

    let response = client
        .post(DEVICE_CODE_URL)
        .header("Accept", "application/json")
        .form(&[("client_id", GITHUB_CLIENT_ID), ("scope", OAUTH_SCOPES)])
        .send()
        .await
        .map_err(|e| format!("Failed to reach GitHub device code endpoint: {e}"))?;

    let status = response.status();
    let body = response
        .text()
        .await
        .map_err(|e| format!("Failed to read response body: {e}"))?;

    let resp: serde_json::Value = serde_json::from_str(&body).unwrap_or(serde_json::Value::Null);

    if !status.is_success() {
        let error_hint = resp["error"].as_str().unwrap_or("unknown error");
        let description = resp["error_description"].as_str().unwrap_or(&body);
        return Err(format!(
            "GitHub returned HTTP {status}: {error_hint} — {description}\n\
             Verify that the Client ID is correct and Device Flow is enabled \
             on your OAuth App at https://github.com/settings/developers"
        ));
    }

    // Also check application-level errors signalled in 200 responses
    // (RFC 8628 §3.2).
    if let Some(error) = resp["error"].as_str() {
        let description = resp["error_description"]
            .as_str()
            .unwrap_or("No description provided by GitHub.");
        return Err(format!(
            "GitHub auth error: {error} — {description}\n\
             If the error is \"unauthorized_client\", ensure Device Flow is enabled \
             on your OAuth App at https://github.com/settings/developers"
        ));
    }

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

        let poll_response = client
            .post(ACCESS_TOKEN_URL)
            .header("Accept", "application/json")
            .form(&[
                ("client_id", GITHUB_CLIENT_ID),
                ("device_code", device_code.as_str()),
                ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
            ])
            .send()
            .await
            .map_err(|e| format!("Failed to poll for access token: {e}"))?;

        let poll_status = poll_response.status();
        let poll_body = poll_response
            .text()
            .await
            .map_err(|e| format!("Failed to read poll response body: {e}"))?;

        let resp: serde_json::Value =
            serde_json::from_str(&poll_body).unwrap_or(serde_json::Value::Null);

        if !poll_status.is_success() {
            let error_hint = resp["error"].as_str().unwrap_or("unknown error");
            let description = resp["error_description"].as_str().unwrap_or(&poll_body);
            return Err(format!(
                "GitHub returned HTTP {poll_status}: {error_hint} — {description}"
            ));
        }

        // Success path
        if let Some(token) = resp["access_token"].as_str() {
            let token = token.to_string();

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
            let new_id = Uuid::new_v4().to_string();

            // Add to multi-account store and make active
            let mut app = state.lock().map_err(|e| e.to_string())?;
            let account_id = if let Some(id) = app
                .accounts
                .iter()
                .find(|a| a.username == username)
                .map(|a| a.id.clone())
            {
                // Update token for existing account
                store_account_token(&id, &token).map_err(|e| e.to_string())?;
                id
            } else {
                store_account_token(&new_id, &token).map_err(|e| e.to_string())?;
                let account = Account {
                    id: new_id.clone(),
                    label: username.clone(),
                    username: username.clone(),
                };
                app.accounts.push(account);
                if let Err(e) = save_account_index(&app.accounts) {
                    eprintln!("Warning: could not persist account index: {e}");
                }
                new_id
            };
            if let Err(e) = save_active_account_id(&account_id) {
                eprintln!("Warning: could not save active account ID: {e}");
            }
            app.client = Some(octocrab_client);
            app.token = Some(token);
            app.username = Some(username.clone());
            app.active_account_id = Some(account_id);

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
                return Err("Unexpected response from GitHub authorization server.".to_string());
            }
        }
    }
}

/// Authenticate using a GitHub Personal Access Token (PAT).
/// Adds the account to the multi-account store and makes it the active account.
#[tauri::command]
pub async fn authenticate_with_pat(
    token: String,
    label: Option<String>,
    state: tauri::State<'_, Mutex<AppState>>,
) -> Result<String, String> {
    // Validate token and resolve username before acquiring the lock
    let client = authenticate_with_token(&token)
        .await
        .map_err(|e| e.to_string())?;
    let user = client
        .current()
        .user()
        .await
        .map_err(|e| format!("Failed to fetch GitHub user: {e}"))?;

    let username = user.login.clone();
    let display_label = label.unwrap_or_else(|| username.clone());
    let new_id = Uuid::new_v4().to_string();

    let mut app = state.lock().map_err(|e| e.to_string())?;

    // If this username already exists, update its token and make it active
    if let Some(existing_id) = app
        .accounts
        .iter()
        .find(|a| a.username == username)
        .map(|a| a.id.clone())
    {
        store_account_token(&existing_id, &token).map_err(|e| e.to_string())?;
        app.client = Some(client);
        app.token = Some(token);
        app.username = Some(username.clone());
        app.active_account_id = Some(existing_id.clone());
        let _ = save_active_account_id(&existing_id);
        return Ok(username);
    }

    // New account — store token and append to index
    store_account_token(&new_id, &token).map_err(|e| e.to_string())?;
    let account = Account {
        id: new_id.clone(),
        label: display_label,
        username: username.clone(),
    };
    app.accounts.push(account);
    if let Err(e) = save_account_index(&app.accounts) {
        eprintln!("Warning: could not persist account index: {e}");
    }
    app.client = Some(client);
    app.token = Some(token);
    app.username = Some(username.clone());
    app.active_account_id = Some(new_id.clone());
    let _ = save_active_account_id(&new_id);

    Ok(username)
}

// ── Multi-account Tauri commands ─────────────────────────────────────────────

/// Restore a previously saved session from the OS keyring.
/// Handles legacy single-account migration automatically.
// Only compiled in non-mock builds — mock mode provides its own restore_session.
#[cfg(not(feature = "dev-mock"))]
#[tauri::command]
pub async fn restore_session(
    state: tauri::State<'_, Mutex<AppState>>,
) -> Result<Option<RestoreResult>, String> {
    // Step 1: Load account index from keyring
    let mut accounts = load_account_index().unwrap_or_default();

    // Step 2: Legacy migration — if no accounts exist but old token does
    if accounts.is_empty() {
        if let Ok(legacy_token) = load_token() {
            if let Ok(client) = authenticate_with_token(&legacy_token).await {
                if let Ok(user) = client.current().user().await {
                    let username = user.login.clone();
                    let id = Uuid::new_v4().to_string();
                    let account = Account {
                        id: id.clone(),
                        label: username.clone(),
                        username: username.clone(),
                    };
                    if store_account_token(&id, &legacy_token).is_ok() {
                        accounts.push(account);
                        let _ = save_account_index(&accounts);
                        // Only remove legacy token after successful migration
                        let _ = delete_token();
                    }
                }
            }
        }
    }

    // Step 3: Always populate accounts in state so later operations see them
    {
        let mut app = state.lock().map_err(|e| e.to_string())?;
        app.accounts = accounts.clone();
    }

    if accounts.is_empty() {
        return Ok(None);
    }

    // Step 4: Determine which account to restore
    let preferred_id = load_active_account_id().unwrap_or(None);
    let active_id = preferred_id
        .filter(|id| accounts.iter().any(|a| &a.id == id))
        .or_else(|| accounts.first().map(|a| a.id.clone()));

    let active_id = match active_id {
        Some(id) => id,
        None => return Ok(None),
    };

    // Step 5: Load and validate the active account's token
    let token = match load_account_token(&active_id) {
        Ok(t) => t,
        Err(_) => return Ok(None),
    };
    let client = match authenticate_with_token(&token).await {
        Ok(c) => c,
        Err(_) => return Ok(None),
    };
    let user = match client.current().user().await {
        Ok(u) => u,
        Err(_) => return Ok(None),
    };
    let username = user.login.clone();

    // Persist active account ID
    let _ = save_active_account_id(&active_id);

    // Step 6: Commit authenticated session
    {
        let mut app = state.lock().map_err(|e| e.to_string())?;
        app.client = Some(client);
        app.token = Some(token);
        app.username = Some(username.clone());
        app.active_account_id = Some(active_id.clone());
        // accounts already populated in step 3
    }

    let account_infos: Vec<AccountInfo> = accounts
        .iter()
        .map(|a| AccountInfo {
            id: a.id.clone(),
            label: a.label.clone(),
            username: a.username.clone(),
            is_active: a.id == active_id,
        })
        .collect();

    Ok(Some(RestoreResult {
        username,
        accounts: account_infos,
    }))
}

/// List all accounts known to this application.
#[tauri::command]
pub fn list_accounts(state: tauri::State<'_, Mutex<AppState>>) -> Result<Vec<AccountInfo>, String> {
    let app = state.lock().map_err(|e| e.to_string())?;
    let active_id = app.active_account_id.as_deref().unwrap_or("");
    let infos = app
        .accounts
        .iter()
        .map(|a| AccountInfo {
            id: a.id.clone(),
            label: a.label.clone(),
            username: a.username.clone(),
            is_active: a.id == active_id,
        })
        .collect();
    Ok(infos)
}

/// Add a new GitHub account by validating a PAT and storing it in the keyring.
/// Does not make the new account active — call `switch_account` after if desired.
#[tauri::command]
pub async fn add_account(
    token: String,
    label: Option<String>,
    state: tauri::State<'_, Mutex<AppState>>,
) -> Result<AccountInfo, String> {
    // Validate token and resolve username (no lock held during HTTP call)
    let client = authenticate_with_token(&token)
        .await
        .map_err(|e| e.to_string())?;
    let user = client
        .current()
        .user()
        .await
        .map_err(|e| format!("Failed to fetch GitHub user: {e}"))?;
    let username = user.login.clone();
    let display_label = label.unwrap_or_else(|| username.clone());
    let id = Uuid::new_v4().to_string();

    let mut app = state.lock().map_err(|e| e.to_string())?;
    if app.accounts.iter().any(|a| a.username == username) {
        return Err(format!("Account @{username} is already saved."));
    }
    store_account_token(&id, &token).map_err(|e| e.to_string())?;
    let account = Account {
        id: id.clone(),
        label: display_label.clone(),
        username: username.clone(),
    };
    app.accounts.push(account);
    save_account_index(&app.accounts).map_err(|e| e.to_string())?;
    let is_active = app.active_account_id.as_deref() == Some(id.as_str());
    Ok(AccountInfo {
        id,
        label: display_label,
        username,
        is_active,
    })
}

/// Switch the active account: loads its token, rebuilds the Octocrab client,
/// and updates `AppState`.
#[tauri::command]
pub async fn switch_account(
    account_id: String,
    state: tauri::State<'_, Mutex<AppState>>,
) -> Result<String, String> {
    // Verify account exists (brief lock, then release before I/O + HTTP)
    {
        let app = state.lock().map_err(|e| e.to_string())?;
        if !app.accounts.iter().any(|a| a.id == account_id) {
            return Err(format!("Account {account_id} not found"));
        }
    }

    // Load token from keyring (no lock held)
    let token = load_account_token(&account_id).map_err(|e| e.to_string())?;

    // Build client and validate (no lock held during async HTTP call)
    let client = authenticate_with_token(&token)
        .await
        .map_err(|e| format!("Failed to build GitHub client: {e}"))?;
    let user = client
        .current()
        .user()
        .await
        .map_err(|e| format!("Failed to validate token for account: {e}"))?;
    let resolved_username = user.login.clone();

    // Persist active account ID
    if let Err(e) = save_active_account_id(&account_id) {
        eprintln!("Warning: could not save active account ID: {e}");
    }

    // Commit state
    let mut app = state.lock().map_err(|e| e.to_string())?;
    app.client = Some(client);
    app.token = Some(token);
    app.username = Some(resolved_username.clone());
    app.active_account_id = Some(account_id);
    Ok(resolved_username)
}

/// Remove an account: deletes its token and removes it from the in-memory list.
/// If the removed account was active, clears the current session.
#[tauri::command]
pub fn remove_account(
    account_id: String,
    state: tauri::State<'_, Mutex<AppState>>,
) -> Result<(), String> {
    let mut app = state.lock().map_err(|e| e.to_string())?;
    delete_account_token(&account_id).map_err(|e| e.to_string())?;
    app.accounts.retain(|a| a.id != account_id);
    save_account_index(&app.accounts).map_err(|e| e.to_string())?;
    if app.active_account_id.as_deref() == Some(account_id.as_str()) {
        app.client = None;
        app.token = None;
        app.username = None;
        app.active_account_id = None;
        let _ = delete_active_account_id();
    }
    Ok(())
}
