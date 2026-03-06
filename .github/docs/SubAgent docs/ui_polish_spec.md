# UI Polish Spec — Account Dropdown & Add Repository Button

**Date:** 2026-03-05  
**Scope:** CSS-only redesign (with one minimal, justified HTML structural change)  
**Files to modify:** `src/styles.css`, `src/index.html`  
**JS impact:** None — all JS-driven class changes are preserved exactly as-is

---

## 1. Current HTML Structure

### 1.1 Sidebar Header & Account Chip (trigger)

Location: `src/index.html`, inside `<aside id="sidebar">`

```html
<div class="sidebar-header">

  <!-- Active account chip — the visible trigger row -->
  <div id="account-chip" class="account-chip">
    <span id="username" class="account-username"></span>
    <button id="account-menu-btn" class="account-menu-btn"
            title="Manage accounts"
            aria-haspopup="true"
            aria-expanded="false">▾</button>
  </div>

  <!-- Account dropdown panel — hidden by default, toggled by JS -->
  <div id="account-menu" class="account-menu hidden" role="menu">
    <div class="account-menu-section-label">Switch account</div>
    <ul id="account-list" class="account-switcher-list"></ul>
    <div class="account-menu-divider"></div>
    <button id="add-account-btn"    class="account-menu-action" role="menuitem">+ Add account</button>
    <button id="remove-account-btn" class="account-menu-action account-menu-danger" role="menuitem">Remove this account</button>
    <div class="account-menu-divider"></div>
    <button id="logout-btn" class="account-menu-action" role="menuitem"
            title="Disconnect (keep account saved)">Disconnect</button>
  </div>

</div>
```

**Dynamically-rendered account list items** (injected by `renderAccountSwitcher()` in `main.js`):

```html
<!-- Non-active account -->
<li class="account-switcher-item" role="menuitem">
  <span class="account-item-username">@username</span>
  <span class="account-item-label">display label</span>
</li>

<!-- Active account -->
<li class="account-switcher-item account-active" role="menuitem">
  <span class="account-item-username">@username</span>
  <span class="account-item-label">display label</span>
  <span class="account-active-dot" aria-label="Active">●</span>
</li>
```

### 1.2 Add Repository Button

Location: `src/index.html`, directly below `<h3>Repositories</h3>` in the sidebar

```html
<button id="add-repo-btn" class="btn-add-repo">+ Add Repository</button>
```

---

## 2. JS-Driven Class Changes (must not be broken by CSS)

All relevant JS behaviour lives in `src/main.js`. The following class/attribute mutations must remain functional:

| Element | JS action | Trigger |
|---|---|---|
| `#account-menu` | `.hidden` toggled via `classList.toggle("hidden", isOpen)` | Click on `#account-menu-btn` |
| `#account-menu` | `.hidden` added via `classList.add("hidden")` | Click anywhere on `document` |
| `#account-menu-btn` | `aria-expanded` set to `"true"` / `"false"` | Same click handlers |
| `#account-menu-btn` | `aria-expanded` set to `"false"` | Global `document` click close |
| `li.account-switcher-item` | `.account-active` class present if account `is_active` | Set in `renderAccountSwitcher()` |
| `li.account-switcher-item` | No `click` listener if `.account-active` | Set in `renderAccountSwitcher()` |

**Critical constraint:** `.hidden { display: none !important; }` is defined globally. The dropdown show/hide mechanism relies entirely on this utility class — do not add CSS `display` rules to `.account-menu` or `#account-menu` that would conflict.

---

## 3. Current CSS — Verbatim Extracts

### 3.1 CSS Custom Properties (`:root`)

```css
:root {
  --bg:        #0d1117;
  --surface:   #161b22;
  --border:    #30363d;
  --text:      #c9d1d9;
  --text-muted:#8b949e;
  --accent:    #58a6ff;
  --green:     #3fb950;
  --red:       #f85149;
  --orange:    #d29922;
  --purple:    #bc8cff;
  --radius:    6px;
  --font:      -apple-system, BlinkMacSystemFont, "Segoe UI", Helvetica, Arial, sans-serif;
}
```

Additional hardcoded values seen in the file (not in `:root`):
- `#21262d` — slightly elevated surface (used in `.btn-copy`, `.btn-oauth-secondary`)
- `#292e36` — not used yet; logical hover step between `#21262d` and `#30363d`
- `#e6edf3` — bright text (used in login card elements)
- `rgba(88,166,255,0.06)` — accent table row hover tint
- `rgba(88,166,255,0.08)` — slightly stronger accent row hover
- `rgba(88,166,255,0.12)` — selected/expanded row
- Border-radius values in use: `6px` (var(--radius)), `8px`, `10px`, `12px`, `20px`

### 3.2 Sidebar Header (first block — general layout)

```css
.sidebar-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 1rem;
}
.sidebar-header span { font-weight: 600; }
.sidebar-header button {
  background: none;
  border: 1px solid var(--border);
  color: var(--text-muted);
  border-radius: var(--radius);
  padding: 0.25rem 0.5rem;
  cursor: pointer;
}
```

### 3.3 Sidebar Header (second block — positioning context for dropdown)

```css
/* ── Multi-account switcher ──────────────────── */
.sidebar-header {
  position: relative;
}
```

### 3.4 Account Chip & Menu Button

```css
.account-chip {
  display: flex;
  align-items: center;
  gap: 6px;
  width: 100%;
}

.account-username {
  font-weight: 600;
  flex: 1;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.account-menu-btn {
  background: none;
  border: none;
  cursor: pointer;
  color: var(--text-muted);
  font-size: 0.9rem;
  padding: 2px 4px;
  border-radius: var(--radius);
  flex-shrink: 0;
  transition: color 0.15s;
}

.account-menu-btn:hover { color: var(--text); }
```

### 3.5 Account Dropdown Panel

```css
.account-menu {
  position: absolute;
  top: calc(100% + 4px);
  left: 0;
  right: 0;
  background: var(--surface);
  border: 1px solid var(--border);
  border-radius: var(--radius);
  z-index: 100;
  min-width: 200px;
  box-shadow: 0 4px 12px rgba(0, 0, 0, 0.4);
}

.account-menu-section-label {
  font-size: 11px;
  color: var(--text-muted);
  padding: 8px 12px 4px;
  text-transform: uppercase;
  letter-spacing: 0.05em;
}

.account-switcher-list {
  list-style: none;
  margin: 0;
  padding: 0;
}

.account-switcher-item {
  padding: 8px 12px;
  cursor: pointer;
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 0.85rem;
}

.account-switcher-item:hover { background: rgba(255, 255, 255, 0.06); }

.account-active { font-weight: 600; cursor: default; }

.account-active-dot {
  color: var(--green);
  font-size: 10px;
  margin-left: auto;
}

.account-item-label {
  font-size: 11px;
  color: var(--text-muted);
}

.account-menu-divider {
  height: 1px;
  background: var(--border);
  margin: 4px 0;
}

.account-menu-action {
  display: block;
  width: 100%;
  text-align: left;
  padding: 8px 12px;
  background: none;
  border: none;
  cursor: pointer;
  color: var(--text);
  font-size: 0.85rem;
}

.account-menu-action:hover { background: rgba(255, 255, 255, 0.06); }
.account-menu-danger { color: var(--red); }
```

### 3.6 Add Repository Button

```css
/* ── Add Repository button ───────────────────── */
.btn-add-repo {
  display: block;
  width: 100%;
  padding: 0.4rem 0.6rem;
  margin-bottom: 0.5rem;
  background: transparent;
  border: 1px dashed var(--border);
  border-radius: var(--radius);
  color: var(--accent);
  font-size: 0.85rem;
  text-align: left;
  cursor: pointer;
  transition: background 0.15s, border-color 0.15s;
}

.btn-add-repo:hover {
  background: rgba(88, 166, 255, 0.08);
  border-color: var(--accent);
}
```

---

## 4. Problems with the Current Design

| Element | Problem |
|---|---|
| Account chip | No visual affordance — looks like plain text + a stray `▾` character. No background, no border, no "button" shape. |
| Account chip | The `▾` is a Unicode text character, not a crisp icon, and lacks padding/alignment. |
| Account chip | The `.account-menu-btn` has a tiny hit target (2px padding). |
| Account dropdown | `border-radius: var(--radius)` (6px) — too rectangular for a polished context menu. |
| Account dropdown | Shadow is weak (`0 4px 12px rgba(0,0,0,0.4)`) — doesn't lift the panel visually. |
| Account dropdown | No open/close animation — dropdown appears/disappears abruptly. |
| Account switcher items | Hover uses `rgba(255,255,255,0.06)` — barely perceptible. |
| Account menu actions | Same invisible hover as list items. |
| `.account-active-dot` | `●` Unicode bullet — visually inconsistent with the rest of the UI. |
| Add Repo button | Dashed border feels unfinished / developer-placeholder-like. |
| Add Repo button | `text-align: left` with `+ Add Repository` as plain text — not visually aligned or icon-forward. |
| Add Repo button | Hover only changes background to a very faint blue — not enough contrast feedback. |

---

## 5. Proposed Redesign

### 5.1 HTML Changes Required

Only **one minimal HTML change** is required: replace the `▾` text content of `#account-menu-btn` with an inline SVG chevron for a crisper icon. This does not affect JS behaviour in any way.

**Current:**
```html
<button id="account-menu-btn" class="account-menu-btn" title="Manage accounts" aria-haspopup="true" aria-expanded="false">▾</button>
```

**Proposed:**
```html
<button id="account-menu-btn" class="account-menu-btn" title="Manage accounts" aria-haspopup="true" aria-expanded="false">
  <svg class="chevron-icon" width="12" height="12" viewBox="0 0 12 12" fill="currentColor" aria-hidden="true">
    <path d="M2.22 4.47a.75.75 0 0 1 1.06 0L6 7.19l2.72-2.72a.75.75 0 1 1 1.06 1.06L6.53 8.78a.75.75 0 0 1-1.06 0L2.22 5.53a.75.75 0 0 1 0-1.06Z"/>
  </svg>
</button>
```

All other HTML is unchanged.

Also add `.chevron-icon` class to CSS (new rule, no conflicts).

---

### 5.2 Proposed CSS — Full Replacement Rules

The following rules replace the existing CSS blocks listed in Section 3. Every other rule in `styles.css` is untouched.

> **Note:** Where a rule block appears twice in the current file (the two `.sidebar-header` blocks), the second block (`position: relative`) must remain — it provides the positioning context for the absolutely-placed dropdown panel. Only the first block's `.sidebar-header button` sub-rule is removed (it used to style the old logout button that no longer exists at that location).

---

#### 5.2.1 Account Chip — Polished Trigger Row

**Design intent:** The chip should look like a compact selectable row — similar to GitHub Desktop's account badge. It has a visible background, subtle border, rounded shape, a username on the left (visually truncated) and a chevron badge on the right. The entire chip area conveys interactivity via cursor and hover states.

```css
/* ── Account chip (trigger) ─────────────────── */
.account-chip {
  display: flex;
  align-items: center;
  gap: 0;
  width: 100%;
  background: #21262d;
  border: 1px solid var(--border);
  border-radius: 8px;
  padding: 5px 6px 5px 10px;
  cursor: pointer;
  transition: background 0.15s, border-color 0.15s;
  position: relative;
}

.account-chip:has(.account-menu-btn:hover),
.account-chip:has(.account-menu-btn:focus-visible) {
  background: #292e36;
  border-color: #484f58;
}

.account-username {
  font-size: 0.85rem;
  font-weight: 600;
  color: var(--text);
  flex: 1;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  line-height: 1.4;
  pointer-events: none;
  user-select: none;
}

.account-menu-btn {
  flex-shrink: 0;
  background: none;
  border: none;
  cursor: pointer;
  color: var(--text-muted);
  padding: 4px 6px;
  border-radius: 4px;
  display: flex;
  align-items: center;
  justify-content: center;
  transition: color 0.15s, background 0.15s;
  /* Expand click target over the right side of the chip */
  position: relative;
  z-index: 1;
}

.account-menu-btn:hover {
  color: var(--text);
  background: rgba(255, 255, 255, 0.08);
}

.chevron-icon {
  display: block;
  transition: transform 0.2s ease;
  flex-shrink: 0;
}

/* Rotate chevron when menu is open */
.account-menu-btn[aria-expanded="true"] .chevron-icon {
  transform: rotate(180deg);
}
```

---

#### 5.2.2 Account Dropdown Panel — Polished Context Menu

**Design intent:** The panel should feel like a GitHub/VS Code-style floating context menu. Deeper shadow, slightly larger border-radius (8px), a slightly darker surface colour to contrast with the sidebar. Items have a clear accent-tinted hover. A slide-in entrance animation gives a polished touch without being distracting.

```css
/* ── Account dropdown panel ──────────────────── */
.account-menu {
  position: absolute;
  top: calc(100% + 6px);
  left: 0;
  right: 0;
  background: #1c2128;
  border: 1px solid #3d444d;
  border-radius: 8px;
  z-index: 100;
  min-width: 210px;
  box-shadow:
    0 0 0 1px rgba(0, 0, 0, 0.25),
    0 8px 24px rgba(0, 0, 0, 0.55),
    0 2px 8px rgba(0, 0, 0, 0.35);
  overflow: hidden;
  /* Entrance animation */
  animation: menu-appear 0.12s ease-out;
  transform-origin: top center;
}

@keyframes menu-appear {
  from {
    opacity: 0;
    transform: translateY(-6px) scale(0.97);
  }
  to {
    opacity: 1;
    transform: translateY(0) scale(1);
  }
}

.account-menu-section-label {
  font-size: 10px;
  font-weight: 600;
  color: var(--text-muted);
  padding: 10px 12px 5px;
  text-transform: uppercase;
  letter-spacing: 0.06em;
}

.account-switcher-list {
  list-style: none;
  margin: 0;
  padding: 2px 4px;
}

.account-switcher-item {
  padding: 7px 10px;
  cursor: pointer;
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 0.85rem;
  border-radius: 6px;
  transition: background 0.1s;
}

.account-switcher-item:hover {
  background: rgba(88, 166, 255, 0.12);
}

.account-active {
  font-weight: 600;
  cursor: default;
}

.account-active:hover {
  background: rgba(88, 166, 255, 0.06);
}

.account-active-dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  background: var(--green);
  flex-shrink: 0;
  margin-left: auto;
  /* Replace Unicode ● with a proper CSS circle */
  font-size: 0;   /* hide the text content (●) */
  display: inline-block;
}

.account-item-username {
  font-size: 0.85rem;
  font-weight: 500;
  color: var(--text);
}

.account-item-label {
  font-size: 11px;
  color: var(--text-muted);
  margin-left: auto;
}

/* When both username and active-dot are present, push label before dot */
.account-active .account-item-label {
  margin-left: 0;
}

.account-menu-divider {
  height: 1px;
  background: #3d444d;
  margin: 4px 0;
}

.account-menu-action {
  display: flex;
  align-items: center;
  width: calc(100% - 8px);
  margin: 0 4px;
  text-align: left;
  padding: 7px 10px;
  background: none;
  border: none;
  cursor: pointer;
  color: var(--text);
  font-size: 0.85rem;
  border-radius: 6px;
  transition: background 0.1s;
}

.account-menu-action:hover {
  background: rgba(255, 255, 255, 0.08);
}

.account-menu-danger {
  color: var(--red);
}

.account-menu-danger:hover {
  background: rgba(248, 81, 73, 0.12);
  color: var(--red);
}
```

---

#### 5.2.3 Add Repository Button — Modern Ghost/Accent Button

**Design intent:** Replace the dashed placeholder-feeling border with a solid, subtle, accent-tinted ghost button. The `+` prefix becomes visually aligned, the button has a clear hover fill transition (accent semi-transparent fill + brighter border), and a gentle `transform: translateY` interaction to give physical depth feedback.

```css
/* ── Add Repository button ───────────────────── */
.btn-add-repo {
  display: flex;
  align-items: center;
  gap: 6px;
  width: 100%;
  padding: 0.45rem 0.75rem;
  margin-bottom: 0.6rem;
  background: transparent;
  border: 1px solid rgba(88, 166, 255, 0.35);
  border-radius: 8px;
  color: var(--accent);
  font-size: 0.85rem;
  font-weight: 500;
  font-family: var(--font);
  text-align: left;
  cursor: pointer;
  transition:
    background 0.15s ease,
    border-color 0.15s ease,
    transform 0.1s ease,
    box-shadow 0.15s ease;
  line-height: 1.4;
}

.btn-add-repo:hover {
  background: rgba(88, 166, 255, 0.1);
  border-color: rgba(88, 166, 255, 0.65);
  transform: translateY(-1px);
  box-shadow: 0 2px 8px rgba(88, 166, 255, 0.15);
}

.btn-add-repo:active {
  transform: translateY(0);
  box-shadow: none;
  background: rgba(88, 166, 255, 0.15);
}
```

---

## 6. Change Summary

### Files to Modify

#### `src/index.html`
One change only — replace the text content of `#account-menu-btn`.

**Line to find:**
```html
<button id="account-menu-btn" class="account-menu-btn" title="Manage accounts" aria-haspopup="true" aria-expanded="false">▾</button>
```

**Replace with:**
```html
<button id="account-menu-btn" class="account-menu-btn" title="Manage accounts" aria-haspopup="true" aria-expanded="false">
  <svg class="chevron-icon" width="12" height="12" viewBox="0 0 12 12" fill="currentColor" aria-hidden="true">
    <path d="M2.22 4.47a.75.75 0 0 1 1.06 0L6 7.19l2.72-2.72a.75.75 0 1 1 1.06 1.06L6.53 8.78a.75.75 0 0 1-1.06 0L2.22 5.53a.75.75 0 0 1 0-1.06Z"/>
  </svg>
</button>
```

#### `src/styles.css`

Replace the following existing rule blocks with the proposed rules above:

| Section | Current selector(s) | Action |
|---|---|---|
| Account chip container | `.account-chip` | Replace |
| Account username | `.account-username` | Replace |
| Account menu button | `.account-menu-btn`, `.account-menu-btn:hover` | Replace |
| Chevron icon | (new) `.chevron-icon`, `.account-menu-btn[aria-expanded="true"] .chevron-icon` | Add new |
| Account dropdown panel | `.account-menu` | Replace |
| Dropdown open animation | (new) `@keyframes menu-appear` | Add new |
| Section label | `.account-menu-section-label` | Replace |
| Switcher list | `.account-switcher-list` | Replace |
| Switcher item | `.account-switcher-item`, `.account-switcher-item:hover` | Replace |
| Active state | `.account-active` | Replace |
| Active dot | `.account-active-dot` | Replace |
| Item label | `.account-item-label` | Replace |
| Divider | `.account-menu-divider` | Replace |
| Menu action buttons | `.account-menu-action`, `.account-menu-action:hover`, `.account-menu-danger` | Replace |
| Add repo button | `.btn-add-repo`, `.btn-add-repo:hover` | Replace, add `:active` |

---

## 7. Risk Assessment

| Risk | Likelihood | Mitigation |
|---|---|---|
| `display: flex` on `.btn-add-repo` breaks the existing `text-align: left` layout | None | `display: flex` + `align-items: center` is strictly better — it properly aligns the `+` prefix left |
| `:has()` not supported in old WebKit | Low | Tauri on Windows uses Edge WebView2 (Chromium); on Linux uses webkit2gtk ≥ 2.38. Both support `:has()` as of 2022. Graceful degradation: chip just doesn't highlight on hover, still fully functional. |
| `animation: menu-appear` conflicts with `.hidden` toggle | None | `.hidden { display: none !important; }` hides the element instantly; the animation only runs when the element is re-shown (display goes from none → block via class removal), which is the correct trigger point |
| `font-size: 0` on `.account-active-dot` hides the `●` text | Intentional | The Unicode `●` is replaced visually with a CSS-painted circle via `width/height/background/border-radius`. The text content remains in the DOM for backwards compat (JS still injects it). |
| `#account-menu-btn` SVG path renders incorrectly | Very low | The SVG path is a standard downward chevron from GitHub's Primer icon set (MIT-licensed). Same dimensions as the `▾` glyph it replaces. If it renders incorrectly, fallback is visible: white square will appear, which still conveys a toggle affordance. |
| `.account-menu-action` `width: calc(100% - 8px)` + `margin: 0 4px` breaks layout | None | This creates inset padding for the action buttons so they appear visually indented with rounded corners inside the panel, consistent with the switcher items' inset. |

---

## 8. Design Token Additions

Two new derived values introduced in this spec (not added to `:root`, used inline):

| Value | Usage |
|---|---|
| `#1c2128` | Dropdown menu background — one step darker than `--surface` (`#161b22`) to create panel-on-sidebar contrast |
| `#3d444d` | Dropdown border and divider — one step brighter than `--border` (`#30363d`) to match elevated surface contrast |
| `#292e36` | Chip hover background — midpoint between `#21262d` and `--border` |
| `#484f58` | Chip hover border — muted highlight step above `#3d444d` |

These values are consistent with GitHub's Primer colour system (GitHub Dark theme scale).
