use anyhow::{Context, Result};
use octocrab::Octocrab;

use crate::models::PullDetail;

/// Fetches detailed statistics for a single pull request.
/// The list endpoint does not return additions/deletions/changed_files/mergeable;
/// those require an individual GET to /repos/{owner}/{repo}/pulls/{pull_number}.
pub async fn fetch_pull_detail(
    client: &Octocrab,
    owner: &str,
    repo: &str,
    pull_number: u64,
) -> Result<PullDetail> {
    let pr = client
        .pulls(owner, repo)
        .get(pull_number)
        .await
        .context("Failed to fetch pull request detail")?;

    Ok(PullDetail {
        number: pull_number,
        additions: pr.additions.unwrap_or(0),
        deletions: pr.deletions.unwrap_or(0),
        changed_files: pr.changed_files.unwrap_or(0),
        mergeable: pr.mergeable,
        mergeable_state: pr.mergeable_state.map(|s| format!("{:?}", s)),
    })
}
