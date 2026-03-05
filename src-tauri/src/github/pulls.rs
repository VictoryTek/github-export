use anyhow::{Context, Result};
use octocrab::params;
use octocrab::Octocrab;

use crate::models::{FilterParams, PullRequest};

/// Fetch pull requests for a specific repository, with optional filters.
pub async fn fetch_pulls(
    client: &Octocrab,
    owner: &str,
    repo: &str,
    filters: Option<&FilterParams>,
) -> Result<Vec<PullRequest>> {
    let pulls_handler = client.pulls(owner, repo);
    let mut builder = pulls_handler.list();

    if let Some(f) = filters {
        // State filter
        if let Some(ref st) = f.state {
            let state = match st.as_str() {
                "closed" => params::State::Closed,
                "all" => params::State::All,
                _ => params::State::Open,
            };
            builder = builder.state(state);
        }

        // Sort
        if let Some(ref sort) = f.sort {
            let sort_param = match sort.as_str() {
                "updated" => params::pulls::Sort::Updated,
                "popularity" => params::pulls::Sort::Popularity,
                "long-running" => params::pulls::Sort::LongRunning,
                _ => params::pulls::Sort::Created,
            };
            builder = builder.sort(sort_param);
        }

        // Direction
        if let Some(ref dir) = f.direction {
            let direction = match dir.as_str() {
                "asc" => params::Direction::Ascending,
                _ => params::Direction::Descending,
            };
            builder = builder.direction(direction);
        }

        // Pagination
        if let Some(per_page) = f.per_page {
            builder = builder.per_page(per_page);
        }
        if let Some(page) = f.page {
            builder = builder.page(page);
        }
    }

    let mut page = builder
        .send()
        .await
        .context("Failed to fetch pull requests")?;

    let pulls = page
        .take_items()
        .into_iter()
        .map(|pr| PullRequest {
            number: pr.number,
            title: pr.title.clone().unwrap_or_default(),
            state: pr
                .state
                .as_ref()
                .map(|s| format!("{:?}", s))
                .unwrap_or_else(|| "unknown".to_string()),
            author: pr
                .user
                .as_ref()
                .map(|u| u.login.clone())
                .unwrap_or_default(),
            labels: pr
                .labels
                .unwrap_or_default()
                .iter()
                .map(|l| l.name.clone())
                .collect(),
            assignees: pr
                .assignees
                .unwrap_or_default()
                .iter()
                .map(|a| a.login.clone())
                .collect(),
            reviewers: pr
                .requested_reviewers
                .unwrap_or_default()
                .iter()
                .map(|r| r.login.clone())
                .collect(),
            head_branch: pr.head.label.clone().unwrap_or_default(),
            base_branch: pr.base.label.clone().unwrap_or_default(),
            created_at: pr.created_at.unwrap_or_default(),
            updated_at: pr.updated_at.unwrap_or_default(),
            merged_at: pr.merged_at,
            closed_at: pr.closed_at,
            html_url: pr
                .html_url
                .as_ref()
                .map(|u| u.to_string())
                .unwrap_or_default(),
            draft: pr.draft.unwrap_or(false),
            body: pr.body,
        })
        .collect();

    Ok(pulls)
}
