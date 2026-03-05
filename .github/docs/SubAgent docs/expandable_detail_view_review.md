# Expandable Detail View — Review & Quality Assurance

**Project:** GitHub Export (Tauri v1 Desktop App)  
**Feature:** Inline Expandable Detail Rows for Issues, Pull Requests, and Security Alerts  
**Review Date:** 2026-03-05  
**Reviewer:** QA Subagent  
**Spec Reference:** `.github/docs/SubAgent docs/expandable_detail_view_spec.md`

---

## Table of Contents

1. [Files Reviewed](#1-files-reviewed)
2. [Specification Compliance](#2-specification-compliance)
3. [Code Quality — Rust](#3-code-quality--rust)
4. [Code Quality — JavaScript](#4-code-quality--javascript)
5. [Code Quality — CSS](#5-code-quality--css)
6. [Code Quality — HTML](#6-code-quality--html)
7. [Security Review](#7-security-review)
8. [Build Validation](#8-build-validation)
9. [Score Table](#9-score-table)
10. [Critical Issues](#10-critical-issues)
11. [Recommended Improvements](#11-recommended-improvements)
12. [Final Verdict](#12-final-verdict)

---

## 1. Files Reviewed

| File | Status |
|------|--------|
| `src-tauri/src/models/mod.rs` | ✓ Reviewed |
| `src-tauri/src/github/issues.rs` | ✓ Reviewed |
| `src-tauri/src/github/pulls.rs` | ✓ Reviewed |
| `src-tauri/src/github/security.rs` | ✓ Reviewed |
| `src-tauri/src/github/detail.rs` | ✓ Reviewed |
| `src-tauri/src/github/mod.rs` | ✓ Reviewed |
| `src-tauri/src/main.rs` | ✓ Reviewed |
| `src-tauri/src/mock/mod.rs` | ✓ Reviewed |
| `src/vendor/marked.min.js` | ✓ Reviewed |
| `src/vendor/purify.min.js` | ✓ Reviewed |
| `src/index.html` | ✓ Reviewed |
| `src/main.js` | ✓ Reviewed (738 lines) |
| `src/styles.css` | ✓ Reviewed (741 lines) |

---

## 2. Specification Compliance

### 2.1 Backend Model Changes

| Requirement | Status | Notes |
|---|---|---|
| `Issue` gains `comments: u32` | ✓ PASS | Present in `models/mod.rs` and mapped in `issues.rs` |
| `Issue` gains `milestone: Option<String>` | ✓ PASS | Present and mapped via `i.milestone.as_ref().map(|m| m.title.clone())` |
| `PullRequest` gains `assignees: Vec<String>` | ✓ PASS | Present and mapped from octocrab `pr.assignees.unwrap_or_default()` |
| `SecurityAlert` gains `cve_id: Option<String>` | ✓ PASS | Present, mapped from `advisory.cve_id` |
| `SecurityAlert` gains `cvss_score: Option<f64>` | ✓ PASS | Present, mapped from `advisory.cvss.score` |
| `SecurityAlert` gains `cwes: Vec<String>` | ✓ PASS | Present, mapped by extracting `cwe_id` from `advisory.cwes` |
| `SecurityAlert` gains `dismissed_reason: Option<String>` | ✓ PASS | Present, mapped from top-level `a.dismissed_reason` |
| `SecurityAlert` gains `dismissed_comment: Option<String>` | ✓ PASS | Present, mapped from top-level `a.dismissed_comment` |
| New `PullDetail` struct | ✓ PASS | Contains `number`, `additions`, `deletions`, `changed_files`, `mergeable`, `mergeable_state` |
| `RawCvss`, `RawCwe` structs added | ✓ PASS | Correctly defined in `security.rs` |
| Code scanning alerts exclude new fields | ✓ PASS | Code scanning alerts use `None` / default for dependabot-only fields |

### 2.2 New Tauri Command — `get_pull_detail`

| Requirement | Status | Notes |
|---|---|---|
| `detail.rs` created | ✓ PASS | Located at `src-tauri/src/github/detail.rs` |
| `pub mod detail` in `github/mod.rs` | ✓ PASS | Declared correctly |
| Command `get_pull_detail` registered in non-mock handler | ✓ PASS | In `tauri::generate_handler![..., get_pull_detail, ...]` |
| `mock::get_pull_detail` registered in dev-mock handler | ✓ PASS | In mock `generate_handler![..., mock::get_pull_detail, ...]` |
| Mock returns realistic per-PR data | ✓ PASS | Match arms for PRs 101, 115, 122 with distinct stats |

### 2.3 Frontend Features

| Requirement | Status | Notes |
|---|---|---|
| `expandedRow` state variable | ✓ PASS | Declared at module level alongside other state |
| Click-to-expand / click-to-collapse | ✓ PASS | `toggleDetailRow()` toggles `expanded` class |
| One-at-a-time across all tabs | ✓ PASS | `collapseAllRows()` called before every expand |
| Detail rows inline below each data row | ✓ PASS | Rendered as paired `<tr class="detail-row">` in each render function |
| Close button (×) in each panel | ✓ PASS | `onclick="collapseAllRows()"` button in each detail header |
| `expandedRow = null` on data refresh | ✓ PASS | Set in `refreshData()` after render calls |
| Lazy PR stats via `get_pull_detail` | ✓ PASS | Fetched on first expand; `.detail-pr-stats-loading` sentinel guards double-fetch |
| Mini-spinner in PR stats slot | ✓ PASS | `.spinner-small` CSS class and element |
| Markdown rendering with DOMPurify | ✓ PASS | `renderMarkdown()` calls `marked.parse()` then `DOMPurify.sanitize()` |
| `renderMarkdown` used for issue/PR body | ✓ PASS | Calls in `buildIssueDetail()` and `buildPullDetail()` |
| Security alert description uses `esc()` (not markdown) | ✓ PASS | `esc(alert.description)` in `buildAlertDetail()` |

### 2.4 Issue Detail Panel Fields

| Field | Present | Escaped |
|---|---|---|
| Issue number | ✓ | N/A (numeric) |
| State badge | ✓ | Via `stateBadge()` (static strings) |
| Body (markdown) | ✓ | Via `renderMarkdown()` → DOMPurify |
| Author | ✓ | `esc(issue.author)` |
| Assignees | ✓ | `.map(esc)` |
| Labels | ✓ | Via `labelBadges()` using `esc` in existing table context |
| Milestone | ✓ | `esc(issue.milestone)` |
| Comments | ✓ | Numeric, no escape needed |
| Created At | ✓ | Via `shortDate()` |
| Updated At | ✓ | Via `shortDate()` |
| Closed At (conditional) | ✓ | Via `shortDate()` |
| Open on GitHub link | ✓ | Anchor present — **see Security §7.3** |

### 2.5 Pull Request Detail Panel Fields

| Field | Present | Escaped |
|---|---|---|
| PR number | ✓ | N/A (numeric) |
| State badge | ✓ | Via `stateBadge()` |
| Draft badge | ✓ | Static string |
| Body (markdown) | ✓ | Via `renderMarkdown()` → DOMPurify |
| Author | ✓ | `esc(pull.author)` |
| Assignees | ✓ | `.map(esc)` |
| Reviewers | ✓ | `.map(esc)` |
| Labels | ✓ | Via `labelBadges()` |
| Head → Base branch | ✓ | `esc(pull.head_branch)` → `esc(pull.base_branch)` |
| Created At | ✓ | `shortDate()` |
| Updated At | ✓ | `shortDate()` |
| Merged At (conditional) | ✓ | `shortDate()` |
| Closed At (conditional, non-merged) | ✓ | `shortDate()` |
| Diff stats: additions, deletions, files | ✓ | Numeric, lazy-loaded |
| Mergeable status | ✓ | Boolean, renders ✓/✗ |
| Open on GitHub link | ✓ | **see Security §7.3** |

### 2.6 Security Alert Detail Panel Fields

| Field | Present | Escaped |
|---|---|---|
| Alert ID | ✓ | N/A (numeric) |
| Type badge | ✓ | Static + `esc(tool_name)` |
| Severity badge | ✓ | `esc(alert.severity)` |
| Description | ✓ | `esc(alert.description)` |
| CVE ID | ✓ | `esc(alert.cve_id)` |
| CVSS Score | ✓ | Numeric `.toFixed(1)` |
| CWEs | ✓ | `.map(esc)` |
| Package | ✓ | `esc(alert.package_name \|\| "—")` |
| Vulnerable Range | ✓ | `esc(...)` |
| Patched Version | ✓ | `esc(...)` |
| State | ✓ | `esc(alert.state)` |
| Created At | ✓ | `shortDate()` |
| Location (conditional) | ✓ | `esc(alert.location_path)` |
| Dismissed Reason (conditional) | ✓ | `esc(alert.dismissed_reason)` |
| Dismissed Comment (conditional) | ✓ | `esc(alert.dismissed_comment)` |
| Open on GitHub link | ✓ | **see Security §7.3** |

### 2.7 Spec Deviations

| Deviation | Severity | Description |
|---|---|---|
| Vendor directory is `vendor/` not `lib/` | Minor | Spec (§6.1) says `<script src="lib/marked.min.js">` but implementation uses `vendor/`. Functionally identical — low concern. |
| `collapseAllRows()` also removes `.row-expanded` from data rows | Enhancement | Spec's version only removes `.expanded` from detail rows. Implementation additionally removes `.row-expanded` from data rows, providing visual consistency. Correct enhancement. |
| `toggleDetailRow` adds `.row-expanded` to sibling data row | Enhancement | Spec's version does not mention `row-expanded` on the data row. The implementation highlights the parent row — a UX improvement. |

Overall spec compliance is **excellent**.

---

## 3. Code Quality — Rust

### 3.1 Struct Derivations

All new structs have correct derives:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PullDetail { ... }
```

- `Issue`, `PullRequest`, `SecurityAlert`: all retain `Debug, Clone, Serialize, Deserialize` ✓
- `PullDetail`: all four derives present ✓
- `RawCvss`, `RawCwe`, `RawAdvisory`, `RawDependabotAlert`: use `Debug, Deserialize` (no Serialize needed for internal-only types) ✓

### 3.2 Error Handling

No `unwrap()` calls on user-facing paths:

| Pattern | Usage |
|---|---|
| `.context()` from `anyhow` | API call failures in `detail.rs`, `issues.rs` |
| `.map_err(\|e\| e.to_string())` | In all Tauri command wrappers |
| `.ok_or("Not authenticated")` | Auth guard in all commands |
| `.unwrap_or(0)` / `.unwrap_or_default()` | Only on optional numeric fields from octocrab |
| `.unwrap_or_else(\|_\| chrono::Utc::now())` | Fallback on date parse failure in security.rs |

The only `unwrap()` present is in `mock/mod.rs`:
```rust
fn dt(rfc3339: &str) -> DateTime<chrono::Utc> {
    DateTime::parse_from_rfc3339(rfc3339).expect("hard-coded datetime must be valid RFC-3339")...
}
```
This is acceptable — `expect()` on hard-coded compile-time constants in the mock module is a deliberate, documented panic guard.

### 3.3 Borrow Checker / Lifetime Handling

In `security.rs`, the `advisory` variable is correctly assigned before the struct literal mapping:
```rust
let advisory = a.security_advisory.as_ref();
```
This is consistent with the spec's note about borrow ordering in §4.3.

### 3.4 `get_pull_detail` Command Signature

`pull_number: u64` is correctly typed as `u64` — consistent with `PullDetail.number`, `pulls[idx].number`, and the JS `invoke("get_pull_detail", { ..., pullNumber: pull.number })`.

### 3.5 Code Scanning Alert Fields

The code scanning path in `fetch_code_scanning_alerts()` correctly sets new SecurityAlert fields to safe defaults:
- `cve_id: None`
- `cvss_score: None`  
- `cwes: vec![]`  
- `dismissed_reason: None`  
- `dismissed_comment: None`

No leakage or incorrect defaults.

### 3.6 Pulls Mapping

`assignees` correctly mapped in `pulls.rs`:
```rust
assignees: pr
    .assignees
    .unwrap_or_default()
    .iter()
    .map(|a| a.login.clone())
    .collect(),
```
Consistent with the spec's `fetch_pulls()` update in §4.2.

---

## 4. Code Quality — JavaScript

### 4.1 No ES Module Syntax

Confirmed: no `import`/`export` statements anywhere in `main.js`. All code uses plain vanilla JS with the global Tauri bridge.

### 4.2 XSS Prevention — `esc()` Usage

The `esc()` function creates a temporary `<span>`, sets `textContent`, and reads back `innerHTML` — the standard browser-based HTML escaping pattern. Every user-controlled string field is passed through `esc()` before being embedded in template literals:

- `esc(i.title)`, `esc(i.author)`, `esc(i.state)` in data rows ✓
- `esc(issue.author)`, `esc(issue.milestone)`, `.map(esc)` for arrays in detail panels ✓
- `esc(alert.severity)`, `esc(alert.summary)`, `esc(alert.description)` ✓
- Error messages: `esc(issuesError)`, `esc(String(e))` ✓

**Exception found (see §7.3 Security):** `html_url` values are embedded in `href` attributes without `esc()`.

### 4.3 DOMPurify Pipeline

The markdown rendering pipeline is correctly chained:
```javascript
function renderMarkdown(text) {
    if (!text) return '<em class="detail-no-body">No description provided.</em>';
    const rawHtml = marked.parse(text, { breaks: true, gfm: true });
    return DOMPurify.sanitize(rawHtml);
}
```
- `marked.parse()` converts Markdown → HTML (potentially with embedded HTML from field content)
- `DOMPurify.sanitize()` cleans the output before it is embedded in `innerHTML`

### 4.4 Lazy PR Stats Guard

The sentinel check prevents redundant API calls on re-expansion:
```javascript
if (statsEl && statsEl.querySelector(".detail-pr-stats-loading")) {
  // Only fetches once — loading spinner is replaced with results
}
```
After the first successful fetch, the spinner element is gone and subsequent expansions reuse the cached content. Correct.

### 4.5 Error Handling in `toggleDetailRow`

The PR stats fetch error path correctly uses `esc()`:
```javascript
statsEl.innerHTML = `<span class="detail-stats-error">Could not load diff stats: ${esc(String(e))}</span>`;
```

### 4.6 `expandedRow` Variable

Properly reset in:
- `collapseAllRows()` → `expandedRow = null` ✓
- `refreshData()` → `expandedRow = null` ✓

### 4.7 Row Expansion Enhancement

The implementation correctly uses `detailRow.previousElementSibling` to find the parent data row, which is safe because the DOM structure guarantees the data row always precedes its companion detail row.

---

## 5. Code Quality — CSS

### 5.1 Animation

```css
.detail-body {
    max-height: 0;
    overflow: hidden;
    opacity: 0;
    transition: max-height 0.32s ease, opacity 0.25s ease, padding 0.32s ease;
}
.detail-row.expanded .detail-body {
    max-height: 700px;
    opacity: 1;
    padding: 1.25rem 0.8rem;
}
```

The spec specifies 300ms. Implementation uses 320ms for `max-height`/`padding` and 250ms for `opacity`. The difference is imperceptible to users. Both are smooth GPU-composited transitions. No layout thrash risk.

### 5.2 Max-Height Ceiling

The `max-height: 700px` ceiling on expanded detail rows is constrained. The inner `.detail-body-text` is capped at `max-height: 300px; overflow-y: auto`, and the metadata grid uses compact layout. For very long PR descriptions or many metadata fields with large dismissed_comment text, content could be clipped. However, the inner scrollable text area should prevent any critical content from being hidden.

### 5.3 No Layout Breaking Issues

- `overflow: hidden` on `.detail-body` prevents content from affecting table layout while collapsed ✓
- `.detail-row td { padding: 0 }` ensures zero-height rows are truly invisible when collapsed ✓
- `user-select: none` on `.clickable-row` prevents text selection on click ✓
- `flex-shrink: 0` on `.detail-close-btn` prevents the close button from shrinking in flex containers ✓

### 5.4 Specificity and Class Naming

All new classes are prefixed with `detail-` or use existing conventions (`badge-draft`, `stat-additions`, etc.). No specificity conflicts with existing rules detected.

---

## 6. Code Quality — HTML

### 6.1 Vendor Script Load Order

```html
<script src="vendor/marked.min.js"></script>
<script src="vendor/purify.min.js"></script>
<script src="main.js"></script>
```

Vendor scripts are loaded **before** `main.js` — correct. The `marked` and `DOMPurify` globals will be available when `main.js` executes.

### 6.2 Table Structure

No changes to `<thead>` structures. The `<tbody>` elements remain empty — populated by JavaScript. `colspan="6"` for issues/pulls and `colspan="7"` for alerts matches the actual column counts. Correct.

### 6.3 Target Blank Usage

All "Open on GitHub ↗" links use `target="_blank"`. These should also have `rel="noopener noreferrer"` to prevent reverse tabnapping (see §11 Recommended Improvements).

---

## 7. Security Review

### 7.1 purify.min.js Analysis

**File size:** 2,723 bytes  
**Real DOMPurify (v3.x minified):** ~35,000 bytes

The `vendor/purify.min.js` is a **custom minimal implementation**, NOT the official DOMPurify library. Key characteristics:

**Protections present:**
- Removes dangerous tags: `script, style, iframe, object, embed, form, input, button, select, textarea, meta, link, base, frame, frameset, applet, noscript, noframes, xmp, plaintext, svg, math`
- Removes all `on*` event handler attributes
- Removes `javascript:`, `data:`, `vbscript:` URL schemes from `href`, `src`, `action`
- Removes `formaction`, `xlink:href`, `srcdoc` attributes
- Removes HTML comments
- Uses `document.implementation.createHTMLDocument('')` for browser-based parsing (prevents regex bypass)

**Protections absent (vs. real DOMPurify):**
- **Allowlist approach**: The real DOMPurify only permits explicitly safe tags/attributes. The custom version uses a blocklist — any new HTML feature not in the blocklist passes through.
- **Mutation XSS (mXSS) protection**: Real DOMPurify re-parses output to detect DOM mutation during serialization.
- **DOM clobbering protection**: No `id`/`name` attribute restrictions.
- **CSS injection via `style` attributes**: `style` elements are blocked (tag-level), but `style` attributes on allowed elements are NOT sanitized. CSS `expression()` attacks are IE-only and thus not a real risk in Chromium-based Tauri, but `background: url('data:...')` in style attributes could leak.
- **Customization hooks**: No `FORCE_BODY`, `ADD_TAGS`, `FORBID_ATTR` configuration.
- **Formal security audit or CVE tracking**.

### 7.2 marked.min.js Analysis

**File size:** 5,386 bytes  
**Real marked.js (v12.x minified):** ~35,000+ bytes

The `vendor/marked.min.js` is also a **custom minimal implementation**. Key concerns:

- The `parseInline()` function does NOT escape captured `$1` content in bold/italic/heading patterns:
  ```javascript
  text = text.replace(/\*\*([^*\n]+)\*\*/g, '<strong>$1</strong>');
  ```
  Raw HTML in bold/italic text passes through to the output. This is safe **only because DOMPurify cleans it afterward.**

- Link handling correctly strips dangerous URL schemes from `href`:
  ```javascript
  var safe = h.replace(/^\s*(javascript|data|vbscript)\s*:/i, '');
  return '<a href="' + escHtml(safe.trim()) + '">' + escHtml(t) + '</a>';
  ```
  ✓ Safe

- Fenced code blocks correctly call `escHtml()` on content ✓

- Does not handle: tables (GFM), strikethrough, task lists, or most GFM extensions. This is a functional limitation, not a security one.

### 7.3 `html_url` in href Attributes — Medium Risk

In all three detail panel builders, `html_url` is embedded in anchor `href` without `esc()`:

```javascript
// In buildIssueDetail():
<a href="${issue.html_url}" target="_blank" class="detail-open-link">Open on GitHub ↗</a>

// In buildPullDetail():
<a href="${pull.html_url}" target="_blank" class="detail-open-link">Open on GitHub ↗</a>

// In buildAlertDetail():
<a href="${alert.html_url}" target="_blank" class="detail-open-link">Open on GitHub ↗</a>
```

**Risk assessment:** `html_url` comes from GitHub's authenticated REST API. GitHub always returns valid HTTPS URLs (e.g., `https://github.com/owner/repo/issues/42`). The practical injection risk is near-zero. However, the absence of `esc()` means if GitHub's response were tampered (MITM, or a compromised token with write access to a specially crafted repo), the URL could break out of the `href` attribute and inject HTML.

**Defense-in-depth:** Should use `esc(issue.html_url)` in all three locations.

### 7.4 `onclick` in Template Literals

```html
<tr ... onclick="toggleDetailRow('issues', ${idx})">
```

The `idx` value is a JavaScript integer, never a string from external data. No XSS risk here.

### 7.5 Summary of Security Posture

| Check | Status |
|---|---|
| All `innerHTML` assignments of user data use `esc()` | ✓ Mostly yes — see §7.3 for `html_url` exception |
| Markdown rendered through DOMPurify | ✓ Yes |
| Script tag injection blocked | ✓ Yes (by custom purify) |
| `on*` event handler injection blocked | ✓ Yes (by custom purify) |
| `javascript:` URL injection blocked | ✓ Yes (by custom purify + marked link handling) |
| Using official, security-reviewed DOMPurify library | ✗ **NO — custom stub** |
| Using official, security-reviewed marked.js library | ✗ **NO — custom stub** |
| `target="_blank"` links have `rel="noopener noreferrer"` | ✗ Missing |
| `html_url` escaped in href attributes | ✗ Missing — see §7.3 |

---

## 8. Build Validation

### 8.1 `cargo build`

**Command:** `cargo build` (from `src-tauri/`)  
**Result:** ✅ PASS  
**Exit Code:** 0  
**Output:**
```
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.47s
```
No compiler errors. No new warnings introduced. The code compiled cleanly on first run (binary was already current from a prior build; confirmed rebuilt artifact is consistent).

### 8.2 `cargo clippy -- -D warnings`

**Command:** `cargo clippy -- -D warnings` (from `src-tauri/`)  
**Result:** ✅ PASS  
**Exit Code:** 0  
**Output:**
```
Checking github-export v0.1.0 (C:\Projects\github-export\src-tauri)
Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.28s
```
No lint warnings. All new code passes Clippy's idiom and correctness checks with `-D warnings` (warnings treated as errors).

### 8.3 `cargo test`

**Command:** `cargo test` (from `src-tauri/`)  
**Result:** ✅ PASS  
**Exit Code:** 0  
**Output:**
```
Finished `test` profile [unoptimized + debuginfo] target(s) in 0.51s
Running unittests src\main.rs (target\debug\deps\github_export-59971f914bfbd6d1.exe)

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```
No tests exist in the project. The test suite passes trivially. **Zero test coverage is noted as a recommended improvement** (see §11.3).

### 8.4 Build Summary

| Step | Command | Exit Code | Result |
|------|---------|-----------|--------|
| Compile | `cargo build` | 0 | ✅ PASS |
| Lint | `cargo clippy -- -D warnings` | 0 | ✅ PASS |
| Tests | `cargo test` | 0 | ✅ PASS |

---

## 9. Score Table

| Category | Score | Grade | Notes |
|----------|-------|-------|-------|
| Specification Compliance | 96% | A | Minor: `vendor/` vs `lib/` directory name deviation |
| Best Practices | 88% | B+ | Good Rust patterns; `esc()` used consistently; no ES modules. Deducted for missing `rel="noopener"` |
| Functionality | 95% | A | All features fully implemented; lazy fetch guard correct; one-at-a-time enforcement correct |
| Code Quality | 88% | B+ | Clean Rust; good JS organization; `html_url` href escaping gap |
| Security | 55% | D+ | **CRITICAL**: Custom minimal stubs replacing DOMPurify and marked.js; `html_url` unescaped in href |
| Performance | 92% | A- | CSS GPU transitions; lazy PR stats; no excess API calls. Minor concern: `max-height:700px` cap |
| Consistency | 90% | A- | Visual style consistent with existing UI; CSS naming consistent |
| Build Success | 100% | A+ | `cargo build`, `clippy`, `cargo test` all exit 0, zero warnings |

**Overall Grade: B+ (88%)**

> ⚠️ Despite the high overall grade, the CRITICAL security issue (custom security libraries) mandates **NEEDS_REFINEMENT**.

---

## 10. Critical Issues

### CRITICAL-1: Custom Stub Libraries Instead of Official DOMPurify and marked.js

**Severity:** Critical  
**Affected files:** `src/vendor/purify.min.js`, `src/vendor/marked.min.js`

**Issue:**  
Both vendor files are custom minimal implementations (2.7KB and 5.4KB respectively) that mimic the DOMPurify and marked.js APIs but are not the actual libraries. The official DOMPurify is a security-critical, community-maintained library with a formal test suite, CVE tracking, and an allowlist-based sanitization model. The custom replacement uses a blocklist model, which is fundamentally less secure.

**Specific weaknesses of the custom purify.min.js:**
- Blocklist approach — any future HTML element or attribute not yet in the list bypasses sanitization
- No mutation XSS (mXSS) protection
- No DOM clobbering protection (`id`/`name` attributes unrestricted)
- `style` attribute content not sanitized (though CSS expressions are IE-only)
- Carries a fake version string `"3.0.0-local"` that could mislead dependency scanners

**Why it matters in this context:**  
GitHub issue/PR bodies can contain arbitrary Markdown authored by any GitHub user. A maliciously crafted body with novel HTML patterns could bypass the blocklist and execute in the Tauri WebView context.

**Fix:**  
Replace both files with the actual official minified libraries:
- DOMPurify: https://github.com/cure53/DOMPurify/releases — download `purify.min.js` (~35KB)
- marked.js: https://github.com/markedjs/marked/releases — download `marked.min.js` (~35KB)

Place them in `src/vendor/` and update `src/index.html` if paths change. No code changes to `main.js` are needed — the APIs are identical.

---

## 11. Recommended Improvements

### REC-1: Escape `html_url` in href Attributes

**Severity:** Medium  
**Affected files:** `src/main.js` — `buildIssueDetail()`, `buildPullDetail()`, `buildAlertDetail()`

**Issue:**  
`html_url` is embedded in `href` attributes without `esc()`. While GitHub's API reliably returns valid HTTPS URLs, defense-in-depth requires all external data to be HTML-escaped before insertion into the DOM.

**Fix — apply `esc()` to all three href usages:**
```javascript
// In buildIssueDetail():
<a href="${esc(issue.html_url)}" target="_blank" class="detail-open-link">Open on GitHub ↗</a>

// In buildPullDetail():
<a href="${esc(pull.html_url)}" target="_blank" class="detail-open-link">Open on GitHub ↗</a>

// In buildAlertDetail():
<a href="${esc(alert.html_url)}" target="_blank" class="detail-open-link">Open on GitHub ↗</a>
```

### REC-2: Add `rel="noopener noreferrer"` to target="_blank" Links

**Severity:** Low  
**Affected files:** `src/main.js` — all three detail builders

**Issue:**  
Links with `target="_blank"` that open untrusted URLs should include `rel="noopener noreferrer"` to prevent reverse tabnapping attacks.

**Fix:**
```javascript
<a href="${esc(issue.html_url)}" target="_blank" rel="noopener noreferrer" class="detail-open-link">Open on GitHub ↗</a>
```

Apply to all three detail panel "Open on GitHub" links.

### REC-3: Add Rust Unit Tests for New Models

**Severity:** Low  
**Notes:**  
The project has zero Rust tests. Consider adding at minimum:
- Serialization round-trip tests for `PullDetail` and the extended `SecurityAlert`
- A test verifying that `fetch_pull_detail` correctly populates `PullDetail` fields from mock data

This would validate that `serde` field names match the JavaScript-side camelCase/snake_case expectations.

### REC-4: Add `max-height` Guard for Very Long Detail Content

**Severity:** Low  
**Notes:**  
The `.detail-row.expanded .detail-body { max-height: 700px }` limit could clip content for PRs with many meta fields and a long `dismissed_comment`. Consider increasing to `1200px` or using `max-height: none` with the inner `.detail-body-text` scroll providing the containment.

---

## 12. Final Verdict

**Build Result:** ✅ PASS — all three build steps exit 0, zero warnings  

**Verdict:** ⚠️ **NEEDS_REFINEMENT**

The implementation is architecturally sound and closely follows the specification. Rust code quality is high, and the JS rendering pipeline is well-constructed with consistent XSS protection via `esc()`. The build, lint, and test commands all pass cleanly.

However, one **CRITICAL** issue prevents approval:

> **CRITICAL-1**: The vendor DOMPurify and marked.js files are custom minimal stubs, not the official security-reviewed libraries. The custom `purify.min.js` (2.7KB) uses a blocklist approach that lacks the robust allowlist-based protection of the real DOMPurify (~35KB). This must be corrected before this feature can ship.

Upon addressing CRITICAL-1 and RECOMMENDED REC-1 (href escaping), the implementation should qualify for **APPROVED** status.

---

*Review produced by QA Subagent — 2026-03-05*
