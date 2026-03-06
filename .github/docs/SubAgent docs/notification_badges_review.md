# Review: Notification Badge Feature — Quality Assurance Report

**Project:** GitHub Export (Tauri v1 — Rust + Vanilla HTML/CSS/JS)  
**Feature:** Numeric count badges on Issues, Pull Requests, and Security Alerts tab buttons  
**Date:** 2026-03-05  
**Reviewer:** QA Subagent  
**Spec File:** `.github/docs/SubAgent docs/notification_badges_spec.md`

---

## Build Validation Results

| Step | Command | Result |
|------|---------|--------|
| Rust build | `cargo build` (from `src-tauri/`) | ✅ PASS — `Finished dev profile in 3.48s` |
| Clippy lint | `cargo clippy -- -D warnings` (from `src-tauri/`) | ✅ PASS — no warnings |
| Tests | `cargo test` (from `src-tauri/`) | ✅ PASS — `0 tests; 0 failed` |

**Build result: PASS**  
No Rust source files were modified by this feature (frontend-only change), which is consistent with the spec's conclusion that no backend changes were required.

---

## Specification Compliance Audit

### HTML (`src/index.html`)

| Check | Status | Notes |
|---|---|---|
| All three tab buttons updated with badge spans | ✅ PASS | `<span class="tab-badge" aria-hidden="true"></span>` in all three buttons |
| `aria-hidden="true"` on badge spans | ✅ PASS | Present statically in HTML as required |
| Badge spans placed *after* text node inside button | ✅ PASS | Correct |
| Button IDs added (`issues-tab`, `pulls-tab`, `security-tab`) | ✅ PASS | Implementation uses button IDs + descendant selector rather than span IDs |
| Spec's `id="badge-issues/pulls/alerts"` on spans | ⚠️ DEVIATION | Spec used span IDs; implementation used button IDs (`id="issues-tab"` etc.) with CSS descendant selector `#issues-tab .tab-badge`. Functionally equivalent — JS correctly finds badges via `$(badgeSel)`. |
| `class="tab-badge hidden"` starting state | ⚠️ DEVIATION | Spec requires `.hidden` class for initial hidden state. Implementation uses CSS `display: none` on `.tab-badge` rule, with JS toggling via `badge.style.display`. Functionally identical — badge is hidden on load. |

### CSS (`src/styles.css`)

| Check | Status | Notes |
|---|---|---|
| `position: relative` added to `.tab` rule | ✅ PASS | Present, positioned first in declaration block |
| `.tab-badge` rule added after `.tab.active` | ✅ PASS | Rule is present immediately after `.tab.active { ... }` |
| `position: absolute` | ✅ PASS | |
| `top: 4px; right: 4px` | ✅ PASS | |
| `min-width: 18px` | ✅ PASS | |
| `height: 18px` | ✅ PASS | |
| `border-radius: 9px` | ✅ PASS | |
| `pointer-events: none` | ✅ PASS | |
| `box-sizing: border-box` | ✅ PASS | |
| `line-height: 18px` | ✅ PASS | |
| `text-align: center` | ✅ PASS | |
| `background: #1f6feb` | ✅ PASS | Correctly implements spec section 8.4 accessibility recommendation |
| `color: #fff` | ✅ PASS | (`#ffffff` — equivalent) |
| `font-size: 0.65rem` | ⚠️ DEVIATION | Implemented as `11px` (≈10.4px at 16px base). Close but not spec-identical. |
| `font-weight: 700` | ⚠️ DEVIATION | Implemented as `600` (semi-bold). Spec required bold (700) for legibility at small size. |
| `padding: 0 5px` | ⚠️ DEVIATION | Implemented as `0 4px`. 1px difference; minor visual effect on wider badges. |
| `white-space: nowrap` | ❌ MISSING | Spec explicitly requires this to prevent "99+" from wrapping. Not present in implemented rule. |
| Default hidden state | ⚠️ DEVIATION | Uses CSS `display: none` instead of spec's dependency on `.hidden` class. Works because JS uses `badge.style.display` directly. |

### JavaScript (`src/main.js`)

| Check | Status | Notes |
|---|---|---|
| `updateTabBadges()` function present | ✅ PASS | |
| `clearTabBadges()` function present | ✅ PASS | |
| `updateTabBadges` called in `refreshData()` after renders | ✅ PASS | Line 640: `updateTabBadges(issues.length, pulls.length, alerts.length)` |
| `clearTabBadges()` called in logout handler | ✅ PASS | Line 167 |
| `clearTabBadges()` called in `handleSwitchAccount()` | ✅ PASS | Line 235 |
| `clearTabBadges()` called in `handleRemoveTrackedRepo()` | ✅ PASS | Line 488 |
| `99+` truncation for count > 99 | ✅ PASS | `count > 99 ? '99+' : String(count)` |
| count = 0 hides badge | ✅ PASS | `badge.style.display = 'none'` when count = 0 |
| Badge text set via `textContent` (not innerHTML) | ✅ PASS | `badge.textContent = label` — no XSS risk |
| No extra IPC calls for badge counts | ✅ PASS | Counts derived from `issues.length`, `pulls.length`, `alerts.length` |
| `aria-label` removed when count = 0 | ⚠️ DEVIATION | Implementation always calls `btn.setAttribute('aria-label', ...)`. When count=0, sets `aria-label="Issues"` (redundant with visible text) instead of removing attribute per spec. Functionally harmless but not spec-compliant. |
| TAB_LABELS constant (section 6.6) | ⚠️ DEVIATION | Spec specifies a `TAB_LABELS` map constant in the State section. Implementation hardcodes labels inline in the tabs array. Functionally equivalent; minor style deviation. |
| Function signature: `updateTabBadges()` (no args) | ⚠️ DEVIATION | Spec's final design reads from module-level state. Implementation uses explicit args `(issueCount, pullCount, alertCount)`. This is a **better** design pattern (explicit data flow, more testable) — acceptable deviation. |
| Error state suppresses badge | ✅ PASS (implicit) | On fetch error, `issues = []` so `issues.length = 0` → badge hides. No `hasError` parameter needed due to data flow. Functionally equivalent to spec's guard. |

---

## Detailed Findings

### CRITICAL Issues
*None found.*

---

### RECOMMENDED Improvements

#### R1 — Missing `white-space: nowrap` in `.tab-badge` CSS
**File:** `src/styles.css`  
**Spec reference:** Section 4.2  
**Description:** The spec explicitly requires `white-space: nowrap` to prevent the "99+" string from wrapping inside the badge element. While unlikely at the current `min-width: 18px` with `padding: 0 4px`, the absence of this property violates the spec and could produce incorrect rendering if padding/zoom conditions change.  
**Fix:** Add `white-space: nowrap;` to the `.tab-badge` rule.

#### R2 — `aria-label` should be removed (not redundantly set) when count = 0
**File:** `src/main.js`  
**Spec reference:** Section 6.1 (final TAB_LABELS version), Section 8.1–8.2  
**Description:** When `count === 0`, the implementation calls `btn.setAttribute('aria-label', 'Issues')` instead of `btn.removeAttribute('aria-label')`. Per WAI-ARIA: when `aria-label` is present, it overrides the accessible name from visible content. Setting `aria-label="Issues"` when the button already reads "Issues" is semantically redundant and adds unnecessary verbosity to the accessibility tree.  
**Fix:** Replace the always-set `setAttribute` with a conditional: remove when count=0, set when count>0.

#### R3 — `font-weight: 600` should be `700`
**File:** `src/styles.css`  
**Spec reference:** Section 4.2 (property rationale table)  
**Description:** Spec specifies `font-weight: 700` (bold) for legibility at the 11px badge size. `600` (semi-bold) is slightly thinner and may reduce contrast with the background at small render sizes.

---

### MINOR Issues

#### M1 — `font-size: 11px` vs spec's `0.65rem`
**File:** `src/styles.css`  
At a standard 16px root, `0.65rem ≈ 10.4px`. The implemented `11px` is slightly larger. Visually near-equivalent, but the spec uses a relative unit for scaling with user font preferences. Not a functional defect.

#### M2 — `padding: 0 4px` vs spec's `0 5px`
**File:** `src/styles.css`  
1px difference on each side. Reduces horizontal breathing room very slightly for multi-digit numbers. Not visually impactful at this scale.

#### M3 — Badge span IDs not added; button IDs used instead
**File:** `src/index.html`  
The spec defines `id="badge-issues"`, `id="badge-pulls"`, `id="badge-alerts"` on the badge spans. The implementation added `id="issues-tab"`, `id="pulls-tab"`, `id="security-tab"` to the **buttons** and uses `#issues-tab .tab-badge` CSS descendant selectors. Functionally equivalent. However, if additional `.tab-badge` children are ever added to the button, the selector becomes ambiguous.

#### M4 — No `TAB_LABELS` constant
**File:** `src/main.js`  
Spec section 6.6 specifies a top-level `TAB_LABELS` constant in the State section. Labels are instead hardcoded inline in the `tabs` array inside `updateTabBadges`. Functionally equivalent; minor maintainability concern.

#### M5 — `clearTabBadges()` leaves redundant `aria-label` attributes
**File:** `src/main.js`  
`clearTabBadges()` delegates to `updateTabBadges(0, 0, 0)`, which sets `aria-label="Issues"` etc. (redundant). Tied to R2 — fixing R2 also resolves this issue.

---

## Security Review

| Check | Status |
|---|---|
| Badge text set via `textContent` (not `innerHTML`) | ✅ PASS — `badge.textContent = label` |
| Badge label derived from numeric array length (never user input) | ✅ PASS |
| No new IPC surface introduced | ✅ PASS |
| No new HTML injection vectors | ✅ PASS |

No security issues found.

---

## Performance Review

| Check | Status |
|---|---|
| No additional IPC calls | ✅ PASS — counts read from existing JS arrays |
| No forced reflows from DOM batching issues | ✅ PASS — reads before writes within each badge update |
| Badges updated once per `refreshData()` cycle | ✅ PASS |

No performance issues found.

---

## Score Table

| Category | Score | Grade |
|----------|-------|-------|
| Specification Compliance | 78% | C+ |
| Best Practices | 80% | B- |
| Functionality | 97% | A+ |
| Code Quality | 85% | B |
| Security | 100% | A+ |
| Performance | 100% | A+ |
| Consistency | 88% | B+ |
| Build Success | 100% | A+ |

**Overall Grade: B+ (91%)**

---

## Summary of Findings

The notification badge feature is **functionally complete and correct**. All three tab buttons received badge spans, `position: relative` was correctly added to `.tab`, and the JavaScript helpers (`updateTabBadges`, `clearTabBadges`) are integrated in all required call sites. Build validation passed all three checks (compile, lint, test) with zero errors or warnings.

Key deviations from the spec are minor:
- The CSS `.tab-badge` rule is missing `white-space: nowrap` (could affect "99+" rendering in edge cases — **RECOMMENDED fix**).
- `aria-label` is not removed on count=0, it is set redundantly (harmless but non-conformant — **RECOMMENDED fix**).
- Cosmetic CSS value differences (`font-size`, `font-weight`, `padding`) are within acceptable range.
- Badge implementation uses button IDs + descendant selectors instead of span IDs — architecturally sound alternative.

No CRITICAL issues were found. No security or performance concerns exist.

---

## Final Verdict

**PASS**

> The feature is functionally correct, secure, and build-valid. Recommended improvements (R1, R2, R3) should be addressed in a follow-up pass for full spec compliance and accessibility conformance, but do not block delivery.
