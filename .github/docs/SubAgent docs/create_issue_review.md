# Review: Create New Issue Feature

**Feature:** `create_issue`  
**Reviewed:** 2026-03-07  
**Reviewer:** QA Subagent  

---

## Build Output

### 1. `cargo build`
```
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.49s
```
**Result: PASS**

### 2. `cargo clippy -- -D warnings`
```
Checking github-export v0.1.0 (C:\Projects\github-export\src-tauri)
Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.38s
```
**Result: PASS** (zero warnings)

### 3. `cargo test`
```
Compiling github-export v0.1.0 (C:\Projects\github-export\src-tauri)
Finished `test` profile [unoptimized + debuginfo] target(s) in 3.47s
Running unittests src\main.rs
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured
```
**Result: PASS**

---

## Findings

### CRITICAL

#### C-1 — Button Never Becomes Visible on Initial Repo Selection

**File:** `src/main.js` (refreshData) · `src/index.html` (toolbar)

`#btn-new-issue` starts with `.hidden` in the HTML. In `refreshData()`, only `disabled = false` is set — the `.hidden` class is **never removed** from here. Visibility is only toggled inside the `$$(".tab").forEach` click handler, which fires only when the user explicitly clicks a tab button.

**Result:** On normal first use — select a repo on the already-active Issues tab — the button never appears. The user must click away to another tab and back to Issues before the "+ New Issue" button becomes visible. The feature is effectively invisible by default.

**Required fix:** In `refreshData()`, after enabling the button, also conditionally un-hide it when the current active tab is `"issues"`:
```js
// After: document.getElementById("btn-new-issue").disabled = false;
if (activeTab === "issues") {
  document.getElementById("btn-new-issue").classList.remove("hidden");
}
```

---

### WARNING

#### W-1 — JS Body Length Validation Missing

**File:** `src/main.js` (btn-create-issue-submit handler, line ~885)

The submit handler validates title (empty + ≤ 256 chars) client-side, but does **not** validate that the body is ≤ 65,536 characters. Rust validates this server-side, but the spec requires validation in both layers.

**Impact:** A user entering a body > 65,536 chars sends a round-trip to Rust before receiving an error. Inconsistent with the defensive JS validation pattern used for the title field.

**Required fix:**
```js
if (body && body.length > 65536) {
  errorEl.textContent = "Issue body must be 65,536 characters or fewer.";
  errorEl.classList.remove("hidden");
  return;
}
```

#### W-2 — Textarea Missing `maxlength` HTML Attribute

**File:** `src/index.html` (line ~209)

The `#new-issue-title` input correctly has `maxlength="256"`. The `#new-issue-body` textarea does **not** have `maxlength="65536"`. Without it, the browser provides no native length enforcement, and the user can type arbitrarily long text before hitting the JS/Rust validation.

**Required fix:** Add `maxlength="65536"` to the `#new-issue-body` textarea.

---

### RECOMMENDED

#### R-1 — Mock `create_issue` Does Not Validate Inputs

**File:** `src-tauri/src/mock/mod.rs` (line ~524)

The spec (section 4.1) called for the mock to also validate empty title and length > 256, returning `Err(...)` in those cases. The current mock passes through any title unchecked. While the JS validates before invoke, this means dev-mock mode provides different error feedback than production.

#### R-2 — Minor DRY Violation in `issues.rs` `create_issue`

**File:** `src-tauri/src/github/issues.rs` (lines ~215–246)

The `match body` duplicates `.context("Failed to create issue")` across both arms. The mutable-builder pattern from the spec is cleaner:

```rust
let mut builder = client.issues(owner, repo).create(title);
if let Some(b) = body {
    builder = builder.body(b);
}
let issue = builder.send().await.context("Failed to create issue")?;
Ok(map_issue(issue))
```

#### R-3 — Mock State Value Inconsistency

**File:** `src-tauri/src/mock/mod.rs` (line ~538)

Mock `create_issue` returns `state: "Open"` (capitalized), while all other mock issues return `state: "open"` (lowercase). The JS `stateBadge()` uses `.toLowerCase()` so rendering is correct, but the inconsistency may cause confusion in tests. (Note: production `map_issue` via `format!("{:?}", i.state)` returns "Open", so mock `create_issue` is actually closer to production behavior here. The issue is with the pre-existing mocks using lowercase, not with this feature specifically.)

---

## Specification Compliance: Detailed Checklist

| Check | Status | Notes |
|---|---|---|
| `create_issue` function in `issues.rs` | ✅ Pass | Uses octocrab builder, `map_issue`, `context()` |
| Tauri command in `main.rs` | ✅ Pass | Auth pattern matches `close_issue`/`reopen_issue` |
| Mock stub in `mock/mod.rs` | ✅ Pass | Returns `Issue { number: 999, ... }` |
| Command registered in non-mock handler | ✅ Pass | Present in `generate_handler![]` |
| Command registered in mock handler | ✅ Pass | `mock::create_issue` in mock block |
| `#btn-new-issue` in toolbar | ✅ Pass | Present, starts hidden + disabled |
| `#create-issue-modal` present | ✅ Pass | With all required sub-elements |
| Title input with `maxlength="256"` | ✅ Pass | `#new-issue-title` |
| Body textarea | ✅ Pass | `#new-issue-body`, rows=6 |
| Error span `#create-issue-error` | ✅ Pass | Uses `.login-error hidden` |
| Submit + Cancel buttons | ✅ Pass | Correct IDs |
| Button hidden on non-issues tabs | ✅ Pass | Tab click handler correctly hides it |
| Button shown on issues tab (initial) | ❌ Fail | **C-1** — requires tab switch to appear |
| After success: issue prepended | ✅ Pass | `issues.unshift(newIssue)` + `renderIssues()` |
| After success: modal closed | ✅ Pass | `hidden` class added |
| After success: form cleared | ✅ Pass | title + body values set to "" |
| No `.unwrap()` in production paths | ✅ Pass | Uses `?`/`.context()` throughout |
| Buttons disabled during in-flight | ✅ Pass | Both submit + cancel disabled |
| Title validated empty (JS) | ✅ Pass | |
| Title validated ≤ 256 (JS) | ✅ Pass | |
| Title validated empty (Rust) | ✅ Pass | |
| Title validated ≤ 256 (Rust) | ✅ Pass | |
| Body validated ≤ 65536 (JS) | ❌ Fail | **W-1** — missing JS-side check |
| Body validated ≤ 65536 (Rust) | ✅ Pass | |
| Textarea `maxlength` attribute | ❌ Fail | **W-2** — missing `maxlength="65536"` |
| No token exposed to frontend | ✅ Pass | Client extracted server-side only |
| Modal HTML matches existing pattern | ✅ Pass | Same structure as `add-account-modal` |
| Modal JS open/close matches pattern | ✅ Pass | backdrop click, field clear, focus |
| CSS reuses base modal classes | ✅ Pass | New styles additive only |
| Error inline (no alert()) | ✅ Pass | `errorEl.textContent` used |

---

## Score Table

| Category | Score | Grade |
|---|---|---|
| Specification Compliance | 84% | B |
| Best Practices | 91% | A- |
| Functionality | 80% | B- |
| Code Quality | 88% | B+ |
| Security | 85% | B |
| Performance | 100% | A+ |
| Consistency | 95% | A |
| Build Success | 100% | A+ |

**Overall Grade: B+ (90%)**

---

## Summary

The implementation is structurally complete and all three build commands pass. The Rust backend (`issues.rs`, `main.rs`, `mock/mod.rs`) is well-implemented with no `.unwrap()` calls and full input validation. The modal HTML, CSS, and JS follow existing patterns closely.

**Two issues require fixes before this can ship:**

1. **C-1 (CRITICAL):** The "+ New Issue" button is never shown unless the user switches tabs — `refreshData()` must also call `classList.remove("hidden")` when on the issues tab.
2. **W-1 (WARNING):** The JS submit handler must validate body ≤ 65,536 chars before invoking, consistent with how title is validated client-side.
3. **W-2 (WARNING):** The textarea should carry `maxlength="65536"` for browser-level enforcement.

---

## Final Verdict: **NEEDS_REFINEMENT**

Build: PASS | Review: NEEDS_REFINEMENT  
Critical issues: 1 | Warnings: 2 | Recommended: 3
