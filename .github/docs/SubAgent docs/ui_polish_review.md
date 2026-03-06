# UI Polish Review — Account Dropdown & Add Repository Button

**Date:** 2026-03-05  
**Reviewer:** Review & QA Subagent  
**Spec:** `.github/docs/SubAgent docs/ui_polish_spec.md`  
**Verdict:** ✅ **PASS**

---

## Build Validation

### `cargo build`
```
Compiling github-export v0.1.0 (C:\Projects\github-export\src-tauri)
Finished `dev` profile [unoptimized + debuginfo] target(s) in 3.68s
```
**Exit code: 0** ✅

### `cargo clippy -- -D warnings`
```
Checking github-export v0.1.0 (C:\Projects\github-export\src-tauri)
Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.28s
```
**Exit code: 0** ✅

### `cargo test`
```
Compiling github-export v0.1.0 (C:\Projects\github-export\src-tauri)
Finished `test` profile [unoptimized + debuginfo] target(s) in 3.03s
Running unittests src\main.rs (target\debug\deps\github_export-5352030d8f3a8e9e.exe)

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```
**Exit code: 0** ✅

---

## Score Table

| Category | Score | Grade |
|---|---|---|
| Specification Compliance | 100% | A+ |
| Best Practices | 93% | A |
| Functionality | 100% | A+ |
| Code Quality | 97% | A+ |
| Security | 100% | A+ |
| Performance | 97% | A+ |
| Consistency | 100% | A+ |
| Build Success | 100% | A+ |

**Overall Grade: A+ (98%)**

---

## Review Findings

### CSS Quality ✅

**Old conflicting rules — fully cleared:**
- `.sidebar-header button` (the old logout-button sub-rule) has been **removed** from the first `.sidebar-header` block. No orphaned rule found. ✅
- The second `.sidebar-header { position: relative; }` block (positioning context for the dropdown) is **preserved** at its correct location in the multi-account switcher section. ✅
- No duplicate rules for `.account-chip`, `.account-menu-btn`, `.btn-add-repo`, or `.account-menu`. Each selector appears exactly once. ✅

**Animation:**
- `@keyframes menu-appear` is present and correct: `opacity: 0 → 1`, `translateY(-6px) → 0`, `scale(0.97) → 1`. ✅
- `transform-origin: top center` on `.account-menu` ensures the scale origin is the menu top edge. ✅
- The animation fires each time `.hidden` is removed (element becomes `display: block`), which is the correct trigger point. No conflict with `.hidden { display: none !important; }`. ✅

**Chevron rotation:**
- `.chevron-icon { transition: transform 0.2s ease; }` — present at CSS line 796. ✅
- `.account-menu-btn[aria-expanded="true"] .chevron-icon { transform: rotate(180deg); }` — present at CSS line 802. Targets the correct element (`#account-menu-btn`), and JS sets `aria-expanded` on that same element. ✅

**`.btn-add-repo` states:**
- `:hover` — replaces dashed border with `rgba(88,166,255,0.1)` fill + `rgba(88,166,255,0.65)` border + `translateY(-1px)` lift. ✅
- `:active` — resets transform and shadow, slightly stronger fill. ✅
- Transitions cover all four changed properties: `background`, `border-color`, `transform`, `box-shadow`. ✅

**Color consistency:**
- All new selectors use `var(--bg)`, `var(--surface)`, `var(--border)`, `var(--accent)`, `var(--text)`, `var(--text-muted)`, `var(--green)`, `var(--red)` where appropriate. ✅
- New inline values (`#1c2128`, `#3d444d`, `#21262d`, `#292e36`, `#484f58`) are all within GitHub's Primer dark-mode colour ramp as documented in the spec. ✅
- `.account-menu` does **not** have a `display` property, preserving full control to `.hidden`. ✅

---

### HTML Correctness ✅

- SVG chevron inside `#account-menu-btn` is valid inline SVG with correct `viewBox="0 0 12 12"`, `fill="currentColor"`, and the standard Primer downward-chevron path. ✅
- `aria-haspopup="true"` — present. ✅
- `aria-expanded="false"` — present (initial state). ✅
- `.chevron-icon` class on the `<svg>` element — present. ✅
- `aria-hidden="true"` on the `<svg>` — present (correct, as the button's `title` attribute carries the accessible label). ✅
- `▾` Unicode character **fully removed** — the button content is now the SVG only, no `▾` remains. ✅

---

### JS Compatibility ✅

All JS-driven class mutations validated against implemented CSS:

| JS reference | CSS rule present | Notes |
|---|---|---|
| `menu.classList.toggle("hidden", isOpen)` | `.hidden { display: none !important; }` | ✅ Global utility, unchanged |
| `btn.setAttribute("aria-expanded", ...)` | `.account-menu-btn[aria-expanded="true"] .chevron-icon` | ✅ Correct selector target |
| `document.addEventListener("click", ...)` closes menu | same `.hidden` + `aria-expanded="false"` | ✅ |
| `li.className = "account-switcher-item"` | `.account-switcher-item` | ✅ |
| `+ " account-active"` | `.account-active`, `.account-active:hover`, `.account-active .account-item-label` | ✅ |
| `class="account-active-dot"` | `.account-active-dot` (CSS circle, `font-size: 0`) | ✅ text `●` suppressed by `font-size: 0`, CSS circle rendered |
| `class="account-item-username"` | `.account-item-username` (new rule) | ✅ |
| `class="account-item-label"` | `.account-item-label` | ✅ |
| `class="account-menu-action"` | `.account-menu-action` | ✅ |
| `class="account-menu-action account-menu-danger"` | `.account-menu-danger`, `.account-menu-danger:hover` | ✅ |
| `class="account-menu-divider"` | `.account-menu-divider` | ✅ |
| `class="account-menu-section-label"` | `.account-menu-section-label` | ✅ |
| `id="account-list"` / `.account-switcher-list` | `.account-switcher-list` | ✅ |

---

### Security ✅

- No new user-controlled content is inserted as raw HTML in modified elements.
- All dynamic content in `renderAccountSwitcher()` is wrapped with `esc()` (HTML-escaping helper) — `esc(acct.username)` and `esc(acct.label)`. ✅
- The `●` injected into `.account-active-dot` is a static string constant, not user data. ✅
- `aria-label="Active"` on the dot element is a hardcoded string. ✅

---

## Issues

### SUGGESTION (non-blocking) — Missing `aria-controls` on `#account-menu-btn`

**Severity:** SUGGESTION  
**Location:** `src/index.html` — `#account-menu-btn`  
**Detail:** The button does not include `aria-controls="account-menu"`. This attribute would link the toggle button to the controlled panel for screen readers (ARIA 1.1 best practice). This was not present in the original HTML and was not required by the spec, so it is a pre-existing gap, not a regression introduced here. Adding it is a low-effort improvement for a future pass.

**Suggested fix:**
```html
<button id="account-menu-btn" class="account-menu-btn"
        title="Manage accounts"
        aria-haspopup="true"
        aria-expanded="false"
        aria-controls="account-menu">
```

---

### SUGGESTION (non-blocking) — No exit/close animation on menu dismiss

**Severity:** SUGGESTION  
**Location:** `src/styles.css` — `.account-menu`  
**Detail:** The entrance animation (`menu-appear`) runs when the menu opens. No exit animation exists — when `.hidden` is re-applied the menu disappears immediately. A matching `menu-disappear` animation would complete the polish, but it requires JS coordination (delay adding `.hidden` until animation finishes) and was explicitly out of scope for this spec. No action required at this time.

---

## Summary

The implementation is a faithful, high-quality execution of the spec. Every CSS replacement block matches the specification exactly. The HTML change is minimal and correct. All JS-driven class mutations are compatible with the new CSS rules. No conflicting or orphaned CSS rules were introduced. All three build/lint/test commands pass with exit code 0. The two suggestions above are pre-existing or explicitly out-of-scope items — neither affects functionality, correctness, or accessibility in a blocking way.

---

## Final Verdict

**✅ PASS**
