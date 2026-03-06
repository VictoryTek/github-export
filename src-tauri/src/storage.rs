// src-tauri/src/storage.rs
//
// File-system persistence for the user's tracked repository list.
// Tracked repos are stored as a JSON file in the Tauri app data directory,
// keyed by account UUID so multiple accounts each have their own list.

use crate::models::TrackedRepo;
use std::collections::HashMap;
use std::fs;
use tauri::AppHandle;

/// Returns the path to `tracked_repos.json` in the app data directory,
/// creating the directory if it does not yet exist.
fn tracked_repos_path(app_handle: &AppHandle) -> Result<std::path::PathBuf, String> {
    let data_dir = tauri::api::path::app_data_dir(app_handle.config().as_ref())
        .ok_or_else(|| "Failed to resolve app data directory".to_string())?;
    fs::create_dir_all(&data_dir)
        .map_err(|e| format!("Failed to create app data directory: {e}"))?;
    Ok(data_dir.join("tracked_repos.json"))
}

/// Load the full per-account tracked repos map from disk.
/// Returns an empty map if the file does not exist or cannot be parsed.
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

/// Persist the full per-account tracked repos map to disk.
fn save_all(app_handle: &AppHandle, map: &HashMap<String, Vec<TrackedRepo>>) -> Result<(), String> {
    let path = tracked_repos_path(app_handle)?;
    let json = serde_json::to_string_pretty(map)
        .map_err(|e| format!("Failed to serialize tracked repos: {e}"))?;
    fs::write(&path, json).map_err(|e| format!("Failed to write tracked repos file: {e}"))
}

/// Load the tracked repos for a specific account.
/// Returns an empty list if the account has no tracked repos or the file is missing.
pub fn load_tracked_repos(app_handle: &AppHandle, account_id: &str) -> Vec<TrackedRepo> {
    let mut map = load_all(app_handle);
    map.remove(account_id).unwrap_or_default()
}

/// Replace the tracked repos for a specific account and persist to disk.
pub fn save_tracked_repos(
    app_handle: &AppHandle,
    account_id: &str,
    repos: &[TrackedRepo],
) -> Result<(), String> {
    let mut map = load_all(app_handle);
    map.insert(account_id.to_string(), repos.to_vec());
    save_all(app_handle, &map)
}
