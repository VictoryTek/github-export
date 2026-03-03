use anyhow::{Context, Result};
use octocrab::Octocrab;
use serde::Deserialize;

use crate::models::SecurityAlert;

/// Raw Dependabot alert from the GitHub REST API.
/// The octocrab crate does not have first-class support for this endpoint yet,
/// so we call it manually via `Octocrab::_get`.
#[derive(Debug, Deserialize)]
struct RawDependabotAlert {
    number: u64,
    state: String,
    html_url: String,
    created_at: String,
    security_advisory: Option<RawAdvisory>,
    security_vulnerability: Option<RawVulnerability>,
}

#[derive(Debug, Deserialize)]
struct RawAdvisory {
    summary: Option<String>,
    description: Option<String>,
    severity: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RawVulnerability {
    package: Option<RawPackage>,
    vulnerable_version_range: Option<String>,
    first_patched_version: Option<RawPatchedVersion>,
}

#[derive(Debug, Deserialize)]
struct RawPackage {
    name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RawPatchedVersion {
    identifier: Option<String>,
}

/// Fetch Dependabot security alerts for a repository.
///
/// Requires the `security_events` scope on the personal access token or
/// the repository to have Dependabot alerts enabled.
pub async fn fetch_alerts(
    client: &Octocrab,
    owner: &str,
    repo: &str,
) -> Result<Vec<SecurityAlert>> {
    let url = format!("/repos/{owner}/{repo}/dependabot/alerts?per_page=50&state=open");

    let raw_alerts: Vec<RawDependabotAlert> = client
        .get(&url, None::<&()>)
        .await
        .context("Failed to fetch Dependabot alerts (ensure token has `security_events` scope)")?;

    let alerts = raw_alerts
        .into_iter()
        .map(|a| {
            let advisory = a.security_advisory.as_ref();
            let vuln = a.security_vulnerability.as_ref();

            SecurityAlert {
                id: a.number,
                severity: advisory
                    .and_then(|ad| ad.severity.clone())
                    .unwrap_or_else(|| "unknown".into()),
                summary: advisory
                    .and_then(|ad| ad.summary.clone())
                    .unwrap_or_default(),
                description: advisory
                    .and_then(|ad| ad.description.clone())
                    .unwrap_or_default(),
                package_name: vuln.and_then(|v| v.package.as_ref().and_then(|p| p.name.clone())),
                vulnerable_version_range: vuln
                    .and_then(|v| v.vulnerable_version_range.clone()),
                patched_version: vuln.and_then(|v| {
                    v.first_patched_version
                        .as_ref()
                        .and_then(|p| p.identifier.clone())
                }),
                state: a.state,
                html_url: a.html_url,
                created_at: a
                    .created_at
                    .parse()
                    .unwrap_or_else(|_| chrono::Utc::now()),
            }
        })
        .collect();

    Ok(alerts)
}
