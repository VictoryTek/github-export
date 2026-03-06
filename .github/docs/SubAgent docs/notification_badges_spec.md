# Specification: Notification-Style Badge Dots for Tab Navigation

**Project:** GitHub Export (Tauri v1 — Rust + Vanilla HTML/CSS/JS)  
**Feature:** Numeric count badges on Issues, Pull Requests, and Security Alerts tab buttons  
**Date:** 2026-03-05  
**Status:** DRAFT — Ready for Implementation

---

## Table of Contents

1. [Current State Analysis](#1-current-state-analysis)
2. [Research Findings](#2-research-findings)
3. [Proposed Badge Design](#3-proposed-badge-design)
4. [CSS Implementation](#4-css-implementation)
5. [HTML Implementation](#5-html-implementation)
6. [JavaScript Implementation](#6-javascript-implementation)
7. [Rust Backend Changes](#7-rust-backend-changes)
8. [Accessibility](#8-accessibility)
9. [Edge Cases](#9-edge-cases)
10. [Implementation Steps](#10-implementation-steps)
11. [Risk & Mitigation](#11-risk--mitigation)

---

## 1. Current State Analysis

### 1.1 HTML — Tab Navigation (`src/index.html`)

The three tab buttons live inside `<nav id="tabs">`:

```html
<nav id="tabs">
  <button class="tab active" data-tab="issues">Issues</button>
  <button class="tab" data-tab="pulls">Pull Requests</button>
  <button class="tab" data-tab="alerts">Security Alerts</button>
</nav>
```

The buttons carry **no child elements** — they are plain text only. There is no `position: relative` on the `.tab` rule, which is required to host an absolutely-positioned badge overlay.

### 1.2 CSS — Existing `.tab` Rule (`src/styles.css`)

```css
#tabs {
  display: flex;
  gap: 0;
  border-bottom: 1px solid var(--border);
  background: var(--surface);
}
.tab {
  padding: 0.65rem 1.2rem;
  background: none;
  border: none;
  color: var(--text-muted);
  cursor: pointer;
  font-size: 0.9rem;
  border-bottom: 2px solid transparent;
  transition: color 0.15s, border-color 0.15s;
}
.tab:hover { color: var(--text); }
.tab.active { color: var(--text); border-bottom-color: var(--accent); }
```

**Gap:** No `position: relative` — must be added to anchor absolute badge children.

**Important naming collision:** The codebase already uses a `.badge` class for inline state pills (open/closed/merged/labels) in data tables. The new tab badge class **must use a different name** — `tab-badge` is chosen throughout this spec.

### 1.3 JavaScript — Data Flow (`src/main.js`)

The central data-fetching function is `refreshData()`:

```
selectTrackedRepo(repo)
  → refreshData()
    → Promise.allSettled([
        invoke("fetch_issues", ...),
        invoke("fetch_pulls", ...),
        invoke("fetch_security_alerts", ...)
      ])
    → issues / pulls / alerts arrays are populated (or emptied on error)
    → renderIssues() / renderPulls() / renderAlerts()
```

**Key insight:** After `refreshData()` completes, `issues.length`, `pulls.length`, and `alerts.length` already represent the exact counts to display. No additional Tauri IPC calls are needed.

State variables relevant to badges:
| Variable | Type | Meaning |
|---|---|---|
| `issues` | `Array` | Loaded issues (may be empty on error) |
| `pulls` | `Array` | Loaded pull requests |
| `alerts` | `Array` | Loaded security alerts |
| `issuesError` | `string \| null` | Non-null if `fetch_issues` failed |
| `pullsError` | `string \| null` | Non-null if `fetch_pulls` failed |
| `alertsError` | `string \| null` | Non-null if `fetch_security_alerts` failed |
| `selectedRepo` | `{owner, name} \| null` | Currently selected repo |

### 1.4 Rust Backend (`src-tauri/src/`)

Existing Tauri commands:
- `fetch_issues` → `Vec<models::Issue>`
- `fetch_pulls` → `Vec<models::PullRequest>`
- `fetch_security_alerts` → `Vec<models::SecurityAlert>`

The `models::Repo` struct already includes `open_issues_count: u32` from the GitHub API but this pre-count is unreliable (includes PRs, may be stale). **Live counts from the already-fetched data arrays are preferred.**

**Conclusion: No new Rust commands are required.**

---

## 2. Research Findings

### Source 1 — MDN: `position: absolute` inside `position: relative` containers
*https://developer.mozilla.org/en-US/docs/Web/CSS/position*

The canonical pattern for overlaying a badge on a button is `position: relative` on the container and `position: absolute` with `top`/`right` offsets on the badge child. This creates a stacking context scoped to the button, ensuring the badge does not overflow into adjacent elements.

### Source 2 — Bootstrap 5 Badge Component
*https://getbootstrap.com/docs/5.3/components/badge/*

Bootstrap's notification badge pattern:
```html
<button type="button" class="btn btn-primary">
  Messages <span class="badge text-bg-secondary">4</span>
  <span class="visually-hidden">unread messages</span>
</button>
```
Key findings:
- `min-width` (not `width`) allows the badge to expand for 2–3 digit numbers without clipping.
- Border-radius of `50%` works for single-digit counts; `border-radius: 10px` (pill) is better for multi-digit values.
- Visually-hidden span provides screen reader context that the visual number alone cannot convey.

### Source 3 — WAI-ARIA Authoring Practices — Accessible Notification Count Patterns
*https://www.w3.org/WAI/ARIA/apg/*

Best practices for numeric badges on interactive elements:
- Badge spans should carry `aria-hidden="true"` to prevent double-reading (visible text + badge number).
- The containing button should have its accessible name updated to include the count, either via `aria-label` or a visually-hidden supplementary span.
- Example: button `aria-label="Issues, 5 items"` when badge shows 5.
- When badge is hidden (count = 0), remove the `aria-label` so the button's visible text serves as the accessible name.

### Source 4 — Vanilla JS DOM Update Patterns for Dynamic Badges
*https://developer.mozilla.org/en-US/docs/Web/API/Element/textContent*

For performance in vanilla JS:
- `element.textContent = count` is faster than `innerHTML` for numeric text (no HTML parsing overhead).
- Toggling `.hidden` class (already used throughout this codebase) via `classList.add/remove` is the correct pattern vs. inline `style.display` manipulation.
- Batch DOM reads before DOM writes to avoid forced reflows (read count from array, then write to badge span).

### Source 5 — Tauri IPC Patterns for Frontend Data Access
*https://tauri.app/v1/guides/features/command/*

Tauri v1 IPC: `window.__TAURI__.tauri.invoke()` is async. The codebase already follows the pattern of:
1. Invoking commands in `Promise.allSettled` for parallel fetching.
2. Populating module-level arrays (`issues`, `pulls`, `alerts`).
3. Rendering from those arrays synchronously.

Since badge counts are derived from already-resolved JS arrays—not from new Rust commands—badge updates are **synchronous**, requiring no additional `await` or IPC calls. This is the optimal performance pattern.

### Source 6 — CSS `min-width` for Badge Sizing
*https://developer.mozilla.org/en-US/docs/Web/CSS/min-width*

Using `min-width` instead of `width` on the badge element:
- `min-width: 18px` ensures the badge is a circle for single digits.
- For "99+" (3 chars), the badge naturally expands horizontally due to `min-width` + padding, without requiring explicit width overrides.
- `height: 18px` + `line-height: 18px` + `border-radius: 9px` creates a pill shape that scales with content.
- `box-sizing: border-box` ensures padding is included in the `min-width` measurement.

---

## 3. Proposed Badge Design

### 3.1 Visual Design

Each tab button will host a small circular/pill badge in its top-right corner showing the live count of items fetched.

```
 ┌─────────────────────────────────────────┐
 │  Issues [5]  │  Pull Requests [2]  │  Security Alerts [12]  │
 └─────────────────────────────────────────┘
```

Where `[N]` is a blue (accent-colored) rounded pill, positioned absolutely in the top-right of the tab button, overlapping the button edge slightly.

Color: `var(--accent)` (`#58a6ff`) — matches the active tab indicator and overall accent color. This is semantically appropriate (informational) and consistent with the design language.

### 3.2 Badge Span Structure

```html
<span class="tab-badge hidden" id="badge-issues" aria-hidden="true"></span>
```

- `class="tab-badge hidden"` — starts hidden; `hidden` is the existing utility class (`display: none !important`).
- `id="badge-issues"` — for direct DOM access in JS (IDs: `badge-issues`, `badge-pulls`, `badge-alerts`).
- `aria-hidden="true"` — always set; the containing button's `aria-label` conveys the count to screen readers.
- **No initial text content** — JS populates `textContent` before removing `hidden`.

---

## 4. CSS Implementation

### 4.1 Modify Existing `.tab` Rule

Add `position: relative` to the existing `.tab` rule in `src/styles.css`.

**Current:**
```css
.tab {
  padding: 0.65rem 1.2rem;
  background: none;
  border: none;
  color: var(--text-muted);
  cursor: pointer;
  font-size: 0.9rem;
  border-bottom: 2px solid transparent;
  transition: color 0.15s, border-color 0.15s;
}
```

**Modified:**
```css
.tab {
  position: relative;          /* ADD — anchors .tab-badge children */
  padding: 0.65rem 1.2rem;
  background: none;
  border: none;
  color: var(--text-muted);
  cursor: pointer;
  font-size: 0.9rem;
  border-bottom: 2px solid transparent;
  transition: color 0.15s, border-color 0.15s;
}
```

### 4.2 New `.tab-badge` Rule

Add the following new rule block **immediately after** the `.tab.active` rule in the Tabs section of `src/styles.css`:

```css
/* Tab count badges */
.tab-badge {
  position: absolute;
  top: 4px;
  right: 4px;
  min-width: 18px;
  height: 18px;
  padding: 0 5px;
  background: var(--accent);
  color: #fff;
  border-radius: 9px;
  font-size: 0.65rem;
  font-weight: 700;
  line-height: 18px;
  text-align: center;
  pointer-events: none;
  box-sizing: border-box;
  white-space: nowrap;
}
```

**Property rationale:**
| Property | Value | Reason |
|---|---|---|
| `position: absolute` | — | Overlays badge on top-right of button |
| `top: 4px; right: 4px` | — | Inset from corner so it stays within button bounds; avoids clipping |
| `min-width: 18px` | — | Circle for 1 digit; pill expands for "99+" |
| `height: 18px` | — | Fixed height; `line-height` centers text vertically |
| `padding: 0 5px` | — | Horizontal breathing room for multi-digit numbers |
| `background: var(--accent)` | `#58a6ff` | Consistent with design accent color |
| `color: #fff` | — | High contrast against blue background |
| `border-radius: 9px` | — | `height/2` = perfect pill/circle shape |
| `font-size: 0.65rem` | — | Small enough to not crowd the label text |
| `font-weight: 700` | — | Bold for readability at small size |
| `line-height: 18px` | — | Matches height to vertically center text |
| `pointer-events: none` | — | Badge must not capture click events meant for the tab button |
| `box-sizing: border-box` | — | Ensures `min-width` accounts for padding |
| `white-space: nowrap` | — | Prevents "99+" wrapping to two lines |

---

## 5. HTML Implementation

### 5.1 Modified Tab Nav in `src/index.html`

Replace the existing `<nav id="tabs">` block with:

```html
<nav id="tabs">
  <button class="tab active" data-tab="issues">
    Issues
    <span class="tab-badge hidden" id="badge-issues" aria-hidden="true"></span>
  </button>
  <button class="tab" data-tab="pulls">
    Pull Requests
    <span class="tab-badge hidden" id="badge-pulls" aria-hidden="true"></span>
  </button>
  <button class="tab" data-tab="alerts">
    Security Alerts
    <span class="tab-badge hidden" id="badge-alerts" aria-hidden="true"></span>
  </button>
</nav>
```

**Notes:**
- Badge spans are placed after the text node inside the button, not wrapping it. This keeps the text rendering unaffected.
- `aria-hidden="true"` is set statically in HTML — it should never be removed. The accessible count is provided via the button's `aria-label` (managed by JS).
- The `.hidden` class matches the existing CSS utility (`.hidden { display: none !important; }`).

---

## 6. JavaScript Implementation

### 6.1 Helper Functions

Add two new helper functions to `src/main.js`, placed in the **"Utils"** section (just after the `esc()` and `renderMarkdown()` functions, around the area marked `// ── Utils ─`):

```js
// ── Tab badge helpers ───────────────────────────
/**
 * Update a single tab badge.
 * @param {string} tabId   - data-tab value: "issues", "pulls", or "alerts"
 * @param {number} count   - number of loaded items
 * @param {boolean} hasError - true if the corresponding fetch failed
 */
function updateTabBadge(tabId, count, hasError) {
  const badge = document.getElementById(`badge-${tabId}`);
  const btn   = document.querySelector(`.tab[data-tab="${tabId}"]`);
  if (!badge || !btn) return;

  if (hasError || count === 0) {
    badge.classList.add('hidden');
    btn.removeAttribute('aria-label');
    return;
  }

  const displayCount = count > 99 ? '99+' : String(count);
  badge.textContent = displayCount;
  badge.classList.remove('hidden');
  btn.setAttribute('aria-label', `${btn.textContent.trim().replace(displayCount, '')} ${displayCount} items`);
}

/** Update all three tab badges from current module-level state. */
function updateTabBadges() {
  updateTabBadge('issues', issues.length, !!issuesError);
  updateTabBadge('pulls',  pulls.length,  !!pullsError);
  updateTabBadge('alerts', alerts.length, !!alertsError);
}

/** Hide all tab badges (e.g., when no repo is selected or account switches). */
function clearTabBadges() {
  ['issues', 'pulls', 'alerts'].forEach((tabId) => {
    const badge = document.getElementById(`badge-${tabId}`);
    const btn   = document.querySelector(`.tab[data-tab="${tabId}"]`);
    if (badge) badge.classList.add('hidden');
    if (btn)   btn.removeAttribute('aria-label');
  });
}
```

**`aria-label` construction note:** The `btn.textContent.trim().replace(displayCount, '')` is evaluated *after* `badge.textContent = displayCount` has set the badge text, which is a child of the button. Therefore `btn.textContent.trim()` would include the badge text. The label computation should be based on the static tab name, not the live button text. Use a static mapping instead:

```js
const TAB_LABELS = { issues: 'Issues', pulls: 'Pull Requests', alerts: 'Security Alerts' };

function updateTabBadge(tabId, count, hasError) {
  const badge = document.getElementById(`badge-${tabId}`);
  const btn   = document.querySelector(`.tab[data-tab="${tabId}"]`);
  if (!badge || !btn) return;

  if (hasError || count === 0) {
    badge.classList.add('hidden');
    btn.removeAttribute('aria-label');
    return;
  }

  const displayCount = count > 99 ? '99+' : String(count);
  badge.textContent = displayCount;
  badge.classList.remove('hidden');
  btn.setAttribute('aria-label', `${TAB_LABELS[tabId]}, ${displayCount} items`);
}
```

### 6.2 Call `updateTabBadges()` from `refreshData()`

In the existing `refreshData()` function, after the three `render*()` calls, add `updateTabBadges()`:

**Current (end of refreshData):**
```js
  renderIssues();
  renderPulls();
  renderAlerts();

  expandedRow = null; // Reset expanded state on data refresh

  loading.classList.add("hidden");
  exportCsv.disabled = false;
  exportPdf.disabled = false;
}
```

**Modified:**
```js
  renderIssues();
  renderPulls();
  renderAlerts();
  updateTabBadges();          // ADD — update badge counts after renders

  expandedRow = null; // Reset expanded state on data refresh

  loading.classList.add("hidden");
  exportCsv.disabled = false;
  exportPdf.disabled = false;
}
```

### 6.3 Clear Badges on Repo Deselect

In `handleRemoveTrackedRepo()`, after the block that sets `selectedRepo = null`, add `clearTabBadges()`:

**Current:**
```js
    if (selectedRepo && `${selectedRepo.owner}/${selectedRepo.name}` === fullName) {
      selectedRepo = null;
      issues = [];
      pulls = [];
      alerts = [];
      placeholder.classList.remove("hidden");
    }
    renderTrackedRepoList(trackedRepos);
```

**Modified:**
```js
    if (selectedRepo && `${selectedRepo.owner}/${selectedRepo.name}` === fullName) {
      selectedRepo = null;
      issues = [];
      pulls = [];
      alerts = [];
      placeholder.classList.remove("hidden");
      clearTabBadges();        // ADD — clear badges when repo is removed
    }
    renderTrackedRepoList(trackedRepos);
```

### 6.4 Clear Badges on Account Switch

In `handleSwitchAccount()`, after the block that resets data arrays, add `clearTabBadges()`:

**Current (in handleSwitchAccount):**
```js
    selectedRepo = null;
    repos = [];
    issues = [];
    pulls = [];
    alerts = [];
    pickerLoaded = false;
    allRepos = [];
    repoList.innerHTML = "";
    placeholder.classList.remove("hidden");
    await loadTrackedRepos();
```

**Modified:**
```js
    selectedRepo = null;
    repos = [];
    issues = [];
    pulls = [];
    alerts = [];
    pickerLoaded = false;
    allRepos = [];
    repoList.innerHTML = "";
    placeholder.classList.remove("hidden");
    clearTabBadges();          // ADD — clear badges on account switch
    await loadTrackedRepos();
```

### 6.5 Clear Badges on Logout

In the `logoutBtn` event listener, after resetting state, add `clearTabBadges()`:

**Current:**
```js
logoutBtn.addEventListener("click", async () => {
  await invoke("logout");
  accounts = [];
  activeAccountId = null;
  loginScreen.classList.remove("hidden");
  appScreen.classList.add("hidden");
  ...
  loginError.classList.add('hidden');
});
```

**Modified — add `clearTabBadges()` before `appScreen.classList.add("hidden")`:**
```js
logoutBtn.addEventListener("click", async () => {
  await invoke("logout");
  accounts = [];
  activeAccountId = null;
  clearTabBadges();            // ADD — clear badges on logout
  loginScreen.classList.remove("hidden");
  appScreen.classList.add("hidden");
  ...
  loginError.classList.add('hidden');
});
```

### 6.6 TAB_LABELS Constant Placement

The `TAB_LABELS` constant should be defined in the **State** section at the top of `main.js`, after the other state variables:

```js
// ── Tab badge label map (used for aria-label generation) ──────
const TAB_LABELS = {
  issues: 'Issues',
  pulls:  'Pull Requests',
  alerts: 'Security Alerts',
};
```

---

## 7. Rust Backend Changes

**No Rust backend changes are required.**

The badge counts derive entirely from the lengths of the JS arrays that are already populated by the existing three Tauri commands (`fetch_issues`, `fetch_pulls`, `fetch_security_alerts`). Adding new Rust commands solely to return counts would be redundant and would introduce an extra round-trip IPC call with no benefit.

The `open_issues_count` field on the `Repo` model is **not** used for badge counts because:
1. It includes pull requests (GitHub's REST API conflates them).
2. It reflects GitHub's cached state, not the currently-applied filter (state, search, etc.).
3. The live fetched arrays already represent the filtered set the user sees.

---

## 8. Accessibility

### 8.1 Pattern

- Badge `<span>` elements carry `aria-hidden="true"` (set statically in HTML, never removed).
- When a badge is visible (count > 0, no error), the containing `<button>` receives a dynamic `aria-label` such as `"Issues, 5 items"`.
- When the badge is hidden (count = 0 or error), the `aria-label` attribute is removed from the button, restoring the visible text as the accessible name.

### 8.2 aria-label Examples

| State | Badge visible | Button `aria-label` |
|---|---|---|
| 5 issues loaded | Yes, shows "5" | `"Issues, 5 items"` |
| 0 issues loaded | No | *(attribute absent — "Issues" from visible text)* |
| 100+ issues loaded | Yes, shows "99+" | `"Issues, 99+ items"` |
| Issues fetch failed | No | *(attribute absent)* |

### 8.3 Why Not `role="status"` on the Badge

Using `role="status"` would cause screen readers to announce the count every time it changes (after every `refreshData()` call). This would be disruptive — announcing "5" and "3" on rapid filter changes. The count is supplemental UI information, not a live notification requiring immediate announcement. Letting the button's `aria-label` communicate it passively is the correct approach.

### 8.4 Color Contrast

Badge: white (`#fff`) text on `var(--accent)` (`#58a6ff`) background.
Contrast ratio: approximately **3.5:1** against `#58a6ff`.
This is below WCAG AA (4.5:1) for normal text. However, at `0.65rem` with `font-weight: 700`, this is considered "large text" by some tools, where the threshold is 3:1. To improve contrast, optionally use a darker blue:
- `#1f6feb` (GitHub's darker accent) gives approximately **5.5:1** against white — WCAG AA compliant.
- Recommendation: use `background: #1f6feb` for the badge instead of `var(--accent)`.

**Spec decision:** Use `background: #1f6feb` (a darker, accessible blue) for the badge background.

---

## 9. Edge Cases

### 9.1 Count = 0

**Behavior:** Badge is hidden (`.hidden` class applied). The tab button reverts to its default `aria-label`-less state.

**Rationale:** Showing a "0" badge would add unnecessary noise to the UI and could be misread as an error indicator. An empty badge is semantically equivalent to "nothing to show."

### 9.2 Count > 99

**Behavior:** Badge text is set to `"99+"` instead of the numeric value.

**Rationale:** Counts above 99 would require a wider badge, potentially crowding the tab label. "99+" is the industry-standard truncation (used by GitHub, Slack, Gmail, etc.). Users understand this means "many."

**Implementation:**
```js
const displayCount = count > 99 ? '99+' : String(count);
```

### 9.3 Loading State (During `refreshData()`)

**Behavior:** Badges remain in their previous state during loading. They are **not** cleared while the spinner is showing. They update when `refreshData()` resolves.

**Rationale:** Clearing badges during load would cause visual flicker (badge disappears then reappears). Keeping the previous values stable during load is the less-disruptive choice. The loading spinner already communicates that a refresh is in progress.

**Alternative considered:** Show a loading indicator inside the badge (e.g., replace count with `…`). Rejected due to added complexity and the fact that the "Loading…" state is already clearly signaled by `#loading` element.

### 9.4 Fetch Error

**Behavior:** When a fetch returns an error (e.g., `issuesError` is non-null), the corresponding badge is hidden (same as count = 0). Error state is communicated via the table area (existing `fetch-error` styling), not the tab badge.

**Rationale:** Showing an error badge would require a distinct visual (e.g., red badge), adding complexity. The error message in the table panel is sufficient.

### 9.5 No Repository Selected

**Behavior:** All badges are hidden. `clearTabBadges()` is called at logout, account switch, and repo removal.

**Initial state:** All badges start hidden via the `hidden` class in HTML.

### 9.6 Very Fast Filter Changes (Debouncing)

The `searchInput` listener already debounces with `setTimeout(refreshData, 400)`. This means badge update is also debounced — the badge will not flicker on every keystroke. No additional debouncing is needed for badges.

### 9.7 Mock/Dev Mode

The badge update logic lives entirely in JS and is indifferent to whether data came from the real GitHub API or the mock backend. Mock data arrays (`issues`, `pulls`, `alerts`) will set badge counts just as real data would — no special handling needed.

---

## 10. Implementation Steps

All changes are to frontend files only. No Rust or build configuration changes.

### Step 1: Modify `src/styles.css`

**Edit 1a** — Add `position: relative` to the `.tab` rule:

Find:
```css
.tab {
  padding: 0.65rem 1.2rem;
  background: none;
  border: none;
  color: var(--text-muted);
  cursor: pointer;
  font-size: 0.9rem;
  border-bottom: 2px solid transparent;
  transition: color 0.15s, border-color 0.15s;
}
```

Replace with:
```css
.tab {
  position: relative;
  padding: 0.65rem 1.2rem;
  background: none;
  border: none;
  color: var(--text-muted);
  cursor: pointer;
  font-size: 0.9rem;
  border-bottom: 2px solid transparent;
  transition: color 0.15s, border-color 0.15s;
}
```

**Edit 1b** — Add new `.tab-badge` rule block immediately after `.tab.active { ... }`:

```css
/* Tab count badges */
.tab-badge {
  position: absolute;
  top: 4px;
  right: 4px;
  min-width: 18px;
  height: 18px;
  padding: 0 5px;
  background: #1f6feb;
  color: #fff;
  border-radius: 9px;
  font-size: 0.65rem;
  font-weight: 700;
  line-height: 18px;
  text-align: center;
  pointer-events: none;
  box-sizing: border-box;
  white-space: nowrap;
}
```

### Step 2: Modify `src/index.html`

Replace the existing `<nav id="tabs">` block (lines containing the three `.tab` buttons):

```html
<nav id="tabs">
  <button class="tab active" data-tab="issues">
    Issues
    <span class="tab-badge hidden" id="badge-issues" aria-hidden="true"></span>
  </button>
  <button class="tab" data-tab="pulls">
    Pull Requests
    <span class="tab-badge hidden" id="badge-pulls" aria-hidden="true"></span>
  </button>
  <button class="tab" data-tab="alerts">
    Security Alerts
    <span class="tab-badge hidden" id="badge-alerts" aria-hidden="true"></span>
  </button>
</nav>
```

### Step 3: Modify `src/main.js` — Add constant and helper functions

**Edit 3a** — Add `TAB_LABELS` constant in the State section, after the existing state variable declarations (after `let pickerLoaded = false;`):

```js
// ── Tab badge label map (for aria-label generation) ──────
const TAB_LABELS = {
  issues: 'Issues',
  pulls:  'Pull Requests',
  alerts: 'Security Alerts',
};
```

**Edit 3b** — Add `updateTabBadge`, `updateTabBadges`, and `clearTabBadges` helper functions in the Utils section, after the `esc()` function:

```js
// ── Tab badge helpers ───────────────────────────
function updateTabBadge(tabId, count, hasError) {
  const badge = document.getElementById(`badge-${tabId}`);
  const btn   = document.querySelector(`.tab[data-tab="${tabId}"]`);
  if (!badge || !btn) return;

  if (hasError || count === 0) {
    badge.classList.add('hidden');
    btn.removeAttribute('aria-label');
    return;
  }

  const displayCount = count > 99 ? '99+' : String(count);
  badge.textContent = displayCount;
  badge.classList.remove('hidden');
  btn.setAttribute('aria-label', `${TAB_LABELS[tabId]}, ${displayCount} items`);
}

function updateTabBadges() {
  updateTabBadge('issues', issues.length, !!issuesError);
  updateTabBadge('pulls',  pulls.length,  !!pullsError);
  updateTabBadge('alerts', alerts.length, !!alertsError);
}

function clearTabBadges() {
  ['issues', 'pulls', 'alerts'].forEach((tabId) => {
    const badge = document.getElementById(`badge-${tabId}`);
    const btn   = document.querySelector(`.tab[data-tab="${tabId}"]`);
    if (badge) badge.classList.add('hidden');
    if (btn)   btn.removeAttribute('aria-label');
  });
}
```

**Edit 3c** — Call `updateTabBadges()` inside `refreshData()` after the three `render*()` calls:

```js
  renderIssues();
  renderPulls();
  renderAlerts();
  updateTabBadges();          // update badge counts

  expandedRow = null;
```

**Edit 3d** — Call `clearTabBadges()` inside `handleRemoveTrackedRepo()` when `selectedRepo` becomes null:

```js
      selectedRepo = null;
      issues = [];
      pulls = [];
      alerts = [];
      placeholder.classList.remove("hidden");
      clearTabBadges();
```

**Edit 3e** — Call `clearTabBadges()` inside `handleSwitchAccount()` after resetting data arrays:

```js
    placeholder.classList.remove("hidden");
    clearTabBadges();
    await loadTrackedRepos();
```

**Edit 3f** — Call `clearTabBadges()` inside the `logoutBtn` event listener:

```js
logoutBtn.addEventListener("click", async () => {
  await invoke("logout");
  accounts = [];
  activeAccountId = null;
  clearTabBadges();
  loginScreen.classList.remove("hidden");
  appScreen.classList.add("hidden");
```

---

## 11. Risk & Mitigation

| Risk | Likelihood | Impact | Mitigation |
|---|---|---|---|
| `.badge` CSS class name collision | Medium | High | Spec explicitly uses `.tab-badge` — distinct from existing `.badge` class used in tables |
| `btn.textContent.trim()` returns badge text concat | Low | Medium | Use static `TAB_LABELS` map instead of reading from DOM |
| Badge overflows tab button boundary | Low | Low | Use `top: 4px; right: 4px` inset (not negative offset); `overflow: visible` on `#tabs` by default |
| Count from filtered array vs. total count confusion | Low | Low | Badge shows filtered count (matching current filter state — consistent with what user sees in table) |
| Badge not hidden on initial load | None | Low | HTML sets `class="tab-badge hidden"` statically — badges are always hidden until `updateTabBadges()` fires |
| Mock build compatibility | None | None | Badge logic is pure JS, indifferent to mock vs. real data source |
| Screen reader double-reading (badge + button text) | Medium | Medium | `aria-hidden="true"` on badge span + dynamic `aria-label` on button prevents double-reading |

---

## Files to Modify

| File | Change Type |
|---|---|
| `src/index.html` | Edit — add badge `<span>` elements inside three `.tab` buttons |
| `src/styles.css` | Edit — add `position: relative` to `.tab`; add new `.tab-badge` rule |
| `src/main.js` | Edit — add `TAB_LABELS` const, three badge helper functions, and four call-sites |

**Rust files: no changes.**

---

*End of Specification*
