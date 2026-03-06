use anyhow::{Context, Result};
use octocrab::Octocrab;
use serde::Deserialize;

use crate::models::WorkflowRun;

#[derive(Debug, Deserialize)]
struct RawActor {
    login: String,
}

#[derive(Debug, Deserialize)]
struct RawWorkflowRun {
    id: u64,
    name: Option<String>,
    head_branch: Option<String>,
    run_number: u64,
    event: String,
    status: Option<String>,
    conclusion: Option<String>,
    actor: Option<RawActor>,
    created_at: String,
    run_started_at: Option<String>,
    html_url: String,
    workflow_id: u64,
}

#[derive(Debug, Deserialize)]
struct WorkflowRunsPage {
    workflow_runs: Vec<RawWorkflowRun>,
}

/// Fetch the most recent workflow runs for a repository.
///
/// Returns up to 30 runs, sorted newest-first (API default).
pub async fn fetch_workflow_runs(
    client: &Octocrab,
    owner: &str,
    repo: &str,
) -> Result<Vec<WorkflowRun>> {
    let url = format!("/repos/{owner}/{repo}/actions/runs?per_page=30&page=1");

    let page: WorkflowRunsPage = client
        .get(&url, None::<&()>)
        .await
        .with_context(|| format!("Failed to fetch workflow runs for {owner}/{repo}"))?;

    let runs = page
        .workflow_runs
        .into_iter()
        .map(|r| WorkflowRun {
            id: r.id,
            name: r.name.unwrap_or_default(),
            head_branch: r.head_branch,
            run_number: r.run_number,
            event: r.event,
            status: r.status.unwrap_or_default(),
            conclusion: r.conclusion,
            actor_login: r.actor.map(|a| a.login).unwrap_or_default(),
            created_at: r.created_at,
            run_started_at: r.run_started_at,
            html_url: r.html_url,
            workflow_id: r.workflow_id,
        })
        .collect();

    Ok(runs)
}
