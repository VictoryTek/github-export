use chrono::{DateTime, Utc};
use octocrab::Octocrab;
use serde::{Deserialize, Serialize};

// ──────────────────────────────────────────────
// Application state
// ──────────────────────────────────────────────

#[derive(Default)]
pub struct AppState {
    pub client: Option<Octocrab>,
    pub token: Option<String>,
    pub username: Option<String>,
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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PullRequest {
    pub number: u64,
    pub title: String,
    pub state: String,
    pub author: String,
    pub labels: Vec<String>,
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
