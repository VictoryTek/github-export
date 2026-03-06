use anyhow::{Context, Result};
use csv::Writer;
use std::fs::File;

use crate::models::{Issue, PullRequest, SecurityAlert};

/// Export issues, pull requests, and security alerts to a single CSV file,
/// broken into labelled sections.
pub fn export_to_csv(
    issues: &[Issue],
    pulls: &[PullRequest],
    alerts: &[SecurityAlert],
    path: &str,
) -> Result<()> {
    let file = File::create(path).context("Could not create CSV file")?;
    let mut wtr = Writer::from_writer(file);

    // ── Issues ──────────────────────────────
    if !issues.is_empty() {
        wtr.write_record(["[Issues]", "", "", "", "", "", ""])?;
        wtr.write_record([
            "Number", "Title", "State", "Author", "Labels", "Created", "URL",
        ])?;
        for i in issues {
            wtr.write_record([
                &i.number.to_string(),
                &i.title,
                &i.state,
                &i.author,
                &i.labels.join(", "),
                &i.created_at.to_rfc3339(),
                &i.html_url,
            ])?;
        }
        wtr.write_record(["", "", "", "", "", "", ""])?;
    }

    // ── Pull Requests ───────────────────────
    if !pulls.is_empty() {
        wtr.write_record(["[Pull Requests]", "", "", "", "", "", "", ""])?;
        wtr.write_record([
            "Number", "Title", "State", "Author", "Head", "Base", "Draft", "URL",
        ])?;
        for pr in pulls {
            wtr.write_record([
                &pr.number.to_string(),
                &pr.title,
                &pr.state,
                &pr.author,
                &pr.head_branch,
                &pr.base_branch,
                &pr.draft.to_string(),
                &pr.html_url,
            ])?;
        }
        wtr.write_record(["", "", "", "", "", "", "", ""])?;
    }

    // ── Security Alerts ─────────────────────
    if !alerts.is_empty() {
        wtr.write_record(["[Security Alerts]", "", "", "", "", "", ""])?;
        wtr.write_record([
            "ID",
            "Severity",
            "Summary",
            "Package",
            "Vulnerable Range",
            "Patched",
            "URL",
        ])?;
        for a in alerts {
            wtr.write_record([
                &a.id.to_string(),
                &a.severity,
                &a.summary,
                a.package_name.as_deref().unwrap_or("—"),
                a.vulnerable_version_range.as_deref().unwrap_or("—"),
                a.patched_version.as_deref().unwrap_or("—"),
                &a.html_url,
            ])?;
        }
    }

    wtr.flush()?;
    Ok(())
}

/// Export workflow runs to a standalone CSV file.
pub fn export_actions_csv(runs: &[crate::models::WorkflowRun], path: &str) -> anyhow::Result<()> {
    let file = std::fs::File::create(path).context("Could not create CSV file")?;
    let mut wtr = csv::Writer::from_writer(file);

    wtr.write_record(["ID", "Workflow", "Branch", "Status", "Conclusion", "Actor", "Started", "URL"])?;
    for r in runs {
        wtr.write_record([
            &r.id.to_string(),
            &r.name,
            r.head_branch.as_deref().unwrap_or(""),
            &r.status,
            r.conclusion.as_deref().unwrap_or(""),
            &r.actor_login,
            r.run_started_at.as_deref().unwrap_or(&r.created_at),
            &r.html_url,
        ])?;
    }

    wtr.flush()?;
    Ok(())
}
