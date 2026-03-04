// ── Tauri API bridge ────────────────────────────
// Use lazy wrappers — window.__TAURI__ is injected after script parse time
const invoke = (...args) => window.__TAURI__.tauri.invoke(...args);
const save   = (...args) => window.__TAURI__.dialog.save(...args);

// ── DOM references ──────────────────────────────
const $  = (sel) => document.querySelector(sel);
const $$ = (sel) => document.querySelectorAll(sel);

const loginScreen  = $("#login-screen");
const appScreen    = $("#app-container");
const loginError   = $("#login-error");
const usernameEl   = $("#username");
const logoutBtn    = $("#logout-btn");
const repoSearch   = $("#repo-search");
const repoList     = $("#repo-list");
const stateFilter  = $("#state-filter");
const sortFilter   = $("#sort-filter");
const searchInput  = $("#search-input");
const exportCsv    = $("#export-csv-btn");
const exportPdf    = $("#export-pdf-btn");
const placeholder  = $("#placeholder");
const loading      = $("#loading");

// ── State ───────────────────────────────────────
let repos     = [];
let issues    = [];
let pulls     = [];
let alerts    = [];
let activeTab = "issues";
let selectedRepo = null;   // { owner, name }

// ── Boot ────────────────────────────────────────
window.addEventListener("error", (e) => {
  document.getElementById("login-error").textContent = "JS error: " + e.message;
  document.getElementById("login-error").classList.remove("hidden");
});

document.addEventListener("DOMContentLoaded", async () => {
  const errEl = document.getElementById("login-error");
  const showErr = (msg) => { errEl.textContent = msg; errEl.classList.remove("hidden"); };

  let devMode = false;
  try {
    devMode = await invoke("get_dev_mode");
  } catch (e) {
    showErr("get_dev_mode failed: " + String(e));
    return;
  }

  if (devMode) {
    document.getElementById("dev-mode-banner").classList.remove("hidden");
    let user;
    try {
      user = await invoke("restore_session");
    } catch (e) {
      showErr("restore_session failed: " + String(e));
      return;
    }
    if (!user) { showErr("restore_session returned null in mock mode"); return; }
    try {
      await showApp(user);
    } catch (e) {
      showErr("showApp failed: " + String(e));
    }
    return;
  }

  // Normal mode: try to restore a saved session from the OS keyring
  try {
    const user = await invoke("restore_session");
    if (user) await showApp(user);
  } catch (_) { /* no stored session — stay on login screen */ }
});

// ── Auth ────────────────────────────────────────
// OAuth Device Flow
let pollingCancelled = false;

document.getElementById('signin-btn').addEventListener('click', async () => {
  pollingCancelled = false;
  document.getElementById('signin-btn').disabled = true;
  loginError.classList.add('hidden');

  try {
    const flow = await invoke('start_device_flow');

    // Show the code card with the user code
    document.getElementById('user-code-text').textContent = flow.user_code;
    document.getElementById('device-code-card').classList.remove('hidden');

    // Copy button handler (register once per flow invocation)
    const copyBtn = document.getElementById('copy-code-btn');
    copyBtn.addEventListener('click', () => {
      navigator.clipboard.writeText(flow.user_code);
      copyBtn.title = 'Copied!';
    }, { once: true });

    // Cancel button handler
    document.getElementById('cancel-auth-btn').addEventListener('click', () => {
      pollingCancelled = true;
      document.getElementById('device-code-card').classList.add('hidden');
      document.getElementById('signin-btn').disabled = false;
    }, { once: true });

    document.getElementById('auth-status-text').textContent = 'Waiting for authorization\u2026';

    if (!pollingCancelled) {
      try {
        const username = await invoke('poll_device_flow', {
          deviceCode: flow.device_code,
          expiresIn: flow.expires_in,
          interval: flow.interval,
        });

        // Success — transition to main app
        await showApp(username);
      } catch (err) {
        loginError.textContent = String(err);
        loginError.classList.remove('hidden');
        document.getElementById('device-code-card').classList.add('hidden');
        document.getElementById('signin-btn').disabled = false;
      }
    }
  } catch (err) {
    loginError.textContent = 'Failed to start sign-in: ' + String(err);
    loginError.classList.remove('hidden');
    document.getElementById('signin-btn').disabled = false;
  }
});

logoutBtn.addEventListener("click", async () => {
  await invoke("logout");
  loginScreen.classList.remove("hidden");
  appScreen.classList.add("hidden");
});

async function showApp(username) {
  usernameEl.textContent = `@${username}`;
  loginScreen.classList.add("hidden");
  appScreen.classList.remove("hidden");
  await loadRepos();
}

// ── Repositories ────────────────────────────────
async function loadRepos() {
  repos = await invoke("list_repos");
  renderRepoList(repos);
}

function renderRepoList(list) {
  repoList.innerHTML = "";
  list.forEach((r) => {
    const li = document.createElement("li");
    li.textContent = r.full_name;
    li.title = r.description || "";
    li.addEventListener("click", () => selectRepo(r));
    repoList.appendChild(li);
  });
}

repoSearch.addEventListener("input", () => {
  const q = repoSearch.value.toLowerCase();
  renderRepoList(repos.filter((r) => r.full_name.toLowerCase().includes(q)));
});

async function selectRepo(repo) {
  selectedRepo = { owner: repo.owner, name: repo.name };
  $$('#repo-list li').forEach((li) => li.classList.remove("selected"));
  const idx = repos.indexOf(repo);
  if (repoList.children[idx]) repoList.children[idx].classList.add("selected");
  await refreshData();
}

// ── Tabs ────────────────────────────────────────
$$(".tab").forEach((btn) => {
  btn.addEventListener("click", () => {
    $$(".tab").forEach((b) => b.classList.remove("active"));
    $$(".tab-panel").forEach((p) => p.classList.remove("active"));
    btn.classList.add("active");
    activeTab = btn.dataset.tab;
    $(`#tab-${activeTab}`).classList.add("active");
  });
});

// ── Filters ─────────────────────────────────────
stateFilter.addEventListener("change", refreshData);
sortFilter.addEventListener("change", refreshData);

let searchTimeout;
searchInput.addEventListener("input", () => {
  clearTimeout(searchTimeout);
  searchTimeout = setTimeout(refreshData, 400);
});

function buildFilters() {
  return {
    state: stateFilter.value,
    sort: sortFilter.value,
    direction: "desc",
    search: searchInput.value || null,
    label: null,
    page: 1,
    per_page: 100,
  };
}

// ── Data fetching ───────────────────────────────
async function refreshData() {
  if (!selectedRepo) return;
  placeholder.classList.add("hidden");
  loading.classList.remove("hidden");

  const { owner, name } = selectedRepo;
  const filters = buildFilters();

  try {
    [issues, pulls, alerts] = await Promise.all([
      invoke("fetch_issues",          { owner, repo: name, filters }),
      invoke("fetch_pulls",           { owner, repo: name, filters }),
      invoke("fetch_security_alerts", { owner, repo: name }),
    ]);
  } catch (e) {
    console.error(e);
    issues = []; pulls = []; alerts = [];
  }

  // Client-side text search (the API search is limited)
  const q = (filters.search || "").toLowerCase();
  if (q) {
    issues = issues.filter((i) => i.title.toLowerCase().includes(q));
    pulls  = pulls.filter((p)  => p.title.toLowerCase().includes(q));
    alerts = alerts.filter((a) => a.summary.toLowerCase().includes(q));
  }

  renderIssues();
  renderPulls();
  renderAlerts();

  loading.classList.add("hidden");
  exportCsv.disabled = false;
  exportPdf.disabled = false;
}

// ── Rendering ───────────────────────────────────
function stateBadge(state) {
  const s = state.toLowerCase();
  if (s === "open")   return `<span class="badge badge-open">open</span>`;
  if (s === "closed") return `<span class="badge badge-closed">closed</span>`;
  if (s === "merged" || s.includes("merged"))
    return `<span class="badge badge-merged">merged</span>`;
  return `<span class="badge">${state}</span>`;
}

function labelBadges(labels) {
  return labels.map((l) => `<span class="badge badge-label">${l}</span>`).join(" ");
}

function shortDate(iso) {
  return new Date(iso).toLocaleDateString(undefined, { year: "numeric", month: "short", day: "numeric" });
}

function renderIssues() {
  const tbody = $("#issues-table tbody");
  tbody.innerHTML = issues
    .map((i) => `<tr>
        <td>${i.number}</td>
        <td><a href="${i.html_url}" target="_blank">${esc(i.title)}</a></td>
        <td>${stateBadge(i.state)}</td>
        <td>${esc(i.author)}</td>
        <td>${labelBadges(i.labels)}</td>
        <td>${shortDate(i.created_at)}</td>
      </tr>`)
    .join("");
}

function renderPulls() {
  const tbody = $("#pulls-table tbody");
  tbody.innerHTML = pulls
    .map((p) => `<tr>
        <td>${p.number}</td>
        <td><a href="${p.html_url}" target="_blank">${esc(p.title)}</a></td>
        <td>${stateBadge(p.state)}</td>
        <td>${esc(p.author)}</td>
        <td>${esc(p.head_branch)} → ${esc(p.base_branch)}</td>
        <td>${p.draft ? "✓" : ""}</td>
      </tr>`)
    .join("");
}

function renderAlerts() {
  const tbody = $("#alerts-table tbody");
  tbody.innerHTML = alerts
    .map((a) => {
      const sev = a.severity.toLowerCase();
      const cls = sev === "critical" ? "severity-critical"
                : sev === "high"     ? "severity-high"
                : sev === "medium"   ? "severity-medium"
                :                      "severity-low";
      return `<tr>
        <td>${a.id}</td>
        <td class="${cls}">${esc(a.severity)}</td>
        <td><a href="${a.html_url}" target="_blank">${esc(a.summary)}</a></td>
        <td>${esc(a.package_name || "—")}</td>
        <td>${esc(a.vulnerable_version_range || "—")}</td>
        <td>${esc(a.patched_version || "—")}</td>
      </tr>`;
    })
    .join("");
}

// ── Export ───────────────────────────────────────
exportCsv.addEventListener("click", () => doExport("csv"));
exportPdf.addEventListener("click", () => doExport("pdf"));

async function doExport(format) {
  const ext = format === "csv" ? "csv" : "pdf";
  const filePath = await save({
    filters: [{ name: ext.toUpperCase(), extensions: [ext] }],
    defaultPath: `github-export.${ext}`,
  });
  if (!filePath) return;

  try {
    const msg = await invoke("export_data", {
      format,
      issues,
      pulls,
      alerts,
      filePath,
    });
    alert(msg);
  } catch (e) {
    alert(`Export failed: ${e}`);
  }
}

// ── Utils ───────────────────────────────────────
function esc(str) {
  const el = document.createElement("span");
  el.textContent = str;
  return el.innerHTML;
}
