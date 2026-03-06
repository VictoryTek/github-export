use anyhow::{Context, Result};
use reqwest::Client;
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
    event: Option<String>,
    status: Option<String>,
    conclusion: Option<String>,
    actor: Option<RawActor>,
    created_at: String,
    run_started_at: Option<String>,
    html_url: String,
    workflow_id: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct WorkflowRunsPage {
    workflow_runs: Vec<RawWorkflowRun>,
}

#[derive(Debug, Deserialize)]
struct RawWorkflow {
    id: u64,
}

#[derive(Debug, Deserialize)]
struct WorkflowsPage {
    workflows: Vec<RawWorkflow>,
}

/// Build the standard GitHub API request headers.
fn make_headers(token: &str) -> Vec<(String, String)> {
    vec![
        ("Authorization".into(), format!("token {token}")),
        ("Accept".into(), "application/vnd.github+json".into()),
        ("User-Agent".into(), "github-export/0.1.0".into()),
    ]
}

/// Make an authenticated GET request to the GitHub API and return the body.
async fn github_get(
    client: &Client,
    url: &str,
    headers: &[(String, String)],
) -> Result<String> {
    let mut req = client.get(url);
    for (k, v) in headers {
        req = req.header(k.as_str(), v.as_str());
    }
    let response = req.send().await.with_context(|| format!("GET {url} failed"))?;
    let status = response.status();
    let body = response.text().await.context("Failed to read response body")?;

    if !status.is_success() {
        let message = serde_json::from_str::<serde_json::Value>(&body)
            .ok()
            .and_then(|v| v.get("message").and_then(|m| m.as_str()).map(str::to_owned))
            .unwrap_or_else(|| body.chars().take(300).collect());
        anyhow::bail!("GitHub API returned {status}: {message}");
    }

    Ok(body)
}

/// Fetch the most recent workflow runs for a repository.
///
/// # GitHub API Issue — LAST INVESTIGATED: 2026-03-06
///
/// The GitHub REST API (`/repos/{owner}/{repo}/actions/runs`) consistently
/// returns `{"total_count":0,"workflow_runs":[]}` even for repos with many
/// visible runs on github.com. This was exhaustively verified:
///
/// - Token: valid PAT (ghp_*) with admin, repo, workflow, audit_log scopes
/// - Permissions: admin:true, push:true on the repo
/// - Repo: both public and private repos affected
/// - Same result from reqwest AND PowerShell using identical token
/// - Workflows endpoint works fine (returns 3 definitions)
/// - Per-workflow fallback (`/actions/workflows/{id}/runs`) also returns 0
/// - Not a rate-limit issue (tested with fresh tokens)
/// - Actions enabled in repo settings ("Allow all actions")
///
/// The Actions tab in the UI has been hidden until GitHub resolves this.
/// To re-enable: remove the comment in src/index.html around `#actions-tab`,
/// restore the `#tab-actions` panel HTML, restore the CSS in styles.css,
/// and restore the `loadActions` / `renderWorkflowRuns` / `updateActionStatusDot`
/// functions + click listener in main.js.
/// Test first with: `curl -H "Authorization: token TOKEN" https://api.github.com/repos/OWNER/REPO/actions/runs`
/// If that returns real data, the issue is resolved.
///
/// Tries the global `/actions/runs` endpoint first. If that returns zero runs
/// but the repo has workflow definitions, falls back to fetching runs
/// per-workflow via `/actions/workflows/{id}/runs` and merging results.
pub async fn fetch_workflow_runs(
    token: &str,
    owner: &str,
    repo: &str,
) -> Result<Vec<WorkflowRun>> {
    let client = Client::new();
    let headers = make_headers(token);

    // ── Attempt 1: global runs endpoint ──
    let runs_url = format!(
        "https://api.github.com/repos/{owner}/{repo}/actions/runs?per_page=100"
    );

    let body = github_get(&client, &runs_url, &headers).await?;

    let page: WorkflowRunsPage = serde_json::from_str(&body).with_context(|| {
        format!("Failed to parse workflow runs for {owner}/{repo}")
    })?;

    if !page.workflow_runs.is_empty() {
        return Ok(convert_runs(page.workflow_runs));
    }

    // ── Global endpoint returned 0 — try per-workflow fallback ──
    let workflows_url = format!(
        "https://api.github.com/repos/{owner}/{repo}/actions/workflows"
    );
    let wf_body = github_get(&client, &workflows_url, &headers).await?;
    let wf_page: WorkflowsPage = serde_json::from_str(&wf_body)
        .context("Failed to parse workflows list")?;

    if wf_page.workflows.is_empty() {
        return Ok(vec![]);
    }

    // ── Attempt 2: fetch runs per-workflow ──
    let mut all_runs: Vec<RawWorkflowRun> = Vec::new();

    for wf in &wf_page.workflows {
        let wf_runs_url = format!(
            "https://api.github.com/repos/{owner}/{repo}/actions/workflows/{}/runs?per_page=100",
            wf.id
        );

        match github_get(&client, &wf_runs_url, &headers).await {
            Ok(wf_runs_body) => {
                if let Ok(wf_runs_page) = serde_json::from_str::<WorkflowRunsPage>(&wf_runs_body) {
                    all_runs.extend(wf_runs_page.workflow_runs);
                }
            }
            Err(_) => continue,
        }
    }

    // Deduplicate by run id and sort newest-first
    all_runs.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    all_runs.dedup_by_key(|r| r.id);

    Ok(convert_runs(all_runs))
}

fn convert_runs(raw: Vec<RawWorkflowRun>) -> Vec<WorkflowRun> {
    raw.into_iter()
        .map(|r| WorkflowRun {
            id: r.id,
            name: r.name.unwrap_or_default(),
            head_branch: r.head_branch,
            run_number: r.run_number,
            event: r.event.unwrap_or_default(),
            status: r.status.unwrap_or_default(),
            conclusion: r.conclusion,
            actor_login: r.actor.map(|a| a.login).unwrap_or_default(),
            created_at: r.created_at,
            run_started_at: r.run_started_at,
            html_url: r.html_url,
            workflow_id: r.workflow_id.unwrap_or(0),
        })
        .collect()
}
