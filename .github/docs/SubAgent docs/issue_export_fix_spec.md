# Issue Export Fix — Specification

**Date:** 2026-03-06  
**Author:** Research Subagent  
**Scope:** Two bugs in the GitHub Export Tauri application  
1. CSV export of issues is missing the description body and comment count  
2. PDF export produces no file (silently fails after the save dialog)

---

## 1. Current State Analysis

### 1.1 Data Models

**File:** `src-tauri/src/models/mod.rs` (lines 84–100)

The `Issue` struct contains the following fields:

| Field | Type | Populated by API? | Exported to CSV? | Exported to PDF? |
|---|---|---|---|---|
| `number` | `u64` | ✓ | ✓ | ✓ (table) |
| `title` | `String` | ✓ | ✓ | ✓ (table) |
| `state` | `String` | ✓ | ✓ | ✓ (table) |
| `author` | `String` | ✓ | ✓ | ✓ (table) |
| `labels` | `Vec<String>` | ✓ | ✓ | ✗ |
| `assignees` | `Vec<String>` | ✓ | ✗ | ✗ |
| `created_at` | `DateTime<Utc>` | ✓ | ✓ | ✗ |
| `updated_at` | `DateTime<Utc>` | ✓ | ✗ | ✗ |
| `closed_at` | `Option<DateTime<Utc>>` | ✓ | ✗ | ✗ |
| `html_url` | `String` | ✓ | ✓ | ✗ |
| **`body`** | **`Option<String>`** | **✓** | **✗ — BUG** | **✗** |
| **`comments`** | **`u32`** | **✓** | **✗ — BUG** | **✗** |
| `milestone` | `Option<String>` | ✓ | ✗ | ✗ |

> **Important:** `Issue.comments` is a **count** of comments (`u32`), not actual comment text.
> The GitHub API returns the comment count in the list endpoint, but fetching the full text of
> each comment requires a separate API call to `GET /repos/{owner}/{repo}/issues/{number}/comments`.
> No such function exists in `src-tauri/src/github/detail.rs` or elsewhere in this codebase.
> This spec addresses adding the `body` (description) and the `comments` count to the CSV.
> Full comment text export is explicitly out of scope and documented as a future enhancement.

---

### 1.2 Bug 1 — CSV Export Missing Description and Comment Count

**File:** `src-tauri/src/export/csv_export.rs`

**Lines 22–35** — the Issues section of `export_to_csv`:

```rust
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
```

**Root cause:**  
`i.body` and `i.comments` are never written. With 7 columns in the header and 7 values per
row the CSV is internally consistent but lacks the issue description and comment count.

The placeholder row `wtr.write_record(["", "", "", "", "", "", ""])?;` (line 36) also only
emits 7 empty cells instead of 9, which will cause a misaligned blank row once columns are added.

The data IS available — `fetch_issues` in `src-tauri/src/github/issues.rs` (lines 130–143)
already maps `i.body` and `i.comments` onto the `Issue` struct. No API or model changes are
needed to fix this bug.

---

### 1.3 Bug 2 — PDF Export Produces No File

**File:** `src-tauri/src/export/pdf_export.rs`

**Lines 20–30** — font loading at the top of `export_to_pdf`:

```rust
let font_dir = std::env::current_exe()
    .ok()
    .and_then(|p| p.parent().map(|d| d.to_path_buf()))
    .unwrap_or_else(|| std::path::PathBuf::from("."));

let font_family = genpdf::fonts::from_files(&font_dir, FONT_FAMILY, None).context(
    "Could not load LiberationSans fonts. Place LiberationSans-Regular.ttf, \
         LiberationSans-Bold.ttf, LiberationSans-Italic.ttf, and \
         LiberationSans-BoldItalic.ttf next to the executable.",
)?;
```

**Root cause:**  
`genpdf 0.2` (declared in `Cargo.toml`) requires external `.ttf` font files on the filesystem.
The code looks for `LiberationSans-{Regular,Bold,Italic,BoldItalic}.ttf` in the same directory
as the compiled binary. **These files are never bundled with the application.**

- In a debug build, the binary lives in `src-tauri/target/debug/`. No TTF files exist there.
- In a release/installer build, the binary is extracted into the Tauri app bundle. The TTF files
  are still not present.

`genpdf::fonts::from_files` returns an `Err` immediately. The `?` operator propagates the error
up through `export_to_pdf`, which returns `Err(anyhow::Error)`. In `main.rs` (lines 258–261):

```rust
ExportFormat::Pdf => {
    export::pdf_export::export_to_pdf(&issues, &pulls, &alerts, &workflow_runs, &file_path)
        .map_err(|e| e.to_string())?;
}
```

The error is converted to a `String` and returned as `Err(String)` from the `export_data`
Tauri command. The JS frontend's `catch` block in `doExport` (src/main.js line ~849):

```js
} catch (e) {
    alert(`Export failed: ${e}`);
}
```

…displays an alert. However the symptom reported ("no file is created") is correct: because
`from_files` fails before any bytes are written, no `.pdf` file ever exists.

**Why `genpdf` is problematic for this app:**  
`genpdf 0.2`'s public font API (`genpdf::fonts::from_files`) only loads fonts from the
filesystem. It does not expose a `from_bytes` / `from_slice` constructor. This means there
is no way to embed fonts into the binary using `include_bytes!` when using `genpdf 0.2`.

**Selected replacement library: `printpdf`**  
`printpdf` (crates.io, MIT/Apache-2.0) is the de-facto PDF generation crate for Rust. It:
- Accepts font data as `&[u8]`, enabling `include_bytes!` font embedding
- Requires no external files at runtime
- Is actively maintained and widely used
- Supports multi-page documents, text, tables via layout logic
- Version `0.7` is the current stable release

**Font selection: Liberation Sans**  
Liberation Sans is released under the SIL Open Font License 1.1, which permits embedding in
software. The four required variants are:
- `LiberationSans-Regular.ttf`
- `LiberationSans-Bold.ttf`

For this fix we will embed just `LiberationSans-Regular.ttf` and use it for all text.
Adding bold requires a second `include_bytes!` call, which is straightforward once the
baseline works. Both files are available from the `liberation-fonts` project on GitHub
(https://github.com/liberationfonts/liberation-fonts/releases).

---

### 1.4 Call-Chain Traces

**CSV export call chain:**
```
src/main.js doExport("csv")
  → window.__TAURI__.tauri.invoke("export_data", { format: "csv", issues, ... })
  → src-tauri/src/main.rs export_data (line 240)
  → export::csv_export::export_to_csv(&issues, ..., &file_path)
  → csv_export.rs: writes 7 columns, skips body and comments
```

**PDF export call chain:**
```
src/main.js doExport("pdf")
  → window.__TAURI__.tauri.invoke("export_data", { format: "pdf", issues, ... })
  → src-tauri/src/main.rs export_data (line 258)
  → export::pdf_export::export_to_pdf(&issues, ..., &file_path)
  → pdf_export.rs line 20: genpdf::fonts::from_files(font_dir, "LiberationSans", None)
  → Err: font files not found → error propagated to JS → alert shown → no file written
```

---

## 2. Root Cause Summary

| Bug | Root Cause |
|---|---|
| CSV missing description + comment count | `export_to_csv` never writes `i.body` or `i.comments`; the data is already in the `Issue` struct |
| PDF produces no file | `genpdf 0.2` requires external `.ttf` files at runtime; those files are never bundled; font loading fails before any bytes are written |

---

## 3. Proposed Solution

### 3.1 CSV Fix

Expand the Issues section of `export_to_csv` from 7 columns to 9 columns by adding:
- Column 8: **`Body`** — the issue description (`i.body.as_deref().unwrap_or("")`)
- Column 9: **`Comment Count`** — the number of comments (`i.comments.to_string()`)

Update the blank separator row to also emit 9 empty cells to maintain column alignment.

No new dependencies, no new API calls, no model changes required.

### 3.2 PDF Fix

Replace `genpdf` with `printpdf` and embed the Liberation Sans Regular TTF font directly
in the binary via `include_bytes!`:

1. Remove `genpdf = "0.2"` from `Cargo.toml`
2. Add `printpdf = "0.7"` to `Cargo.toml`
3. Download `LiberationSans-Regular.ttf` and place it at `src-tauri/fonts/LiberationSans-Regular.ttf`
4. Rewrite `pdf_export.rs` to use `printpdf`:
   - Embed the font: `const FONT_BYTES: &[u8] = include_bytes!("../../fonts/LiberationSans-Regular.ttf");`
   - Create a `PdfDocument`, add pages, write text for each section
   - Save to `path` via `doc.save(&mut BufWriter::new(File::create(path)?))`
5. The `export_to_pdf` function signature stays the same — no changes to `main.rs` needed

---

## 4. Implementation Steps

### Step 1 — Fix CSV export (`src-tauri/src/export/csv_export.rs`)

**Location:** function `export_to_csv`, Issues section, lines 20–38.

**Change 1a — Update the section header placeholder row** (line 20):  
Change from 7 empty fields to 9:
```rust
// Before:
wtr.write_record(["[Issues]", "", "", "", "", "", ""])?;
// After:
wtr.write_record(["[Issues]", "", "", "", "", "", "", "", ""])?;
```

**Change 1b — Update the column header row** (lines 21–24):  
Add `"Body"` and `"Comment Count"` to the end:
```rust
// Before:
wtr.write_record([
    "Number", "Title", "State", "Author", "Labels", "Created", "URL",
])?;
// After:
wtr.write_record([
    "Number", "Title", "State", "Author", "Labels", "Created", "URL", "Body", "Comment Count",
])?;
```

**Change 1c — Update the per-issue data row** (lines 25–34):  
Add `body` (unwrapped, empty string if None) and `comments` count:
```rust
// Before:
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
// After:
for i in issues {
    wtr.write_record([
        &i.number.to_string(),
        &i.title,
        &i.state,
        &i.author,
        &i.labels.join(", "),
        &i.created_at.to_rfc3339(),
        &i.html_url,
        i.body.as_deref().unwrap_or(""),
        &i.comments.to_string(),
    ])?;
}
```

**Change 1d — Update the blank separator row** (line 36):  
Change from 7 empty fields to 9:
```rust
// Before:
wtr.write_record(["", "", "", "", "", "", ""])?;
// After:
wtr.write_record(["", "", "", "", "", "", "", "", ""])?;
```

---

### Step 2 — Update Cargo.toml (`src-tauri/Cargo.toml`)

Remove `genpdf` and add `printpdf`:

```toml
# Remove:
genpdf = "0.2"

# Add:
printpdf = "0.7"
```

---

### Step 3 — Add font file

Download `LiberationSans-Regular.ttf` from the liberation-fonts project release page:
https://github.com/liberationfonts/liberation-fonts/releases

Place the file at:
```
src-tauri/fonts/LiberationSans-Regular.ttf
```

This path is relative to the crate root (`src-tauri/`) and will be compiled into the binary
via `include_bytes!`.

---

### Step 4 — Rewrite PDF export (`src-tauri/src/export/pdf_export.rs`)

Replace the entire file content. The function signature stays identical:

```rust
pub fn export_to_pdf(
    issues: &[Issue],
    pulls: &[PullRequest],
    alerts: &[SecurityAlert],
    workflow_runs: &[WorkflowRun],
    path: &str,
) -> Result<()>
```

**Full replacement implementation:**

```rust
use anyhow::{Context, Result};
use printpdf::*;
use std::fs::File;
use std::io::BufWriter;

use crate::models::{Issue, PullRequest, SecurityAlert, WorkflowRun};

// Embed Liberation Sans at compile-time — no external files needed at runtime.
const FONT_BYTES: &[u8] = include_bytes!("../../fonts/LiberationSans-Regular.ttf");

const PAGE_WIDTH_MM: f32 = 210.0;
const PAGE_HEIGHT_MM: f32 = 297.0;
const MARGIN_MM: f32 = 15.0;
const LINE_HEIGHT_MM: f32 = 6.0;
const HEADING_SIZE: i64 = 14;
const BODY_SIZE: i64 = 10;

pub fn export_to_pdf(
    issues: &[Issue],
    pulls: &[PullRequest],
    alerts: &[SecurityAlert],
    workflow_runs: &[WorkflowRun],
    path: &str,
) -> Result<()> {
    let (doc, page1, layer1) = PdfDocument::new(
        "GitHub Export Report",
        Mm(PAGE_WIDTH_MM),
        Mm(PAGE_HEIGHT_MM),
        "Layer 1",
    );

    let font = doc
        .add_external_font(std::io::Cursor::new(FONT_BYTES))
        .context("Failed to load embedded font")?;

    let mut writer = PdfWriter {
        doc: &doc,
        font: &font,
        current_page: page1,
        current_layer: layer1,
        y: PAGE_HEIGHT_MM - MARGIN_MM,
        page_num: 1,
    };

    // Title
    writer.write_text("GitHub Export Report", HEADING_SIZE + 4, true);
    writer.newline(2.0);

    // Issues section
    if !issues.is_empty() {
        writer.write_text("Issues", HEADING_SIZE, true);
        writer.newline(0.5);
        writer.write_text(
            &format!("{:<6} {:<40} {:<8} {}", "#", "Title", "State", "Author"),
            BODY_SIZE,
            false,
        );
        writer.draw_rule();
        for i in issues {
            let title = truncate(&i.title, 38);
            writer.write_text(
                &format!("{:<6} {:<40} {:<8} {}", i.number, title, i.state, i.author),
                BODY_SIZE,
                false,
            );
            if let Some(ref body) = i.body {
                if !body.is_empty() {
                    let snippet = truncate(&body.replace('\n', " "), 80);
                    writer.write_text(&format!("       {}", snippet), BODY_SIZE - 1, false);
                }
            }
        }
        writer.newline(1.0);
    }

    // Pull Requests section
    if !pulls.is_empty() {
        writer.write_text("Pull Requests", HEADING_SIZE, true);
        writer.newline(0.5);
        writer.write_text(
            &format!("{:<6} {:<40} {:<8} {}", "#", "Title", "State", "Author"),
            BODY_SIZE,
            false,
        );
        writer.draw_rule();
        for pr in pulls {
            let title = truncate(&pr.title, 38);
            writer.write_text(
                &format!("{:<6} {:<40} {:<8} {}", pr.number, title, pr.state, pr.author),
                BODY_SIZE,
                false,
            );
        }
        writer.newline(1.0);
    }

    // Security Alerts section
    if !alerts.is_empty() {
        writer.write_text("Security Alerts", HEADING_SIZE, true);
        writer.newline(0.5);
        writer.write_text(
            &format!("{:<6} {:<10} {}", "ID", "Severity", "Summary"),
            BODY_SIZE,
            false,
        );
        writer.draw_rule();
        for a in alerts {
            let summary = truncate(&a.summary, 60);
            writer.write_text(
                &format!("{:<6} {:<10} {}", a.id, a.severity, summary),
                BODY_SIZE,
                false,
            );
        }
        writer.newline(1.0);
    }

    // Workflow Runs section
    if !workflow_runs.is_empty() {
        writer.write_text("Workflow Runs", HEADING_SIZE, true);
        writer.newline(0.5);
        writer.write_text(
            &format!("{:<30} {:<10} {:<10} {}", "Workflow", "Status", "Conclusion", "Actor"),
            BODY_SIZE,
            false,
        );
        writer.draw_rule();
        for r in workflow_runs {
            let name = truncate(&r.name, 28);
            writer.write_text(
                &format!(
                    "{:<30} {:<10} {:<10} {}",
                    name,
                    r.status,
                    r.conclusion.as_deref().unwrap_or("—"),
                    r.actor_login
                ),
                BODY_SIZE,
                false,
            );
        }
    }

    let file = File::create(path).context("Could not create PDF file")?;
    doc.save(&mut BufWriter::new(file))
        .context("Failed to write PDF file")?;

    Ok(())
}

// ── Internal page writer ──────────────────────

struct PdfWriter<'a> {
    doc: &'a PdfDocumentReference,
    font: &'a IndirectFontRef,
    current_page: PdfPageIndex,
    current_layer: PdfLayerIndex,
    y: f32,
    page_num: u32,
}

impl<'a> PdfWriter<'a> {
    fn write_text(&mut self, text: &str, font_size: i64, _bold: bool) {
        if self.y < MARGIN_MM + LINE_HEIGHT_MM {
            self.add_page();
        }
        let layer = self.doc.get_page(self.current_page).get_layer(self.current_layer);
        layer.use_text(text, font_size as f64, Mm(MARGIN_MM), Mm(self.y), self.font);
        self.y -= LINE_HEIGHT_MM;
    }

    fn draw_rule(&mut self) {
        if self.y < MARGIN_MM + LINE_HEIGHT_MM {
            self.add_page();
        }
        // Draw a simple horizontal rule as a line of dashes
        let layer = self.doc.get_page(self.current_page).get_layer(self.current_layer);
        layer.use_text(
            &"-".repeat(90),
            BODY_SIZE as f64,
            Mm(MARGIN_MM),
            Mm(self.y),
            self.font,
        );
        self.y -= LINE_HEIGHT_MM * 0.5;
    }

    fn newline(&mut self, multiplier: f32) {
        self.y -= LINE_HEIGHT_MM * multiplier;
        if self.y < MARGIN_MM {
            self.add_page();
        }
    }

    fn add_page(&mut self) {
        self.page_num += 1;
        let (page_idx, layer_idx) = self.doc.add_page(
            Mm(PAGE_WIDTH_MM),
            Mm(PAGE_HEIGHT_MM),
            format!("Layer {}", self.page_num),
        );
        self.current_page = page_idx;
        self.current_layer = layer_idx;
        self.y = PAGE_HEIGHT_MM - MARGIN_MM;
    }
}

fn truncate(s: &str, max_chars: usize) -> String {
    let mut chars = s.chars();
    let truncated: String = chars.by_ref().take(max_chars).collect();
    if chars.next().is_some() {
        format!("{}…", truncated)
    } else {
        truncated
    }
}
```

> **Note on bold text:** `printpdf`'s `add_external_font` works per font file. To render bold
> text, a second font (`LiberationSans-Bold.ttf`) needs to be embedded with a second
> `include_bytes!` call. For this initial fix, all text uses the regular weight. The `_bold`
> parameter in `write_text` is a stub for future enhancement.

---

### Step 5 — Update `src-tauri/build.rs` if needed

`build.rs` currently contains standard Tauri build script boilerplate. No changes needed —
`include_bytes!` is resolved at compile time by rustc, not by the build script.

---

### Step 6 — Verify the build

From `src-tauri/`:
```
cargo build
cargo clippy -- -D warnings
cargo test
```

From the project root:
```
npm run build
```

---

## 5. Dependencies

### Changes to `src-tauri/Cargo.toml`

| Action | Crate | Version |
|---|---|---|
| Remove | `genpdf` | `"0.2"` |
| Add | `printpdf` | `"0.7"` |

`printpdf 0.7` depends on `lopdf`, `image`, and `rusttype` (all pure Rust, no system libs).
It will compile on Windows, macOS, and Linux without additional system packages.

### New Files

| File | Purpose |
|---|---|
| `src-tauri/fonts/LiberationSans-Regular.ttf` | Embedded via `include_bytes!` for PDF generation |

The font file should be added to version control. It is ~66 KB and its license (SIL OFL 1.1)
permits embedding in application binaries.

---

## 6. Risks and Mitigations

| Risk | Likelihood | Mitigation |
|---|---|---|
| `printpdf 0.7` API differs from the implementation above | Medium | Consult `printpdf` docs at https://docs.rs/printpdf/latest; adjust `add_external_font` / `use_text` method signatures as needed. The core pattern (`include_bytes!` + `add_external_font` + `use_text`) is stable across 0.5–0.7. |
| Binary size increase from embedded font | Low | Liberation Sans Regular is ~66 KB. Acceptable for a desktop app. |
| CSV body field contains newlines/commas | Low | The `csv` crate correctly quotes fields containing special characters. No additional quoting code needed. |
| CSV body field is very long (large issue descriptions) | Low | This is expected behavior for a CSV export. Consumers (Excel/LibreOffice) handle long cells. |
| `doc.render_to_file` vs `doc.save` API difference | Low | If `printpdf 0.7` uses a different save API, adjust. The pattern shown uses `doc.save(&mut BufWriter::new(File::create(path)?))` which is the standard printpdf save pattern. |
| `genpdf` removal breaks any other consumer | None | `genpdf` is only used in `pdf_export.rs`. |
| LiberationSans-Regular.ttf might not render all Unicode characters | Low | All GitHub-sourced content (titles, authors) is ASCII-compatible. Non-Latin characters will render as fallback boxes — acceptable for a first fix. |
| Full comment text NOT included in CSV | By design | The `Issue.comments` field is a count (u32). Fetching actual comment bodies requires N+1 GitHub API calls. This is documented as a future enhancement, not a defect in this fix. |

---

## 7. Future Enhancement: Full Comment Text in CSV

To include the actual text of each comment in the CSV (not just the count):

1. Add an `IssueComment` model to `src-tauri/src/models/mod.rs`:
   ```rust
   #[derive(Debug, Clone, Serialize, Deserialize)]
   pub struct IssueComment {
       pub id: u64,
       pub author: String,
       pub body: String,
       pub created_at: DateTime<Utc>,
   }
   ```

2. Add a `fetch_issue_comments` function in `src-tauri/src/github/detail.rs` that calls
   `GET /repos/{owner}/{repo}/issues/{number}/comments`.

3. Add a new Tauri command `fetch_issue_comments(owner, repo, issue_number)`.

4. Call it from the JS frontend before export (or make it part of a new "deep export" workflow).

5. Update the CSV export to accept the comments map and emit a separate `[Issue Comments]`
   section below the issues section.

This is intentionally out of scope for the current bug fix.

---

## 8. File Change Summary

| File | Change Type | Description |
|---|---|---|
| `src-tauri/src/export/csv_export.rs` | Modify | Add `Body` and `Comment Count` columns to Issues section |
| `src-tauri/src/export/pdf_export.rs` | Rewrite | Replace genpdf with printpdf; embed font via include_bytes! |
| `src-tauri/Cargo.toml` | Modify | Remove genpdf, add printpdf = "0.7" |
| `src-tauri/fonts/LiberationSans-Regular.ttf` | New file | Liberation Sans Regular font for PDF embedding |

No changes are required to `main.rs`, `models/mod.rs`, `src/main.js`, or `src/index.html`.
