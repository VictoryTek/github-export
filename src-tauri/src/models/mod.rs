use chrono::{DateTime, Utc};
use octocrab::Octocrab;
use serde::{Deserialize, Serialize};

// ──────────────────────────────────────────────
// Multi-account types
// ──────────────────────────────────────────────

/// A single stored GitHub account (PAT or OAuth token).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Account {
    /// Stable unique identifier (UUID v4).
    pub id: String,
    /// User-visible display name.
    pub label: String,
    /// Resolved GitHub username at the time the account was added.
    pub username: String,
}

/// Serialisable view of an account returned to the frontend.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountInfo {
    pub id: String,
    pub label: String,
    pub username: String,
    pub is_active: bool,
}

/// Returned by `restore_session` when a session is successfully restored.
#[derive(Debug, Serialize, Deserialize)]
pub struct RestoreResult {
    pub username: String,
    pub accounts: Vec<AccountInfo>,
}

// ──────────────────────────────────────────────
// Application state
// ──────────────────────────────────────────────

#[derive(Default)]
pub struct AppState {
    pub client: Option<Octocrab>,
    pub token: Option<String>,
    pub username: Option<String>,
    /// The id of the currently active account.
    pub active_account_id: Option<String>,
    /// All known accounts (metadata only — tokens stay in the keyring).
    pub accounts: Vec<Account>,
}

// ──────────────────────────────────────────────
// Domain models
// ──────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Repo {
    pub id: u64,
    pub name: String,
    pub full_name: String,
    pub owner: String,
    pub description: Option<String>,
    pub private: bool,
    pub html_url: String,
    pub open_issues_count: u32,
}

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
    pub comments: u32,
    pub milestone: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PullRequest {
    pub number: u64,
    pub title: String,
    pub state: String,
    pub author: String,
    pub labels: Vec<String>,
    pub assignees: Vec<String>,
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
    pub cve_id: Option<String>,
    pub cvss_score: Option<f64>,
    pub cwes: Vec<String>,
    pub dismissed_reason: Option<String>,
    pub dismissed_comment: Option<String>,
}

// ──────────────────────────────────────────────
// Pull request detail (diff stats — from individual PR endpoint)
// ──────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PullDetail {
    pub number: u64,
    pub additions: u64,
    pub deletions: u64,
    pub changed_files: u64,
    pub mergeable: Option<bool>,
    pub mergeable_state: Option<String>,
}

// ──────────────────────────────────────────────
// Filter / search parameters
// ──────────────────────────────────────────────

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FilterParams {
    /// Filter by state: "open", "closed", "all"
    pub state: Option<String>,
    /// Filter by label name
    pub label: Option<String>,
    /// Free-text search query
    pub search: Option<String>,
    /// Sort field: "created", "updated", "comments"
    pub sort: Option<String>,
    /// Sort direction: "asc" or "desc"
    pub direction: Option<String>,
    /// Page number (1-based)
    pub page: Option<u32>,
    /// Items per page (max 100)
    pub per_page: Option<u8>,
}

// ──────────────────────────────────────────────
// Export format enum
// ──────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ExportFormat {
    Csv,
    Pdf,
}
