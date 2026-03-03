use anyhow::{Context, Result};
use genpdf::elements::{Break, Paragraph, TableLayout};
use genpdf::style::Style;
use genpdf::{Document, Element, SimplePageDecorator};

use crate::models::{Issue, PullRequest, SecurityAlert};

/// Default font family bundled with genpdf (Liberation Sans).
const FONT_FAMILY: &str = "LiberationSans";

/// Export issues, pull requests, and security alerts to a PDF report.
pub fn export_to_pdf(
    issues: &[Issue],
    pulls: &[PullRequest],
    alerts: &[SecurityAlert],
    path: &str,
) -> Result<()> {
    // genpdf requires a font directory – we bundle Liberation Sans which ships
    // with most Linux distros, or the user can place the .ttf files next to the
    // binary.  On first run the font directory should be resolved correctly.
    let font_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()))
        .unwrap_or_else(|| std::path::PathBuf::from("."));

    let font_family = genpdf::fonts::from_files(&font_dir, FONT_FAMILY, None)
        .context(
            "Could not load LiberationSans fonts. Place LiberationSans-Regular.ttf, \
             LiberationSans-Bold.ttf, LiberationSans-Italic.ttf, and \
             LiberationSans-BoldItalic.ttf next to the executable.",
        )?;

    let mut doc = Document::new(font_family);
    doc.set_title("GitHub Export Report");

    let mut decorator = SimplePageDecorator::new();
    decorator.set_margins(10);
    doc.set_page_decorator(decorator);

    // ── Title ───────────────────────────────
    doc.push(
        Paragraph::new("GitHub Export Report")
            .styled(Style::new().bold().with_font_size(20)),
    );
    doc.push(Break::new(1));

    // ── Issues ──────────────────────────────
    if !issues.is_empty() {
        doc.push(section_heading("Issues"));
        let mut table = TableLayout::new(vec![1, 4, 1, 2]);
        table.set_cell_decorator(genpdf::elements::FrameCellDecorator::new(true, true, false));
        // Header row
        push_table_row(&mut table, &["#", "Title", "State", "Author"]);
        for i in issues {
            push_table_row(
                &mut table,
                &[
                    &i.number.to_string(),
                    &i.title,
                    &i.state,
                    &i.author,
                ],
            );
        }
        doc.push(table);
        doc.push(Break::new(1));
    }

    // ── Pull Requests ───────────────────────
    if !pulls.is_empty() {
        doc.push(section_heading("Pull Requests"));
        let mut table = TableLayout::new(vec![1, 4, 1, 2]);
        table.set_cell_decorator(genpdf::elements::FrameCellDecorator::new(true, true, false));
        push_table_row(&mut table, &["#", "Title", "State", "Author"]);
        for pr in pulls {
            push_table_row(
                &mut table,
                &[
                    &pr.number.to_string(),
                    &pr.title,
                    &pr.state,
                    &pr.author,
                ],
            );
        }
        doc.push(table);
        doc.push(Break::new(1));
    }

    // ── Security Alerts ─────────────────────
    if !alerts.is_empty() {
        doc.push(section_heading("Security Alerts"));
        let mut table = TableLayout::new(vec![1, 1, 4, 2]);
        table.set_cell_decorator(genpdf::elements::FrameCellDecorator::new(true, true, false));
        push_table_row(&mut table, &["ID", "Severity", "Summary", "Package"]);
        for a in alerts {
            push_table_row(
                &mut table,
                &[
                    &a.id.to_string(),
                    &a.severity,
                    &a.summary,
                    a.package_name.as_deref().unwrap_or("—"),
                ],
            );
        }
        doc.push(table);
    }

    doc.render_to_file(path)
        .context("Failed to write PDF file")?;

    Ok(())
}

// ── Helpers ─────────────────────────────────

fn section_heading(text: &str) -> impl Element {
    Paragraph::new(text).styled(Style::new().bold().with_font_size(14))
}

fn push_table_row(table: &mut TableLayout, cells: &[&str]) {
    let mut row = table.row();
    for cell in cells {
        row.push_element(Paragraph::new(*cell));
    }
    row.push().expect("Failed to push table row");
}
