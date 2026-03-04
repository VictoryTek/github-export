use anyhow::{Context, Result};
use octocrab::params;
use octocrab::Octocrab;

use crate::models::{FilterParams, Issue, Repo};

/// List repositories visible to the authenticated user.
pub async fn list_repos(client: &Octocrab) -> Result<Vec<Repo>> {
    let mut page = client
        .current()
        .list_repos_for_authenticated_user()
        .sort("updated")
        .per_page(50)
        .send()
        .await
        .context("Failed to list repositories")?;

    let repos = page
        .take_items()
        .into_iter()
        .map(|r| Repo {
            id: r.id.into_inner(),
            name: r.name.clone(),
            full_name: r.full_name.clone().unwrap_or_default(),
            owner: r
                .owner
                .as_ref()
                .map(|o| o.login.clone())
                .unwrap_or_default(),
            description: r.description.clone(),
            private: r.private.unwrap_or(false),
            html_url: r
                .html_url
                .as_ref()
                .map(|u| u.to_string())
                .unwrap_or_default(),
            open_issues_count: r.open_issues_count.unwrap_or(0),
        })
        .collect();

    Ok(repos)
}

/// Fetch issues for a specific repository, with optional filters.
pub async fn fetch_issues(
    client: &Octocrab,
    owner: &str,
    repo: &str,
    filters: Option<&FilterParams>,
) -> Result<Vec<Issue>> {
    let issues_handler = client.issues(owner, repo);
    let mut builder = issues_handler.list();

    // Pre-compute label vec outside the if-let so it lives long enough
    let label_vec: Vec<String> = filters
        .and_then(|f| f.label.clone())
        .into_iter()
        .collect();

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

        // Label filter
        if !label_vec.is_empty() {
            builder = builder.labels(&label_vec);
        }

        // Sort
        if let Some(ref sort) = f.sort {
            let sort_param = match sort.as_str() {
                "updated" => params::issues::Sort::Updated,
                "comments" => params::issues::Sort::Comments,
                _ => params::issues::Sort::Created,
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
        .context("Failed to fetch issues")?;

    let issues = page
        .take_items()
        .into_iter()
        // GitHub's API returns PRs in the issues endpoint – filter them out
        .filter(|i| i.pull_request.is_none())
        .map(|i| Issue {
            number: i.number,
            title: i.title,
            state: format!("{:?}", i.state),
            author: i.user.login.clone(),
            labels: i.labels.iter().map(|l| l.name.clone()).collect(),
            assignees: i
                .assignees
                .iter()
                .map(|a| a.login.clone())
                .collect(),
            created_at: i.created_at,
            updated_at: i.updated_at,
            closed_at: i.closed_at,
            html_url: i.html_url.to_string(),
            body: i.body,
        })
        .collect();

    Ok(issues)
}
