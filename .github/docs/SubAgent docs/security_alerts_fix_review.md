# Security Alerts Fix — Review

**Feature:** Code Scanning Alerts + Incorrect Help Text Fix  
**Date:** 2026-03-05  
**Reviewer:** Review Subagent  
**Spec:** `.github/docs/SubAgent docs/security_alerts_fix_spec.md`

---

## 1. Validation Findings

### 1.1 `SecurityAlert` Model — New Fields (`models/mod.rs`)

| Spec Field | Spec Type | Actual Field | Actual Type | Match? |
|---|---|---|---|---|
| `alert_kind` | `String` | `alert_type` | `String` | ❌ Name mismatch |
| `tool_name` | `Option<String>` | `tool_name` | `Option<String>` | ✅ |
| `location_path` | `Option<String>` | `rule_id` | `Option<String>` | ❌ Name + purpose mismatch |

**Finding:** Two of the three new fields deviate from the spec. The spec defined `alert_kind` and `location_path`; the implementation chose `alert_type` and `rule_id`. The implementation is internally consistent (all three files — `security.rs`, `mock/mod.rs`, and `main.js` — agree on `alert_type` / `rule_id`), so the feature works end-to-end, but the serialisation contract diverges from the spec.

---

### 1.2 Code Scanning Alerts Fetch (`security.rs`)

**Code scanning endpoint called:** ✅  
`GET /repos/{owner}/{repo}/code-scanning/alerts?per_page=100[&state=open]`

**Struct for deserialisation defined:** ✅ (`RawCodeScanningAlert`, `CodeScanningAlertRule`, `CodeScanningAlertTool`)

**404/403 graceful handling:** ✅  
The `fetch_code_scanning_alerts` function inspects the error message for `"404"` or `"403"` and returns an empty `Vec` rather than propagating the error.

**All code scanning errors non-fatal (as spec requires):** ❌  
The spec requires ALL code scanning fetch failures to be non-fatal (the function should always return `Ok`). The implementation only suppresses 404 and 403 — any other error code (e.g., 422, 500) is re-raised and will surface to the user as an alerts fetch failure even though Dependabot results were retrieved successfully.

**`tokio::join!` for concurrent fetching:** ❌  
The spec required both API calls to run concurrently via `tokio::join!`. The implementation calls `fetch_code_scanning_alerts` sequentially after the Dependabot call resolves, adding latency when both calls must complete.

**`most_recent_instance.location.path` captured:** ❌  
The `RawCodeScanningAlert` struct has no `most_recent_instance` field. The spec required deserialising `most_recent_instance → location → path` into `location_path`. Instead the implementation captures `rule.id` into `rule_id`. Code scanning alerts will always show "—" for the file-path column.

**Severity normalisation (error→high, warning→medium, note→low):** ❌  
The spec explicitly required mapping API severity strings to match Dependabot conventions. The implementation passes the raw API value through unchanged. Code scanning alerts may arrive with `"error"`, `"warning"`, or `"note"` — none of which match the CSS classes `severity-critical`, `severity-high`, `severity-medium`, `severity-low`. These rows will fall through to `severity-low` styling for all three levels, producing incorrect visual indicators.

**Dependabot error message path corrected:** ❌  
The `anyhow!` error string in `fetch_alerts` still reads:  
> `(Settings → Security → Dependabot alerts).`  
The spec required this to be updated to:  
> `(Settings → Security section → Advanced Security → Dependabot Section → Dependabot Alerts → Enable).`  
This is the third occurrence documented in the spec (Table §3, row 3).

**Sort by id descending:** ❌  
The spec required the combined list to be sorted by `id` descending (newest first). No sort is applied.

---

### 1.3 Mock Data — New Fields (`mock/mod.rs`)

**Existing Dependabot mock alerts updated with new fields:** ✅  
Both mock Dependabot alerts include `alert_type: "dependabot"`, `tool_name: None`, `rule_id: None` — matching the actual model fields.

**At least one code scanning mock alert added:** ❌  
The spec explicitly required at least one code scanning mock alert to demonstrate the UI Type column distinction. No code scanning alert exists in the mock. Dev mode cannot demonstrate the feature operating correctly.

---

### 1.4 Help Text — "Code security and analysis" (`main.js`)

**Occurrences of "Code security and analysis" in `main.js`:** 0 ✅  
Both incorrect instances were successfully replaced with `Advanced Security`.

**Error-path guidance (disabled branch):**  
> Go to … **Settings** → **Security** section → **Advanced Security** → **Dependabot Alerts** → click **Enable**. ✅

**Empty-state tips:**  
> GitHub repository → **Settings** → **Security** section → **Advanced Security** → **Dependabot Alerts** → **Enable** ✅

---

### 1.5 "Type" Column in `renderAlerts()` (`main.js`)

**Type column rendered:** ✅  
The `typeLabel` variable distinguishes `alert_type === "code_scanning"` from `"dependabot"` and respects `tool_name`.

**`colspan="7"` in error and empty rows:** ✅

**Field references consistent with model:** ✅ (`a.alert_type`, `a.tool_name`, `a.package_name`)

**Location display for code scanning:** ❌  
Since `location_path` was never added to the model, code scanning alerts will always render "—" in the Package column rather than the file path. The rendering logic uses `a.package_name`, which is `None` for all code scanning alerts.

---

### 1.6 "Type" `<th>` in `index.html`

**`<th>Type</th>` present:** ✅  
The alerts table header now reads: `ID | Type | Severity | Summary | Package | Vulnerable | Patched`

**Spec-specified column labels (`Package / Location`, `Tool / Vuln Range`):** ❌  
The implementation kept the original `Package` and `Vulnerable` labels rather than the dual-purpose labels from the spec. These labels are misleading for code scanning rows.

---

## 2. Build Validation

### 2.1 `cargo build`

```
Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.26s
EXIT_CODE: 0
```
**Result: PASS ✅**

### 2.2 `cargo clippy -- -D warnings`

```
Checking github-export v0.1.0 (C:\Projects\github-export\src-tauri)
Finished `dev` profile [unoptimized + debuginfo] target(s) in 25.24s
EXIT_CODE: 0
```
**Result: PASS ✅** — Zero warnings.

### 2.3 `cargo test`

```
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored
EXIT_CODE: 0
```
**Result: PASS ✅**

---

## 3. Security Review

| Check | Status | Notes |
|---|---|---|
| GitHub tokens handled server-side only | ✅ | Tokens never appear in JS; passed through Tauri state |
| User-controlled data escaped before innerHTML | ✅ | `esc()` uses `textContent`/`innerHTML` DOM trick — XSS-safe |
| `tool_name` escaped | ✅ | `esc(a.tool_name)` used in `typeLabel` |
| `a.html_url` in `href` attribute unescaped | ⚠️ | Pre-existing issue; could allow `javascript:` URIs if API returns malicious data. Not introduced by this change but worth noting. |
| No SQL / command injection surface | ✅ | No database, no shell invocations |

**Security verdict: No new vulnerabilities introduced by this change.**

---

## 4. Score Table

| Category | Score | Grade |
|---|---|---|
| Specification Compliance | 52% | F |
| Best Practices | 78% | C+ |
| Functionality | 62% | D |
| Code Quality | 80% | B |
| Security | 85% | B |
| Performance | 68% | D+ |
| Consistency | 82% | B |
| Build Success | 100% | A+ |

**Overall Grade: C (76%)**

---

## 5. Critical Issues

### CRITICAL-1 — `location_path` not captured (code scanning file paths broken)
**File:** `src-tauri/src/github/security.rs` and `src-tauri/src/models/mod.rs`  
The spec required `most_recent_instance.location.path` to be deserialised and stored in `location_path` on `SecurityAlert`. This field was replaced by `rule_id` (which captures `rule.id` instead). Code scanning alerts display "—" in the Package/Location column rather than the file path where the finding was detected. This is a meaningful regression against the spec's functional goal.

### CRITICAL-2 — Severity normalisation missing (code scanning rows styled incorrectly)
**File:** `src-tauri/src/github/security.rs`  
The Code Scanning API returns severities `"error"`, `"warning"`, `"note"`, and `"none"`. The CSS classes only cover `severity-critical`, `severity-high`, `severity-medium`, `severity-low`. Without normalisation, all code scanning alerts fall through to `severity-low` styling regardless of actual severity. For example, a `"error"` finding looks identical to a `"none"` finding in the UI.  
**Required mapping:**  
- `"error"` → `"high"` → `severity-high`  
- `"warning"` → `"medium"` → `severity-medium`  
- `"note"` → `"low"` → `severity-low`

### CRITICAL-3 — No code scanning mock alert in `mock/mod.rs`
**File:** `src-tauri/src/mock/mod.rs`  
The spec explicitly required at least one code scanning mock alert to demonstrate the Type column distinction in dev mode. The mock only contains Dependabot alerts. Developers running `npm run dev:mock` cannot verify the Type column or code-scanning-specific rendering paths without connecting to a real GitHub repository.

---

## 6. Recommended Improvements

### REC-1 — Rename `alert_type` → `alert_kind` and `rule_id` → `location_path` (spec alignment)
All three files (`models/mod.rs`, `security.rs`, `mock/mod.rs`) and `main.js` use `alert_type` and `rule_id`. Renaming to match the spec (`alert_kind`, `location_path`) improves long-term consistency with the project specification. This rename is a breaking serialisation change so should be done atomically across all four files.

### REC-2 — Fix abbreviated error message path in `security.rs`
The `anyhow!` string in `fetch_alerts` still contains `(Settings → Security → Dependabot alerts)`. Update to the corrected path per the spec (§3, row 3): `Settings → Security section → Advanced Security → Dependabot Section → Dependabot Alerts → Enable`.

### REC-3 — Make all code scanning errors non-fatal
Currently only HTTP 404 and 403 are suppressed. The spec intends for all code scanning failures to be non-fatal since the feature may simply not be enabled. Change the `Err(e)` branch in `fetch_code_scanning_alerts` to always return `Ok(vec![])` and log to stderr.

### REC-4 — Use `tokio::join!` for concurrent fetching
The spec called for concurrent fetching via `tokio::join!`. The sequential implementation adds unnecessary latency when the user's repository has both Dependabot and code scanning enabled. Convert to concurrent execution.

### REC-5 — Sort combined results by id descending
The spec required `combined.sort_by(|a, b| b.id.cmp(&a.id))` after merging the two alert lists. Without sorting, the order is Dependabot results followed by code scanning results, which is confusing.

### REC-6 — Update column headers in `index.html`
`<th>Package</th>` → `<th>Package / Location</th>`  
`<th>Vulnerable</th>` → `<th>Tool / Vuln Range</th>`  
These labels better communicate the dual-purpose nature of the columns for mixed Dependabot + code scanning rows, as specified in §5.5.

---

## 7. Verdict

**Build:** PASS ✅ (all three: `cargo build`, `cargo clippy -- -D warnings`, `cargo test`)

**Overall:** **NEEDS_REFINEMENT**

Three critical issues must be resolved before this can be considered complete:
- CRITICAL-1: `location_path` not captured (file path display broken)
- CRITICAL-2: Severity normalisation missing (incorrect CSS styling for code scanning rows)
- CRITICAL-3: No code scanning mock alert (dev mode cannot demonstrate the feature)

The help text fix (the primary secondary fix) is correctly implemented. The basic code scanning fetch infrastructure is in place. Recommended improvements (REC-1 through REC-6) should also be addressed to bring the implementation fully into spec compliance.
