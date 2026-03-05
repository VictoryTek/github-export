use anyhow::Result;
use octocrab::Octocrab;
use serde::Deserialize;

use crate::models::SecurityAlert;

#[derive(Debug, Deserialize)]
struct RawCvss {
    score: Option<f64>,
}

#[derive(Debug, Deserialize)]
struct RawCwe {
    cwe_id: Option<String>,
}

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
    dismissed_reason: Option<String>,
    dismissed_comment: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RawAdvisory {
    summary: Option<String>,
    description: Option<String>,
    severity: Option<String>,
    cve_id: Option<String>,
    cvss: Option<RawCvss>,
    cwes: Option<Vec<RawCwe>>,
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
    state: Option<&str>,
) -> Result<Vec<SecurityAlert>> {
    // Security alert states don't map to Issues' open/closed model.
    // - UI "open"  → API state=open
    // - UI "closed" or "all" → omit state param (returns all alerts)
    // This ensures the default "Open" filter shows open alerts, and selecting
    // "All" or "Closed" shows everything.
    let url = if state == Some("open") {
        format!("/repos/{owner}/{repo}/dependabot/alerts?per_page=100&state=open")
    } else {
        format!("/repos/{owner}/{repo}/dependabot/alerts?per_page=100")
    };

    let raw_alerts: Vec<RawDependabotAlert> = client
        .get(&url, None::<&()>)
        .await
        .map_err(|e| {
            // Extract the actual GitHub error message if available
            let detail = match &e {
                octocrab::Error::GitHub { source, .. } => {
                    format!("GitHub API error {}: {}", source.status_code, source.message)
                }
                other => other.to_string(),
            };
            anyhow::anyhow!(
                "Failed to fetch Dependabot alerts: {}\n\
                \n\
                Token permission requirements:\n\
                • Fine-grained PAT: under Repository permissions → set 'Dependabot alerts' to Read\n\
                • Classic PAT: check the 'security_events' scope\n\
                \n\
                Also ensure Dependabot alerts are enabled for this repository \
                (Settings → Security → Advanced Security → Dependabot Alerts → Enable).",
                detail
            )
        })?;

    let mut dependabot_alerts: Vec<SecurityAlert> = raw_alerts
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
                vulnerable_version_range: vuln.and_then(|v| v.vulnerable_version_range.clone()),
                patched_version: vuln.and_then(|v| {
                    v.first_patched_version
                        .as_ref()
                        .and_then(|p| p.identifier.clone())
                }),
                state: a.state,
                html_url: a.html_url,
                created_at: a.created_at.parse().unwrap_or_else(|_| chrono::Utc::now()),
                alert_type: "dependabot".to_string(),
                tool_name: None,
                location_path: None,
                cve_id: advisory.and_then(|ad| ad.cve_id.clone()),
                cvss_score: advisory.and_then(|ad| ad.cvss.as_ref().and_then(|c| c.score)),
                cwes: advisory
                    .and_then(|ad| ad.cwes.as_ref())
                    .map(|cwes| cwes.iter().filter_map(|c| c.cwe_id.clone()).collect())
                    .unwrap_or_default(),
                dismissed_reason: a.dismissed_reason,
                dismissed_comment: a.dismissed_comment,
            }
        })
        .collect();

    let code_scanning = fetch_code_scanning_alerts(client, owner, repo, state)
        .await
        .unwrap_or_default();
    dependabot_alerts.extend(code_scanning);
    Ok(dependabot_alerts)
}

// ── Code Scanning ────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct CodeScanningAlertRule {
    id: Option<String>,
    description: Option<String>,
    severity: Option<String>,
}

#[derive(Debug, Deserialize)]
struct CodeScanningAlertTool {
    name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct CodeScanningLocation {
    path: Option<String>,
}

#[derive(Debug, Deserialize)]
struct CodeScanningAlertInstance {
    location: Option<CodeScanningLocation>,
}

#[derive(Debug, Deserialize)]
struct RawCodeScanningAlert {
    number: u64,
    state: String,
    rule: CodeScanningAlertRule,
    tool: CodeScanningAlertTool,
    most_recent_instance: Option<CodeScanningAlertInstance>,
    created_at: String,
    html_url: String,
}

async fn fetch_code_scanning_alerts(
    client: &Octocrab,
    owner: &str,
    repo: &str,
    state: Option<&str>,
) -> anyhow::Result<Vec<SecurityAlert>> {
    let url = if state == Some("open") {
        format!("/repos/{owner}/{repo}/code-scanning/alerts?per_page=100&state=open")
    } else {
        format!("/repos/{owner}/{repo}/code-scanning/alerts?per_page=100")
    };
    let response: Result<Vec<RawCodeScanningAlert>, _> = client.get(&url, None::<&()>).await;
    match response {
        Ok(alerts) => Ok(alerts
            .into_iter()
            .map(|a| {
                let summary =
                    a.rule.description.clone().unwrap_or_else(|| {
                        a.rule.id.clone().unwrap_or_else(|| "Unknown".to_string())
                    });
                SecurityAlert {
                    id: a.number,
                    severity: a.rule.severity.unwrap_or_else(|| "unknown".to_string()),
                    summary,
                    description: String::new(),
                    package_name: None,
                    vulnerable_version_range: None,
                    patched_version: None,
                    state: a.state,
                    html_url: a.html_url,
                    created_at: a.created_at.parse().unwrap_or_else(|_| chrono::Utc::now()),
                    alert_type: "code_scanning".to_string(),
                    tool_name: a.tool.name,
                    location_path: a
                        .most_recent_instance
                        .as_ref()
                        .and_then(|inst| inst.location.as_ref())
                        .and_then(|loc| loc.path.clone()),
                    cve_id: None,
                    cvss_score: None,
                    cwes: vec![],
                    dismissed_reason: None,
                    dismissed_comment: None,
                }
            })
            .collect()),
        Err(e) => {
            eprintln!("Code scanning alerts error (ignored): {}", e);
            Ok(vec![])
        }
    }
}
