# Security Alerts Fix — Specification

**Feature:** Code Scanning Alerts + Incorrect Help Text Fix  
**Date:** 2026-03-05  
**Status:** Draft

---

## 1. Current State Analysis

### 1.1 Backend — `src-tauri/src/github/security.rs`

The only API call currently implemented is:

```
GET /repos/{owner}/{repo}/dependabot/alerts?per_page=100[&state=open]
```

This exclusively fetches **Dependabot** dependency-vulnerability alerts. There is **no implementation** for the GitHub Code Scanning Alerts API endpoint:

```
GET /repos/{owner}/{repo}/code-scanning/alerts
```

The file defines these private deserialisation structs:
- `RawDependabotAlert` — maps number, state, html_url, created_at
- `RawAdvisory` — maps summary, description, severity
- `RawVulnerability` / `RawPackage` / `RawPatchedVersion` — package-specific fields

These structs are Dependabot-specific and have no overlap with the Code Scanning API response shape.

### 1.2 Backend — `src-tauri/src/models/mod.rs`

`SecurityAlert` struct fields:

| Field | From Dependabot? | From Code Scanning? |
|-------|:---:|:---:|
| `id` (u64) | ✓ | ✓ mapped from `number` |
| `severity` (String) | ✓ via advisory | ✓ via `rule.severity` |
| `summary` (String) | ✓ via advisory | ✓ via `rule.description` |
| `description` (String) | ✓ via advisory | ✗ (code scanning uses `rule.full_description`) |
| `package_name` (Option<String>) | ✓ | ✗ (not applicable) |
| `vulnerable_version_range` (Option<String>) | ✓ | ✗ (not applicable) |
| `patched_version` (Option<String>) | ✓ | ✗ (not applicable) |
| `state` (String) | ✓ | ✓ |
| `html_url` (String) | ✓ | ✓ |
| `created_at` (DateTime<Utc>) | ✓ | ✓ |

Missing fields for code scanning:
- `tool_name` — the scanning tool (e.g., "CodeQL", "ESLint")
- `location_path` — file path of the finding (`most_recent_instance.location.path`)
- `alert_kind` — discriminator: `"dependabot"` vs `"code_scanning"`

### 1.3 Backend — `src-tauri/src/main.rs`

The Tauri command `fetch_security_alerts` delegates exclusively to `github::security::fetch_alerts`, which calls only the Dependabot endpoint. The command is registered in both the production `invoke_handler` and the mock handler.

### 1.4 Frontend — `src/main.js`

**`refreshData()`** (lines ~215–250): Invokes `fetch_security_alerts` with `state` from the filter dropdown — correctly passes the state param.

**`renderAlerts()`** (lines ~295–340): Renders the `#alerts-table tbody`. The table has 6 columns: ID, Severity, Summary, Package, Vulnerable, Patched — all Dependabot-specific. No column exists for Tool or Location.

**Help text — Instance 1** (error path, `isDisabled` branch):
```javascript
const guidance = isDisabled
  ? `<br><br><strong>To enable:</strong> Go to your repository on GitHub → 
     <strong>Settings</strong> → <strong>Security</strong> section → 
     <strong>Code security and analysis</strong> → 
     <strong>Dependabot alerts</strong> → click <strong>Enable</strong>. 
     Then click the refresh button (↺) above.`
  : "";
```
**WRONG PATH:** `Settings → Security section → Code security and analysis → Dependabot alerts → Enable`

**Help text — Instance 2** (empty alerts tips):
```javascript
<br>• To enable Dependabot: GitHub repository → <strong>Settings</strong> → <strong>Security</strong> section → <strong>Code security and analysis</strong> → <strong>Dependabot alerts</strong> → <strong>Enable</strong>.
```
**WRONG PATH:** `Settings → Security section → Code security and analysis → Dependabot alerts → Enable`

### 1.5 Frontend — `src/index.html`

The `#alerts-table` thead defines columns: `ID | Severity | Summary | Package | Vulnerable | Patched`  
This is Dependabot-specific. A "Code Scanning" alert has no package/version information; it has a tool name and file location.

### 1.6 Mock — `src-tauri/src/mock/mod.rs`

`fetch_security_alerts` mock data populates `SecurityAlert` with Dependabot-style fields only (no `tool_name`, `alert_kind`, or `location_path`). This must be updated when the model changes.

---

## 2. Root Causes

### Root Cause A — Missing Code Scanning Implementation (PRIMARY)

The application has **zero code** that calls the GitHub Code Scanning Alerts API (`/repos/{owner}/{repo}/code-scanning/alerts`). The entire feature is absent:

- No Rust structs for deserialising code scanning API responses
- No fetch function for the code scanning endpoint
- No Tauri command for code scanning
- No JS invocation of a code scanning command
- No UI columns or rendering logic for code scanning alert fields

**Impact:** A user who navigates to the "Security Alerts" tab will see only Dependabot results. If the repository has code scanning enabled but not Dependabot, the tab shows empty or an error — and gives no way to view code scanning findings.

### Root Cause B — Incorrect Settings Path in Help Text (SECONDARY)

Two separate locations in `src/main.js` (`renderAlerts()`) display the path:
> `Settings → Security section → Code security and analysis → Dependabot alerts → Enable`

The **correct** path in the GitHub UI is:
> `Settings → Security section → Advanced Security → Dependabot Section → Dependabot Alerts → Enable`

The intermediate navigation node `Code security and analysis` does not exist at this level in the current GitHub repository settings UI. The correct intermediate section is **Advanced Security**.

A third occurrence exists in the `anyhow::anyhow!` error string inside `security.rs`:
> `Settings → Security → Dependabot alerts`

This is abbreviated/incomplete and should also be corrected for consistency.

---

## 3. Incorrect Help Text — Full Details

| # | File | Context | Wrong Text | Correct Text |
|---|------|---------|-----------|-------------|
| 1 | `src/main.js` | `renderAlerts()` error guidance (disabled) | `Settings → Security section → Code security and analysis → Dependabot alerts → Enable` | `Settings → Security section → Advanced Security → Dependabot Section → Dependabot Alerts → Enable` |
| 2 | `src/main.js` | `renderAlerts()` empty-state tips | `Settings → Security section → Code security and analysis → Dependabot alerts → Enable` | `Settings → Security section → Advanced Security → Dependabot Section → Dependabot Alerts → Enable` |
| 3 | `src-tauri/src/github/security.rs` | `fetch_alerts` error message | `Settings → Security → Dependabot alerts` | `Settings → Security section → Advanced Security → Dependabot Section → Dependabot Alerts → Enable` |

---

## 4. GitHub API Requirements (Reference)

### Dependabot Alerts
- **Endpoint:** `GET /repos/{owner}/{repo}/dependabot/alerts`
- **Scope required:** `security_events` (classic PAT) or Repository permission "Dependabot alerts: Read" (fine-grained PAT)
- **Enable in GitHub:** Repository → Settings → Security section → Advanced Security → Dependabot Section → Dependabot Alerts → Enable

### Code Scanning Alerts
- **Endpoint:** `GET /repos/{owner}/{repo}/code-scanning/alerts`
- **Scope required:** `security_events` (classic PAT — same scope as Dependabot) or Repository permission "Code scanning alerts: Read" (fine-grained PAT)
- **Public repos:** `public_repo` scope is sufficient
- **Prerequisite:** Repository must have a code scanning workflow (e.g., GitHub Actions CodeQL)
- **Response fields relevant to this app:**
  ```
  number            → id
  state             → state ("open" | "dismissed" | "fixed")
  rule.id           → rule identifier
  rule.description  → summary
  rule.severity     → severity ("error" | "warning" | "note" | "none")
  tool.name         → tool_name
  most_recent_instance.location.path → location_path
  html_url          → html_url
  created_at        → created_at
  ```

---

## 5. Proposed Solution

### Strategy

Extend the existing `SecurityAlert` model with optional code-scanning-specific fields and an `alert_kind` discriminator. The `fetch_alerts` command fetches **both** Dependabot alerts and code scanning alerts, normalises them into the extended `SecurityAlert` type, and returns the combined list. The UI renders both types in a single table with an additional **Type** column and adapts the "Package/Location" column display based on `alert_kind`.

This minimises breaking changes: the Tauri command name, the JS invoke call, and the model serialisation shape remain the same except for new optional fields.

### 5.1 Model Changes — `src-tauri/src/models/mod.rs`

Add three optional fields and an `alert_kind` discriminator to `SecurityAlert`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityAlert {
    pub id: u64,
    pub severity: String,
    pub summary: String,
    pub description: String,
    pub package_name: Option<String>,
    pub vulnerable_version_range: Option<String>,
    pub patched_version: Option<String>,
    pub state: String,
    pub html_url: String,
    pub created_at: DateTime<Utc>,
    // NEW — code scanning specific (None for Dependabot alerts)
    pub alert_kind: String,          // "dependabot" | "code_scanning"
    pub tool_name: Option<String>,   // e.g. "CodeQL"
    pub location_path: Option<String>, // e.g. "src/auth.rs"
}
```

**Default values for existing Dependabot alerts:**
- `alert_kind`: `"dependabot"`
- `tool_name`: `None`
- `location_path`: `None`

**Note:** The `alert_kind`, `tool_name`, and `location_path` fields are additive and optional. Existing export logic (`csv_export.rs`, `pdf_export.rs`) will receive `None` for these new fields unless updated — this is safe (empty/omitted in output).

### 5.2 Backend Changes — `src-tauri/src/github/security.rs`

**Add** raw structs for deserialising code scanning alerts:

```rust
#[derive(Debug, Deserialize)]
struct RawCodeScanningAlert {
    number: u64,
    state: String,
    html_url: String,
    created_at: String,
    rule: Option<RawRule>,
    tool: Option<RawTool>,
    most_recent_instance: Option<RawInstance>,
}

#[derive(Debug, Deserialize)]
struct RawRule {
    description: Option<String>,
    severity: Option<String>, // "error" | "warning" | "note" | "none"
}

#[derive(Debug, Deserialize)]
struct RawTool {
    name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RawInstance {
    location: Option<RawLocation>,
}

#[derive(Debug, Deserialize)]
struct RawLocation {
    path: Option<String>,
}
```

**Add** a private helper `fetch_code_scanning_alerts_raw`:

```rust
async fn fetch_code_scanning_alerts_raw(
    client: &Octocrab,
    owner: &str,
    repo: &str,
    state: Option<&str>,
) -> Result<Vec<SecurityAlert>> {
    let url = if state == Some("open") {
        format!("/repos/{owner}/{repo}/code-scanning/alerts?per_page=100&state=open")
    } else {
        format!("/repos/{owner}/{repo}/code-scanning/alerts?per_page=100")
    };

    let raw: Vec<RawCodeScanningAlert> = client
        .get(&url, None::<&()>)
        .await
        .map_err(|e| {
            let detail = match &e {
                octocrab::Error::GitHub { source, .. } => {
                    format!("GitHub API error {}: {}", source.status_code, source.message)
                }
                other => other.to_string(),
            };
            anyhow::anyhow!(
                "Failed to fetch Code Scanning alerts: {}\n\
                \n\
                Token permission requirements:\n\
                • Fine-grained PAT: under Repository permissions → set 'Code scanning alerts' to Read\n\
                • Classic PAT: check the 'security_events' scope\n\
                \n\
                Also ensure Code Scanning is enabled for this repository \
                (add a CodeQL workflow via GitHub Actions).",
                detail
            )
        })?;

    Ok(raw.into_iter().map(|a| {
        let rule = a.rule.as_ref();
        let severity = rule
            .and_then(|r| r.severity.clone())
            .unwrap_or_else(|| "unknown".into());
        // Map API severity levels to match Dependabot conventions
        let severity = match severity.as_str() {
            "error"   => "high".to_string(),
            "warning" => "medium".to_string(),
            "note"    => "low".to_string(),
            other     => other.to_string(),
        };

        SecurityAlert {
            id: a.number,
            severity,
            summary: rule.and_then(|r| r.description.clone()).unwrap_or_default(),
            description: String::new(),
            package_name: None,
            vulnerable_version_range: None,
            patched_version: None,
            state: a.state,
            html_url: a.html_url,
            created_at: a.created_at.parse().unwrap_or_else(|_| chrono::Utc::now()),
            alert_kind: "code_scanning".to_string(),
            tool_name: a.tool.and_then(|t| t.name),
            location_path: a.most_recent_instance
                .and_then(|i| i.location)
                .and_then(|l| l.path),
        }
    }).collect())
}
```

**Modify** the public `fetch_alerts` function to:
1. Concurrently fetch both Dependabot and code scanning alerts
2. Treat code scanning fetch failures as non-fatal (a repo may have one but not the other)
3. Return a combined, sorted list (by id descending)

```rust
pub async fn fetch_alerts(
    client: &Octocrab,
    owner: &str,
    repo: &str,
    state: Option<&str>,
) -> Result<Vec<SecurityAlert>> {
    // Fetch both alert types concurrently; tolerate independent failures
    let (dep_result, cs_result) = tokio::join!(
        fetch_dependabot_alerts_raw(client, owner, repo, state),
        fetch_code_scanning_alerts_raw(client, owner, repo, state),
    );

    let mut combined: Vec<SecurityAlert> = Vec::new();

    match dep_result {
        Ok(alerts) => combined.extend(alerts),
        Err(e) => {
            // Re-raise only if code scanning also failed — one working source is acceptable
            if cs_result.is_err() {
                return Err(e);
            }
            // Otherwise log and continue
            eprintln!("[security] Dependabot fetch failed (continuing): {e}");
        }
    }

    match cs_result {
        Ok(alerts) => combined.extend(alerts),
        Err(e) => {
            // Code scanning may simply not be enabled — not an error unless Dependabot also failed
            eprintln!("[security] Code scanning fetch failed (non-fatal): {e}");
        }
    }

    // Sort by id descending (newest alert first)
    combined.sort_by(|a, b| b.id.cmp(&a.id));
    Ok(combined)
}
```

Rename the existing fetch logic into a private `fetch_dependabot_alerts_raw` function (same body, just extracted).

**Note:** `tokio::join!` requires both futures to be on the same executor. Since `Octocrab::get` is already async and `main.rs` uses `tokio`, this works without any additional dependencies.

### 5.3 Main Command — `src-tauri/src/main.rs`

No changes needed to the `fetch_security_alerts` Tauri command signature or registration. The command already delegates to `github::security::fetch_alerts` which will now internally fetch both types. The `state: Option<String>` parameter is forwarded correctly to the updated function.

### 5.4 Frontend — `src/main.js`

**Fix 1 — Incorrect help text (error path, Instance 1):**
Change:
```
Settings → Security section → Code security and analysis → Dependabot alerts → Enable
```
To:
```
Settings → Security section → Advanced Security → Dependabot Section → Dependabot Alerts → Enable
```

**Fix 2 — Incorrect help text (empty state, Instance 2):**
Same path correction as Fix 1.

**Fix 3 — Update `renderAlerts()` table rendering:**

Add a `Type` column to the rendered rows to distinguish Dependabot vs code scanning. For code scanning alerts, show `location_path` instead of the package/version fields.

```javascript
function renderAlerts() {
  const tbody = $("#alerts-table tbody");
  // ... (error and empty handling unchanged except text path fix) ...
  tbody.innerHTML = alerts
    .map((a) => {
      const sev = a.severity.toLowerCase();
      const cls = sev === "critical" ? "severity-critical"
                : sev === "high"     ? "severity-high"
                : sev === "medium"   ? "severity-medium"
                :                      "severity-low";
      const isCodeScanning = a.alert_kind === "code_scanning";
      const typeLabel = isCodeScanning
        ? `<span class="badge badge-label">Code Scanning</span>`
        : `<span class="badge badge-label">Dependabot</span>`;
      // Package column: show file location for code scanning, package name for Dependabot
      const packageOrLocation = isCodeScanning
        ? esc(a.location_path || "—")
        : esc(a.package_name || "—");
      // Tool column (replaces "Vulnerable" for code scanning)
      const toolOrVuln = isCodeScanning
        ? esc(a.tool_name || "—")
        : esc(a.vulnerable_version_range || "—");
      // Patched column not applicable to code scanning
      const patched = isCodeScanning ? "—" : esc(a.patched_version || "—");

      return `<tr>
        <td>${a.id}</td>
        <td>${typeLabel}</td>
        <td class="${cls}">${esc(a.severity)}</td>
        <td><a href="${a.html_url}" target="_blank">${esc(a.summary)}</a></td>
        <td>${packageOrLocation}</td>
        <td>${toolOrVuln}</td>
        <td>${patched}</td>
      </tr>`;
    })
    .join("");
}
```

**Fix 4 — Update empty-state message:**
Change "No Dependabot alerts found" to "No security alerts found" since the tab now covers both types.

### 5.5 Frontend — `src/index.html`

Update `#alerts-table` thead to add a **Type** column and relabel **Vulnerable** to **Tool / Vuln Range**:

```html
<thead>
  <tr>
    <th>ID</th>
    <th>Type</th>
    <th>Severity</th>
    <th>Summary</th>
    <th>Package / Location</th>
    <th>Tool / Vuln Range</th>
    <th>Patched</th>
  </tr>
</thead>
```

### 5.6 Mock — `src-tauri/src/mock/mod.rs`

Update `fetch_security_alerts` mock to:
1. Add the new fields `alert_kind`, `tool_name`, `location_path` to existing Dependabot mock entries
2. Add at least one code scanning mock alert to demonstrate the UI difference

```rust
// Existing Dependabot alerts — add new fields:
SecurityAlert {
    // ... existing fields ...
    alert_kind: "dependabot".to_string(),
    tool_name: None,
    location_path: None,
},
// New code scanning alert:
SecurityAlert {
    id: 5,
    severity: "high".to_string(),
    summary: "SQL injection via unsanitized user input".to_string(),
    description: String::new(),
    package_name: None,
    vulnerable_version_range: None,
    patched_version: None,
    state: "open".to_string(),
    html_url: "https://github.com/octocat/Hello-World/security/code-scanning/5".to_string(),
    created_at: dt("2026-01-10T08:00:00Z"),
    alert_kind: "code_scanning".to_string(),
    tool_name: Some("CodeQL".to_string()),
    location_path: Some("src/db/query.rs".to_string()),
},
```

### 5.7 Backend error message — `src-tauri/src/github/security.rs`

Fix the abbreviated settings path in the Dependabot error `anyhow::anyhow!` string:

Change:
```
(Settings → Security → Dependabot alerts).
```
To:
```
(Settings → Security section → Advanced Security → Dependabot Section → Dependabot Alerts → Enable).
```

---

## 6. Implementation Steps

1. **`src-tauri/src/models/mod.rs`** — Add `alert_kind: String`, `tool_name: Option<String>`, `location_path: Option<String>` to `SecurityAlert`. Provide defaults where needed.

2. **`src-tauri/src/github/security.rs`** — Rename existing fetch body to `fetch_dependabot_alerts_raw`. Add `RawCodeScanningAlert`, `RawRule`, `RawTool`, `RawInstance`, `RawLocation` structs. Add `fetch_code_scanning_alerts_raw` function. Rewrite `fetch_alerts` to use `tokio::join!` over both private functions and return the combined list. Fix the error message path string.

3. **`src-tauri/src/mock/mod.rs`** — Update existing `SecurityAlert` mock structs to include new fields. Add one code scanning mock alert.

4. **`src/index.html`** — Update `#alerts-table` thead to 7 columns: ID, Type, Severity, Summary, Package / Location, Tool / Vuln Range, Patched.

5. **`src/main.js`** — Fix two incorrect settings path strings in `renderAlerts()`. Update `renderAlerts()` to render 7 columns with `alert_kind` branching. Update empty-state message from "No Dependabot alerts" to "No security alerts".

6. **Verify build** — `cargo build` from `src-tauri/`, then `cargo clippy -- -D warnings`, then `cargo test`.

---

## 7. Dependencies & Risks

### Dependencies

| Dependency | Status | Notes |
|-----------|--------|-------|
| `tokio` with `full` features | Already present (`Cargo.toml`) | Required for `tokio::join!` |
| `octocrab` `0.38` | Already present | Supports raw `client.get()` for both endpoints |
| No new Cargo crates needed | — | — |

### Risks & Mitigations

| Risk | Likelihood | Mitigation |
|------|-----------|------------|
| Code scanning API returns 403/404 if not enabled on repo | High | Already mitigated — `cs_result` failures are non-fatal; only logged to `eprintln!` unless Dependabot also fails |
| `alert_kind` field breaks existing export CSV/PDF consumers | Low | New field is additive; export modules will receive `None` for optional fields and can safely skip them |
| Column count change in `#alerts-table` (6→7) breaks existing CSS table layout | Low | One extra `<th>/<td>` — may need minor CSS width adjustment in `styles.css` if columns are fixed-width |
| Mock structs fail to compile after model change | Certain (compile error if not updated) | Step 3 above updates all mock structs before build verification |
| `tokio::join!` requires both futures to be `Send` | Low | `Octocrab::get` returns `Send` futures; no `Mutex`/`RefCell` is held across await points |
| Rate limiting — two concurrent API requests per `refreshData()` call | Low | Both calls count against the 5,000 req/hr GitHub API rate limit; two requests per refresh is negligible |

### Breaking Changes

- `SecurityAlert` model gains 3 new serialised fields (`alert_kind`, `tool_name`, `location_path`). Any callers deserialising `SecurityAlert` from JSON (e.g., tests or future frontend code) will receive the new fields. This is **additive** and backward-compatible in JSON.
- The alerts table changes from 6 to 7 columns. The `export_data` Tauri command accepts `Vec<SecurityAlert>` directly, so no signature change is needed — but CSV/PDF export implementations should be reviewed to optionally include the new fields.

---

## 8. Affected Files Summary

| File | Change Type | Summary |
|------|-------------|---------|
| `src-tauri/src/models/mod.rs` | Modify | Add 3 fields to `SecurityAlert` |
| `src-tauri/src/github/security.rs` | Modify | Add code scanning fetch; fix help text; `tokio::join!` both |
| `src-tauri/src/mock/mod.rs` | Modify | Update mock structs; add code scanning mock entry |
| `src/index.html` | Modify | Update `#alerts-table` thead (6→7 columns) |
| `src/main.js` | Modify | Fix 2× incorrect path strings; update `renderAlerts()` for 7 columns + `alert_kind` branching |
| `src-tauri/src/main.rs` | None | No changes required |
| `src-tauri/Cargo.toml` | None | No new dependencies required |
