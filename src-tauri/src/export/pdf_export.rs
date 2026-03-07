use anyhow::Result;
use printpdf::*;
use std::fs::File;
use std::io::BufWriter;

use crate::models::{Issue, PullRequest, SecurityAlert, WorkflowRun};

// A4 page dimensions in millimetres (f32 required by printpdf 0.6 Mm type)
const PAGE_W: f32 = 210.0;
const PAGE_H: f32 = 297.0;
const MARGIN_L: f32 = 15.0;
// Starting y (near the top; PDF origin is bottom-left)
const TOP_Y: f32 = 280.0;
const BOTTOM_Y: f32 = 20.0;
// Vertical step per line of text
const LINE_H: f32 = 5.5;

/// Truncate `s` to `max_chars` characters, appending "..." if longer.
fn truncate(s: &str, max_chars: usize) -> String {
    let chars: Vec<char> = s.chars().collect();
    if chars.len() <= max_chars {
        s.to_owned()
    } else {
        let t: String = chars[..max_chars].iter().collect();
        format!("{}...", t)
    }
}

/// Export issues, pull requests, security alerts, and workflow runs to a PDF report.
/// Uses the PDF standard Helvetica font — no external font files are required.
pub fn export_to_pdf(
    issues: &[Issue],
    pulls: &[PullRequest],
    alerts: &[SecurityAlert],
    workflow_runs: &[WorkflowRun],
    path: &str,
) -> Result<()> {
    let (doc, first_page, first_layer) = PdfDocument::new(
        "GitHub Export Report",
        Mm(PAGE_W),
        Mm(PAGE_H),
        "Layer 1",
    );

    let font = doc
        .add_builtin_font(BuiltinFont::Helvetica)
        .map_err(|e| anyhow::anyhow!("{}", e))?;
    let font_bold = doc
        .add_builtin_font(BuiltinFont::HelveticaBold)
        .map_err(|e| anyhow::anyhow!("{}", e))?;

    let mut layer = doc.get_page(first_page).get_layer(first_layer);
    let mut y: f32 = TOP_Y;

    // Write a line of text at the current y position, then advance downward.
    macro_rules! put_text {
        ($txt:expr, $size:expr, $bold:expr) => {{
            let f = if $bold { &font_bold } else { &font };
            layer.use_text($txt, $size, Mm(MARGIN_L), Mm(y), f);
            y -= LINE_H;
        }};
    }

    // Add a new page if fewer than $n lines of space remain.
    macro_rules! need_space {
        ($n:expr) => {
            if y < BOTTOM_Y + LINE_H * ($n as f32) {
                let (pg, lr) = doc.add_page(Mm(PAGE_W), Mm(PAGE_H), "Layer 1");
                layer = doc.get_page(pg).get_layer(lr);
                y = TOP_Y;
            }
        };
    }

    // ── Document title ─────────────────────────────────────────────────────
    put_text!("GitHub Export Report", 18.0, true);
    y -= 6.0;

    // ── Issues ─────────────────────────────────────────────────────────────
    if !issues.is_empty() {
        need_space!(3);
        put_text!("Issues", 14.0, true);
        y -= 2.0;

        for i in issues {
            need_space!(7);

            put_text!(truncate(&format!("#{}: {}", i.number, i.title), 80), 11.0, true);
            put_text!(
                format!(
                    "State: {}  |  Author: {}  |  Comments: {}",
                    i.state, i.author, i.comments
                ),
                9.0,
                false
            );
            if !i.labels.is_empty() {
                put_text!(
                    format!("Labels: {}", truncate(&i.labels.join(", "), 70)),
                    9.0,
                    false
                );
            }
            put_text!(
                format!("Created: {}", i.created_at.format("%Y-%m-%d")),
                9.0,
                false
            );
            put_text!(truncate(&i.html_url, 90), 9.0, false);
            if let Some(body) = &i.body {
                if !body.is_empty() {
                    put_text!(
                        format!("Body: {}", truncate(&body.replace('\n', " "), 100)),
                        8.0,
                        false
                    );
                }
            }
            y -= 2.0;
        }
    }

    // ── Pull Requests ──────────────────────────────────────────────────────
    if !pulls.is_empty() {
        need_space!(3);
        y -= 4.0;
        put_text!("Pull Requests", 14.0, true);
        y -= 2.0;

        for pr in pulls {
            need_space!(5);

            put_text!(truncate(&format!("#{}: {}", pr.number, pr.title), 80), 11.0, true);
            put_text!(
                format!(
                    "State: {}  |  Author: {}  |  Draft: {}",
                    pr.state, pr.author, pr.draft
                ),
                9.0,
                false
            );
            put_text!(
                format!("Head: {}  ->  Base: {}", pr.head_branch, pr.base_branch),
                9.0,
                false
            );
            put_text!(truncate(&pr.html_url, 90), 9.0, false);
            y -= 2.0;
        }
    }

    // ── Security Alerts ────────────────────────────────────────────────────
    if !alerts.is_empty() {
        need_space!(3);
        y -= 4.0;
        put_text!("Security Alerts", 14.0, true);
        y -= 2.0;

        for a in alerts {
            need_space!(5);

            put_text!(
                truncate(&format!("#{} [{}]: {}", a.id, a.severity, a.summary), 80),
                11.0,
                true
            );
            if let Some(pkg) = &a.package_name {
                put_text!(
                    format!(
                        "Package: {}  Vulnerable: {}  Patched: {}",
                        pkg,
                        a.vulnerable_version_range.as_deref().unwrap_or("N/A"),
                        a.patched_version.as_deref().unwrap_or("N/A"),
                    ),
                    9.0,
                    false
                );
            }
            put_text!(truncate(&a.html_url, 90), 9.0, false);
            y -= 2.0;
        }
    }

    // ── Workflow Runs ──────────────────────────────────────────────────────
    if !workflow_runs.is_empty() {
        need_space!(3);
        y -= 4.0;
        put_text!("Workflow Runs", 14.0, true);
        y -= 2.0;

        for r in workflow_runs {
            need_space!(5);

            put_text!(
                truncate(
                    &format!(
                        "{} [Branch: {}]",
                        r.name,
                        r.head_branch.as_deref().unwrap_or("N/A")
                    ),
                    80
                ),
                11.0,
                true
            );
            put_text!(
                format!(
                    "Status: {}  |  Conclusion: {}  |  Actor: {}",
                    r.status,
                    r.conclusion.as_deref().unwrap_or("N/A"),
                    r.actor_login,
                ),
                9.0,
                false
            );
            put_text!(
                format!(
                    "Started: {}",
                    r.run_started_at.as_deref().unwrap_or(&r.created_at)
                ),
                9.0,
                false
            );
            put_text!(truncate(&r.html_url, 90), 9.0, false);
            y -= 2.0;
        }
    }

    // ── Save ───────────────────────────────────────────────────────────────
    let file = File::create(path).map_err(|e| anyhow::anyhow!("{}", e))?;
    doc.save(&mut BufWriter::new(file))
        .map_err(|e| anyhow::anyhow!("{}", e))?;
    Ok(())
}
