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
let workflowRuns  = [];
let activeTab = "issues";
let selectedRepo = null;   // { owner, name }
let expandedRow  = null;   // { type: "issues"|"pulls"|"alerts", idx: number } | null
let actionsLoaded = false; // whether workflow runs have been fetched for the current repo
let accounts        = [];  // AccountInfo[] — mirrors backend accounts list
let activeAccountId = null; // String — currently active account id
let trackedRepos  = [];     // TrackedRepo[] — the user's curated tracked list
let allRepos      = [];     // Repo[] — full GitHub repo list (lazy-loaded for picker)
let pickerLoaded  = false;  // whether allRepos has been fetched this session
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
    const result = await invoke("restore_session");
    if (result) {
      accounts = result.accounts;
      activeAccountId = accounts.find(a => a.is_active)?.id ?? null;
      await showApp(result.username);
    }
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

        // Success — refresh accounts and transition to main app
        accounts = await invoke('list_accounts');
        activeAccountId = accounts.find(a => a.is_active)?.id ?? null;
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

document.getElementById('pat-submit-btn').addEventListener('click', async () => {
  const token = document.getElementById('pat-input').value.trim();
  if (!token) {
    loginError.textContent = 'Please enter a Personal Access Token.';
    loginError.classList.remove('hidden');
    return;
  }
  document.getElementById('pat-submit-btn').disabled = true;
  loginError.classList.add('hidden');
  try {
    const username = await invoke('authenticate_with_pat', { token, label: null });
    accounts = await invoke('list_accounts');
    activeAccountId = accounts.find(a => a.is_active)?.id ?? null;
    await showApp(username);
  } catch (err) {
    loginError.textContent = 'PAT sign-in failed: ' + String(err);
    loginError.classList.remove('hidden');
    document.getElementById('pat-submit-btn').disabled = false;
  }
});

logoutBtn.addEventListener("click", async () => {
  await invoke("logout");
  accounts = [];
  activeAccountId = null;
  clearTabBadges();
  loginScreen.classList.remove("hidden");
  appScreen.classList.add("hidden");
  document.getElementById('pat-input').value = '';
  document.getElementById('pat-submit-btn').disabled = false;
  document.getElementById('signin-btn').disabled = false;
  document.getElementById('device-code-card').classList.add('hidden');
  loginError.classList.add('hidden');
});

async function showApp(username) {
  usernameEl.textContent = `@${username}`;
  renderAccountSwitcher();
  loginScreen.classList.add("hidden");
  appScreen.classList.remove("hidden");
  pickerLoaded = false;
  allRepos = [];
  await loadTrackedRepos();
}

// ── Account Switcher ────────────────────────────────
function renderAccountSwitcher() {
  const list = document.getElementById("account-list");
  if (!list) return;
  list.innerHTML = "";
  accounts.forEach((acct) => {
    const li = document.createElement("li");
    li.className = "account-switcher-item" + (acct.is_active ? " account-active" : "");
    li.setAttribute("role", "menuitem");
    li.innerHTML = `
      <span class="account-item-username">@${esc(acct.username)}</span>
      <span class="account-item-label">${esc(acct.label)}</span>
      ${acct.is_active ? '<span class="account-active-dot" aria-label="Active">●</span>' : ""}
    `;
    if (!acct.is_active) {
      li.addEventListener("click", () => handleSwitchAccount(acct.id));
    }
    list.appendChild(li);
  });
}

document.getElementById("account-menu-btn").addEventListener("click", (e) => {
  e.stopPropagation();
  const menu = document.getElementById("account-menu");
  const btn  = document.getElementById("account-menu-btn");
  const isOpen = !menu.classList.contains("hidden");
  menu.classList.toggle("hidden", isOpen);
  btn.setAttribute("aria-expanded", String(!isOpen));
});

document.addEventListener("click", () => {
  document.getElementById("account-menu")?.classList.add("hidden");
  document.getElementById("account-menu-btn")?.setAttribute("aria-expanded", "false");
});

async function handleSwitchAccount(accountId) {
  document.getElementById("account-menu").classList.add("hidden");
  try {
    const username = await invoke("switch_account", { accountId });
    accounts = await invoke("list_accounts");
    activeAccountId = accountId;
    usernameEl.textContent = `@${username}`;
    renderAccountSwitcher();
    selectedRepo = null;
    repos = [];
    issues = [];
    pulls = [];
    alerts = [];
    clearTabBadges();
    pickerLoaded = false;
    allRepos = [];
    repoList.innerHTML = "";
    placeholder.classList.remove("hidden");
    await loadTrackedRepos();
  } catch (err) {
    alert(`Failed to switch account: ${err}`);
  }
}

document.getElementById("add-account-btn").addEventListener("click", () => {
  document.getElementById("account-menu").classList.add("hidden");
  document.getElementById("add-account-modal").classList.remove("hidden");
  document.getElementById("add-account-token").value = "";
  document.getElementById("add-account-label").value = "";
  document.getElementById("add-account-error").classList.add("hidden");
  document.getElementById("add-account-submit-btn").disabled = false;
});

document.getElementById("add-account-cancel-btn").addEventListener("click", () => {
  document.getElementById("add-account-modal").classList.add("hidden");
});

document.getElementById("add-account-submit-btn").addEventListener("click", async () => {
  const token = document.getElementById("add-account-token").value.trim();
  const label = document.getElementById("add-account-label").value.trim() || null;
  const errEl = document.getElementById("add-account-error");
  if (!token) {
    errEl.textContent = "Please enter a Personal Access Token.";
    errEl.classList.remove("hidden");
    return;
  }
  document.getElementById("add-account-submit-btn").disabled = true;
  errEl.classList.add("hidden");
  try {
    const newAcct = await invoke("add_account", { token, label });
    await handleSwitchAccount(newAcct.id);
    document.getElementById("add-account-modal").classList.add("hidden");
  } catch (err) {
    errEl.textContent = String(err);
    errEl.classList.remove("hidden");
    document.getElementById("add-account-submit-btn").disabled = false;
  }
});

document.getElementById("remove-account-btn").addEventListener("click", async () => {
  const acct = accounts.find(a => a.is_active);
  if (!acct) return;
  const confirmed = confirm(
    `Remove account @${acct.username} from this app?\n\nYour GitHub token will be deleted from the OS credential store. You will not be logged out of GitHub itself.`
  );
  if (!confirmed) return;
  document.getElementById("account-menu").classList.add("hidden");
  try {
    await invoke("remove_account", { accountId: acct.id });
    accounts = await invoke("list_accounts");
    if (accounts.length === 0) {
      loginScreen.classList.remove("hidden");
      appScreen.classList.add("hidden");
      repos = []; issues = []; pulls = []; alerts = [];
      selectedRepo = null;
    } else {
      await handleSwitchAccount(accounts[0].id);
    }
  } catch (err) {
    alert(`Failed to remove account: ${err}`);
  }
});


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

// ── Tracked Repositories ────────────────────────
async function loadTrackedRepos() {
  try {
    trackedRepos = await invoke("get_tracked_repos");
  } catch (e) {
    console.error("get_tracked_repos failed:", e);
    trackedRepos = [];
  }
  renderTrackedRepoList(trackedRepos);
}

function renderTrackedRepoList(list) {
  const q = repoSearch.value.toLowerCase();
  const filtered = q
    ? list.filter((r) => r.full_name.toLowerCase().includes(q))
    : list;

  repoList.innerHTML = "";

  if (filtered.length === 0 && !q) {
    const li = document.createElement("li");
    li.className = "repo-list-empty";
    li.textContent = "No repositories tracked yet.";
    repoList.appendChild(li);
    return;
  }

  filtered.forEach((r) => {
    const li = document.createElement("li");
    li.className = "repo-list-item";

    const nameSpan = document.createElement("span");
    nameSpan.className = "repo-list-name";
    nameSpan.textContent = r.full_name;
    nameSpan.title = r.full_name;
    li.appendChild(nameSpan);

    const removeBtn = document.createElement("button");
    removeBtn.className = "repo-remove-btn";
    removeBtn.title = `Remove ${r.full_name}`;
    removeBtn.textContent = "×";
    removeBtn.addEventListener("click", (e) => {
      e.stopPropagation();
      handleRemoveTrackedRepo(r.full_name);
    });
    li.appendChild(removeBtn);

    li.addEventListener("click", () => selectTrackedRepo(r));
    repoList.appendChild(li);
  });

  // Re-apply selected state if selectedRepo is in the filtered list
  if (selectedRepo) {
    Array.from(repoList.querySelectorAll(".repo-list-item")).forEach((li, idx) => {
      if (
        filtered[idx] &&
        filtered[idx].full_name === `${selectedRepo.owner}/${selectedRepo.name}`
      ) {
        li.classList.add("selected");
      }
    });
  }
}

function selectTrackedRepo(repo) {
  selectedRepo = { owner: repo.owner, name: repo.name };
  Array.from(repoList.querySelectorAll(".repo-list-item")).forEach((li) =>
    li.classList.remove("selected")
  );
  Array.from(repoList.querySelectorAll(".repo-list-item")).forEach((li) => {
    const span = li.querySelector(".repo-list-name");
    if (span && span.textContent === repo.full_name) {
      li.classList.add("selected");
    }
  });
  refreshData();
}

async function openAddRepoModal() {
  const modal = document.getElementById("add-repo-modal");
  const searchInput = document.getElementById("add-repo-search");
  const listEl = document.getElementById("add-repo-list");
  const errorEl = document.getElementById("add-repo-error");

  modal.classList.remove("hidden");
  searchInput.value = "";
  errorEl.classList.add("hidden");

  if (!pickerLoaded) {
    listEl.innerHTML =
      '<li class="add-repo-loading"><span class="spinner-small"></span> Loading repositories\u2026</li>';
    try {
      allRepos = await invoke("list_all_repos");
      pickerLoaded = true;
    } catch (e) {
      listEl.innerHTML = `<li class="add-repo-error-item">Failed to load repositories: ${esc(String(e))}</li>`;
      return;
    }
  }

  renderPickerList(allRepos, searchInput.value);
  searchInput.focus();
}

function renderPickerList(repos, query) {
  const listEl = document.getElementById("add-repo-list");
  const q = (query || "").toLowerCase();
  const filtered = q ? repos.filter((r) => r.full_name.toLowerCase().includes(q)) : repos;
  const trackedSet = new Set(trackedRepos.map((r) => r.full_name));

  listEl.innerHTML = "";

  if (filtered.length === 0) {
    listEl.innerHTML = '<li class="add-repo-empty">No repositories found.</li>';
    return;
  }

  filtered.forEach((r) => {
    const li = document.createElement("li");
    li.className = "add-repo-item";
    const alreadyTracked = trackedSet.has(r.full_name);
    if (alreadyTracked) li.classList.add("add-repo-item-tracked");

    li.innerHTML = `
      <span class="add-repo-item-name">${esc(r.full_name)}</span>
      ${r.description ? `<span class="add-repo-item-desc">${esc(r.description)}</span>` : ""}
      ${alreadyTracked ? '<span class="add-repo-item-check" aria-label="Already tracked">\u2713</span>' : ""}
    `;

    if (!alreadyTracked) {
      li.addEventListener("click", () => handleAddTrackedRepo(r));
    }

    listEl.appendChild(li);
  });
}

async function handleAddTrackedRepo(repo) {
  const errorEl = document.getElementById("add-repo-error");
  errorEl.classList.add("hidden");
  try {
    trackedRepos = await invoke("add_tracked_repo", {
      fullName: repo.full_name,
      owner: repo.owner,
      name: repo.name,
    });
    document.getElementById("add-repo-modal").classList.add("hidden");
    renderTrackedRepoList(trackedRepos);
    const added = trackedRepos.find((r) => r.full_name === repo.full_name);
    if (added) selectTrackedRepo(added);
  } catch (e) {
    errorEl.textContent = `Failed to add repository: ${esc(String(e))}`;
    errorEl.classList.remove("hidden");
  }
}

async function handleRemoveTrackedRepo(fullName) {
  try {
    trackedRepos = await invoke("remove_tracked_repo", { fullName });
    if (selectedRepo && `${selectedRepo.owner}/${selectedRepo.name}` === fullName) {
      selectedRepo = null;
      issues = [];
      pulls = [];
      alerts = [];
      placeholder.classList.remove("hidden");
      clearTabBadges();
    }
    renderTrackedRepoList(trackedRepos);
  } catch (e) {
    alert(`Failed to remove repository: ${e}`);
  }
}

// ── Add Repository button & modal events ────────
document.getElementById("add-repo-btn").addEventListener("click", openAddRepoModal);

document.getElementById("add-repo-close-btn").addEventListener("click", () => {
  document.getElementById("add-repo-modal").classList.add("hidden");
});

document.getElementById("add-repo-modal").addEventListener("click", (e) => {
  if (e.target === document.getElementById("add-repo-modal")) {
    document.getElementById("add-repo-modal").classList.add("hidden");
  }
});

document.getElementById("add-repo-search").addEventListener("input", (e) => {
  renderPickerList(allRepos, e.target.value);
});

repoSearch.addEventListener("input", () => {
  renderTrackedRepoList(trackedRepos);
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
let issuesError = null, pullsError = null, alertsError = null;

// ── Tab badges ──────────────────────────────────
function updateTabBadges(issueCount, pullCount, alertCount) {
  const tabs = [
    { badgeSel: '#issues-tab .tab-badge',   btnSel: '#issues-tab',   base: 'Issues',          count: issueCount },
    { badgeSel: '#pulls-tab .tab-badge',    btnSel: '#pulls-tab',    base: 'Pull Requests',   count: pullCount  },
    { badgeSel: '#security-tab .tab-badge', btnSel: '#security-tab', base: 'Security Alerts', count: alertCount },
  ];
  tabs.forEach(({ badgeSel, btnSel, base, count }) => {
    const badge = $(badgeSel);
    const btn   = $(btnSel);
    if (!badge || !btn) return;
    if (count === 0) {
      badge.style.display = 'none';
      badge.textContent   = '';
    } else {
      const label = count > 99 ? '99+' : String(count);
      badge.textContent   = label;
      badge.style.display = 'inline-block';
    }
    const countLabel = count > 0 ? ': ' + (count > 99 ? '99+' : count) + ' items' : '';
    btn.setAttribute('aria-label', `${base}${countLabel}`);
  });
}

function clearTabBadges() {
  updateTabBadges(0, 0, 0);
  workflowRuns = [];
  actionsLoaded = false;
  updateActionStatusDot([]);
}

async function refreshData() {
  if (!selectedRepo) return;
  placeholder.classList.add("hidden");
  loading.classList.remove("hidden");
  issuesError = null; pullsError = null; alertsError = null;

  const { owner, name } = selectedRepo;
  const filters = buildFilters();

  // Use allSettled so a failure in one tab doesn't blank out the others
  const [issuesRes, pullsRes, alertsRes, runsRes] = await Promise.allSettled([
    invoke("fetch_issues",          { owner, repo: name, filters }),
    invoke("fetch_pulls",           { owner, repo: name, filters }),
    invoke("fetch_security_alerts", { owner, repo: name, state: filters.state }),
    invoke("get_workflow_runs",     { owner, repo: name }),
  ]);

  if (issuesRes.status === "fulfilled") {
    issues = issuesRes.value;
  } else {
    issuesError = String(issuesRes.reason);
    issues = [];
    console.error("fetch_issues failed:", issuesRes.reason);
  }

  if (pullsRes.status === "fulfilled") {
    pulls = pullsRes.value;
  } else {
    pullsError = String(pullsRes.reason);
    pulls = [];
    console.error("fetch_pulls failed:", pullsRes.reason);
  }

  if (alertsRes.status === "fulfilled") {
    alerts = alertsRes.value;
  } else {
    alertsError = String(alertsRes.reason);
    alerts = [];
    console.error("fetch_security_alerts failed:", alertsRes.reason);
  }

  if (runsRes.status === "fulfilled") {
    workflowRuns = runsRes.value;
    actionsLoaded = true;
    if (activeTab === "actions") renderWorkflowRuns(workflowRuns);
    updateActionStatusDot(workflowRuns);
    document.getElementById("export-actions-btn").disabled = workflowRuns.length === 0;
  } else {
    workflowRuns = [];
    actionsLoaded = false;
    if (activeTab === "actions") {
      const actionsErrorEl = document.getElementById("actions-error");
      const actionsEmptyEl = document.getElementById("actions-empty");
      const actionsTableEl = document.getElementById("actions-table");
      if (actionsErrorEl) {
        actionsErrorEl.textContent = "Failed to load workflow runs: " + String(runsRes.reason);
        actionsErrorEl.classList.remove("hidden");
      }
      if (actionsEmptyEl) actionsEmptyEl.classList.add("hidden");
      if (actionsTableEl) actionsTableEl.classList.add("hidden");
    }
    updateActionStatusDot([]);
    document.getElementById("export-actions-btn").disabled = true;
    console.error("get_workflow_runs failed:", runsRes.reason);
  }

  // Client-side text search
  const q = (filters.search || "").toLowerCase();
  if (q) {
    issues = issues.filter((i) => i.title.toLowerCase().includes(q));
    pulls  = pulls.filter((p)  => p.title.toLowerCase().includes(q));
    alerts = alerts.filter((a) => a.summary.toLowerCase().includes(q));
  }

  renderIssues();
  renderPulls();
  renderAlerts();
  updateTabBadges(issues.length, pulls.length, alerts.length);

  expandedRow = null; // Reset expanded state on data refresh

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
  if (issuesError) {
    tbody.innerHTML = `<tr><td colspan="6" class="fetch-error">Failed to load issues: ${esc(issuesError)}</td></tr>`;
    return;
  }
  tbody.innerHTML = issues
    .map((i, idx) => `
      <tr class="data-row clickable-row" data-idx="${idx}" onclick="toggleDetailRow('issues', ${idx})">
        <td>${i.number}</td>
        <td>${esc(i.title)}</td>
        <td>${stateBadge(i.state)}</td>
        <td>${esc(i.author)}</td>
        <td>${labelBadges(i.labels)}</td>
        <td>${shortDate(i.created_at)}</td>
      </tr>
      <tr class="detail-row" id="detail-issues-${idx}">
        <td colspan="6">
          <div class="detail-body">${buildIssueDetail(i)}</div>
        </td>
      </tr>`)
    .join("");
}

function renderPulls() {
  const tbody = $("#pulls-table tbody");
  if (pullsError) {
    tbody.innerHTML = `<tr><td colspan="6" class="fetch-error">Failed to load pull requests: ${esc(pullsError)}</td></tr>`;
    return;
  }
  tbody.innerHTML = pulls
    .map((p, idx) => `
      <tr class="data-row clickable-row" data-idx="${idx}" onclick="toggleDetailRow('pulls', ${idx})">
        <td>${p.number}</td>
        <td>${esc(p.title)}</td>
        <td>${stateBadge(p.state)}</td>
        <td>${esc(p.author)}</td>
        <td>${esc(p.head_branch)} → ${esc(p.base_branch)}</td>
        <td>${p.draft ? "✓" : ""}</td>
      </tr>
      <tr class="detail-row" id="detail-pulls-${idx}">
        <td colspan="6">
          <div class="detail-body">${buildPullDetail(p)}</div>
        </td>
      </tr>`)
    .join("");
}

function normalizeSeverity(severity, alertType) {
  if (alertType === 'code_scanning') {
    const map = { 'critical': 'critical', 'high': 'high', 'error': 'high', 'medium': 'medium', 'warning': 'medium', 'low': 'low', 'note': 'low', 'info': 'low' };
    return map[severity?.toLowerCase()] || 'low';
  }
  return severity?.toLowerCase() || 'low';
}

function renderAlerts() {
  const tbody = $("#alerts-table tbody");
  if (alertsError) {
    const isDisabled = alertsError.includes("disabled for this repository");
    const cssClass = isDisabled ? "fetch-info" : "fetch-error";
    const htmlMsg = esc(alertsError).replace(/\n/g, "<br>");
    const guidance = isDisabled
      ? `<br><br><strong>To enable:</strong> Go to your repository on GitHub →
         <strong>Settings</strong> → <strong>Security</strong> section →
         <strong>Advanced Security</strong> →
         <strong>Dependabot Alerts</strong> → click <strong>Enable</strong>.
         Then click the refresh button (↺) above.`
      : "";
    tbody.innerHTML = `<tr><td colspan="7" class="${cssClass}">${htmlMsg}${guidance}</td></tr>`;
    return;
  }
  if (alerts.length === 0) {
    tbody.innerHTML = `<tr><td colspan="7" class="fetch-info">
      No security alerts found for the current filter.
      <br><br>
      <strong>Tips:</strong>
      <br>• Try switching the filter to <strong>All</strong> to see dismissed or fixed alerts.
      <br>• If you just enabled Dependabot, GitHub may still be scanning &mdash; wait a minute and click the refresh button (↺) above.
      <br>• To enable Dependabot: GitHub repository → <strong>Settings</strong> → <strong>Security</strong> section → <strong>Advanced Security</strong> → <strong>Dependabot Alerts</strong> → <strong>Enable</strong>.
      <br>• Code Scanning alerts appear here if a code scanning workflow (e.g., GitHub Actions CodeQL) is configured.
    </td></tr>`;
    return;
  }
  tbody.innerHTML = alerts
    .map((a, idx) => {
      const normalizedSev = normalizeSeverity(a.severity, a.alert_type);
      const cls = normalizedSev === "critical" ? "severity-critical"
                : normalizedSev === "high"     ? "severity-high"
                : normalizedSev === "medium"   ? "severity-medium"
                :                               "severity-low";
      const typeLabel = a.alert_type === "code_scanning"
        ? `Code Scanning${a.tool_name ? ` (${esc(a.tool_name)})` : ""}`
        : "Dependabot";
      return `
      <tr class="data-row clickable-row" data-idx="${idx}" onclick="toggleDetailRow('alerts', ${idx})">
        <td>${a.id}</td>
        <td>${typeLabel}</td>
        <td class="${cls}">${esc(a.severity)}</td>
        <td>${esc(a.summary)}</td>
        <td>${esc(a.package_name || "—")}</td>
        <td>${esc(a.vulnerable_version_range || "—")}</td>
        <td>${esc(a.patched_version || "—")}</td>
      </tr>
      <tr class="detail-row" id="detail-alerts-${idx}">
        <td colspan="7">
          <div class="detail-body">${buildAlertDetail(a)}</div>
        </td>
      </tr>`;
    })
    .join("");
}

// ── Refresh button ──────────────────────────────
document.getElementById('refresh-btn').addEventListener('click', () => {
  refreshData();
});

// ── Actions tab ─────────────────────────────────

function updateActionStatusDot(runs) {
  const dot = document.querySelector('#actions-tab .tab-status-dot');
  const btn = document.getElementById('actions-tab');
  if (!dot || !btn) return;

  // Reset to hidden
  dot.className = 'tab-status-dot';
  btn.setAttribute('aria-label', 'Actions');

  if (!runs || runs.length === 0) return;

  const latest = runs[0];
  if (latest.status === 'in_progress' || latest.status === 'queued') {
    dot.classList.add('tab-status-dot--pending');
    btn.setAttribute('aria-label', 'Actions: run in progress');
  } else if (latest.conclusion === 'success') {
    dot.classList.add('tab-status-dot--success');
    btn.setAttribute('aria-label', 'Actions: last run passed');
  } else if (
    latest.conclusion === 'failure' ||
    latest.conclusion === 'timed_out' ||
    latest.conclusion === 'action_required'
  ) {
    dot.classList.add('tab-status-dot--failure');
    btn.setAttribute('aria-label', 'Actions: last run failed');
  } else {
    dot.classList.add('tab-status-dot--neutral');
    btn.setAttribute('aria-label', 'Actions: last run ' + (latest.conclusion || 'unknown'));
  }
}

function renderWorkflowRuns(runs) {
  const tbody = document.getElementById('actions-tbody');
  const tableEl = document.getElementById('actions-table');
  const emptyEl = document.getElementById('actions-empty');

  if (!runs || runs.length === 0) {
    if (tableEl) tableEl.classList.add('hidden');
    if (emptyEl) emptyEl.classList.remove('hidden');
    return;
  }

  if (emptyEl) emptyEl.classList.add('hidden');
  if (tableEl) tableEl.classList.remove('hidden');

  tbody.innerHTML = runs.map((r) => {
    const status = r.status || '';
    const conclusion = r.conclusion || '';

    let badgeColor;
    if (conclusion === 'success') {
      badgeColor = 'var(--green)';
    } else if (conclusion === 'failure' || conclusion === 'timed_out' || conclusion === 'action_required') {
      badgeColor = 'var(--red)';
    } else {
      badgeColor = 'var(--text-muted)';
    }

    const statusLabel = conclusion ? `${esc(status)} / ${esc(conclusion)}` : esc(status);
    const badgeHtml = `<span style="color:${badgeColor};font-weight:600">${statusLabel}</span>`;

    const started = r.run_started_at || r.created_at;
    const startedLabel = started ? shortDate(started) : '—';

    const safeUrl = /^https?:\/\//i.test(r.html_url) ? r.html_url : '#';
    const linkHtml = `<a href="${esc(safeUrl)}" target="_blank" rel="noopener noreferrer">View</a>`;

    return `<tr>
      <td>${esc(r.name)}</td>
      <td>${esc(r.head_branch || '—')}</td>
      <td>${badgeHtml}</td>
      <td>${esc(r.actor_login)}</td>
      <td>${esc(startedLabel)}</td>
      <td>${linkHtml}</td>
    </tr>`;
  }).join('');
}

async function loadActions() {
  if (!selectedRepo) return;

  const loadingEl = document.getElementById('actions-loading');
  const errorEl   = document.getElementById('actions-error');
  const emptyEl   = document.getElementById('actions-empty');
  const tableEl   = document.getElementById('actions-table');

  if (loadingEl) loadingEl.classList.remove('hidden');
  if (errorEl)   errorEl.classList.add('hidden');
  if (emptyEl)   emptyEl.classList.add('hidden');
  if (tableEl)   tableEl.classList.add('hidden');

  const { owner, name } = selectedRepo;
  try {
    workflowRuns = await invoke('get_workflow_runs', { owner, repo: name });
    actionsLoaded = true;
    renderWorkflowRuns(workflowRuns);
    updateActionStatusDot(workflowRuns);
    document.getElementById('export-actions-btn').disabled = false;
  } catch (e) {
    workflowRuns = [];
    if (errorEl) {
      errorEl.textContent = 'Failed to load workflow runs: ' + String(e);
      errorEl.classList.remove('hidden');
    }
    console.error('get_workflow_runs failed:', e);
  } finally {
    if (loadingEl) loadingEl.classList.add('hidden');
  }
}

document.getElementById('actions-tab').addEventListener('click', () => {
  if (!actionsLoaded) {
    loadActions();
  } else {
    renderWorkflowRuns(workflowRuns);
  }
});

document.getElementById('export-actions-btn').addEventListener('click', async () => {
  const filePath = await save({
    filters: [{ name: 'CSV', extensions: ['csv'] }],
    defaultPath: 'workflow-runs.csv',
  });
  if (!filePath) return;
  try {
    const msg = await invoke('export_actions_csv', { runs: workflowRuns, filePath });
    alert(msg);
  } catch (e) {
    alert('Export failed: ' + String(e));
  }
});

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

// ── Markdown rendering ──────────────────────────
function renderMarkdown(text) {
  if (!text) return '<em class="detail-no-body">No description provided.</em>';
  const rawHtml = marked.parse(text, { breaks: true, gfm: true });
  return DOMPurify.sanitize(rawHtml);
}

// ── Expandable row logic ────────────────────────
function collapseAllRows() {
  document.querySelectorAll(".detail-row.expanded").forEach((row) => {
    row.classList.remove("expanded");
  });
  document.querySelectorAll(".data-row.row-expanded").forEach((row) => {
    row.classList.remove("row-expanded");
  });
  expandedRow = null;
}

async function toggleDetailRow(type, idx) {
  const rowId = `detail-${type}-${idx}`;
  const detailRow = document.getElementById(rowId);
  if (!detailRow) return;

  const isExpanded = detailRow.classList.contains("expanded");

  // Collapse any currently open row
  collapseAllRows();

  if (!isExpanded) {
    detailRow.classList.add("expanded");
    // Highlight the parent data row
    const dataRow = detailRow.previousElementSibling;
    if (dataRow) dataRow.classList.add("row-expanded");
    expandedRow = { type, idx };

    // For PRs: lazily fetch diff stats on first expand
    if (type === "pulls" && selectedRepo) {
      const pull = pulls[idx];
      const statsEl = document.getElementById(`pull-stats-${pull.number}`);
      if (statsEl && statsEl.querySelector(".detail-pr-stats-loading")) {
        try {
          const detail = await invoke("get_pull_detail", {
            owner: selectedRepo.owner,
            repo: selectedRepo.name,
            pullNumber: pull.number,
          });
          statsEl.innerHTML = `
            <div class="detail-pr-stats-row">
              <span class="stat-additions">+${detail.additions} additions</span>
              <span class="stat-deletions">−${detail.deletions} deletions</span>
              <span class="stat-files">${detail.changed_files} file${detail.changed_files !== 1 ? "s" : ""} changed</span>
              ${detail.mergeable != null ? `<span class="stat-mergeable ${detail.mergeable ? "mergeable-yes" : "mergeable-no"}">${detail.mergeable ? "✓ Mergeable" : "✗ Not mergeable"}</span>` : ""}
            </div>`;
        } catch (e) {
          statsEl.innerHTML = `<span class="detail-stats-error">Could not load diff stats: ${esc(String(e))}</span>`;
        }
      }
    }
  }
}

// ── Detail panel builders ───────────────────────
function buildIssueDetail(issue) {
  const assignees = issue.assignees && issue.assignees.length
    ? issue.assignees.map(esc).join(", ")
    : "—";
  const labels = issue.labels && issue.labels.length
    ? labelBadges(issue.labels)
    : "—";
  const milestone = issue.milestone ? esc(issue.milestone) : "—";
  const comments = issue.comments != null ? issue.comments : "—";
  const closedDate = issue.closed_at ? shortDate(issue.closed_at) : null;

  return `
    <div class="detail-content">
      <div class="detail-header">
        <span class="detail-type-badge">Issue #${issue.number}</span>
        ${stateBadge(issue.state)}
        <button class="detail-close-btn" onclick="collapseAllRows()" title="Close">×</button>
      </div>
      <div class="detail-body-text markdown-body">${renderMarkdown(issue.body)}</div>
      <div class="detail-meta-grid">
        <div class="detail-meta-item">
          <span class="detail-meta-label">Author</span>
          <span>${esc(issue.author)}</span>
        </div>
        <div class="detail-meta-item">
          <span class="detail-meta-label">Assignees</span>
          <span>${assignees}</span>
        </div>
        <div class="detail-meta-item">
          <span class="detail-meta-label">Labels</span>
          <span>${labels}</span>
        </div>
        <div class="detail-meta-item">
          <span class="detail-meta-label">Milestone</span>
          <span>${milestone}</span>
        </div>
        <div class="detail-meta-item">
          <span class="detail-meta-label">Comments</span>
          <span>${comments}</span>
        </div>
        <div class="detail-meta-item">
          <span class="detail-meta-label">Created</span>
          <span>${shortDate(issue.created_at)}</span>
        </div>
        <div class="detail-meta-item">
          <span class="detail-meta-label">Updated</span>
          <span>${shortDate(issue.updated_at)}</span>
        </div>
        ${closedDate ? `
        <div class="detail-meta-item">
          <span class="detail-meta-label">Closed</span>
          <span>${closedDate}</span>
        </div>` : ""}
      </div>
      <div class="detail-footer">
        <a href="${esc(issue.html_url)}" target="_blank" rel="noopener noreferrer" class="detail-open-link">Open on GitHub ↗</a>
      </div>
    </div>`;
}

function buildPullDetail(pull) {
  const assignees = pull.assignees && pull.assignees.length
    ? pull.assignees.map(esc).join(", ")
    : "—";
  const reviewers = pull.reviewers && pull.reviewers.length
    ? pull.reviewers.map(esc).join(", ")
    : "—";
  const labels = pull.labels && pull.labels.length ? labelBadges(pull.labels) : "—";
  const mergedDate = pull.merged_at ? shortDate(pull.merged_at) : null;
  const closedDate = pull.closed_at && !pull.merged_at ? shortDate(pull.closed_at) : null;
  const statsId = `pull-stats-${pull.number}`;

  return `
    <div class="detail-content">
      <div class="detail-header">
        <span class="detail-type-badge">PR #${pull.number}</span>
        ${stateBadge(pull.state)}
        ${pull.draft ? '<span class="badge badge-draft">draft</span>' : ""}
        <button class="detail-close-btn" onclick="collapseAllRows()" title="Close">×</button>
      </div>
      <div class="detail-body-text markdown-body">${renderMarkdown(pull.body)}</div>
      <div class="detail-meta-grid">
        <div class="detail-meta-item">
          <span class="detail-meta-label">Author</span>
          <span>${esc(pull.author)}</span>
        </div>
        <div class="detail-meta-item">
          <span class="detail-meta-label">Assignees</span>
          <span>${assignees}</span>
        </div>
        <div class="detail-meta-item">
          <span class="detail-meta-label">Reviewers</span>
          <span>${reviewers}</span>
        </div>
        <div class="detail-meta-item">
          <span class="detail-meta-label">Labels</span>
          <span>${labels}</span>
        </div>
        <div class="detail-meta-item">
          <span class="detail-meta-label">Head → Base</span>
          <span>${esc(pull.head_branch)} → ${esc(pull.base_branch)}</span>
        </div>
        <div class="detail-meta-item">
          <span class="detail-meta-label">Created</span>
          <span>${shortDate(pull.created_at)}</span>
        </div>
        <div class="detail-meta-item">
          <span class="detail-meta-label">Updated</span>
          <span>${shortDate(pull.updated_at)}</span>
        </div>
        ${mergedDate ? `
        <div class="detail-meta-item">
          <span class="detail-meta-label">Merged</span>
          <span>${mergedDate}</span>
        </div>` : ""}
        ${closedDate ? `
        <div class="detail-meta-item">
          <span class="detail-meta-label">Closed</span>
          <span>${closedDate}</span>
        </div>` : ""}
      </div>
      <div id="${statsId}" class="detail-pr-stats">
        <div class="detail-pr-stats-loading">
          <span class="spinner-small"></span> Loading diff stats…
        </div>
      </div>
      <div class="detail-footer">
        <a href="${esc(pull.html_url)}" target="_blank" rel="noopener noreferrer" class="detail-open-link">Open on GitHub ↗</a>
      </div>
    </div>`;
}

function buildAlertDetail(alert) {
  const normalizedSev = normalizeSeverity(alert.severity, alert.alert_type);
  const sevClass = normalizedSev === "critical" ? "severity-critical"
                 : normalizedSev === "high"     ? "severity-high"
                 : normalizedSev === "medium"   ? "severity-medium"
                 :                               "severity-low";
  const cvss = alert.cvss_score != null
    ? `<span class="detail-cvss-score">${alert.cvss_score.toFixed(1)}</span>`
    : "—";
  const cve = alert.cve_id ? esc(alert.cve_id) : "—";
  const cwes = alert.cwes && alert.cwes.length ? alert.cwes.map(esc).join(", ") : "—";
  const location = alert.location_path ? esc(alert.location_path) : null;
  const tool = alert.tool_name ? esc(alert.tool_name) : null;
  const typeLabel = alert.alert_type === "code_scanning"
    ? `Code Scanning${tool ? ` (${tool})` : ""}`
    : "Dependabot";
  const dismissedReason = alert.dismissed_reason ? esc(alert.dismissed_reason) : null;
  const dismissedComment = alert.dismissed_comment ? esc(alert.dismissed_comment) : null;

  return `
    <div class="detail-content">
      <div class="detail-header">
        <span class="detail-type-badge">${typeLabel} #${alert.id}</span>
        <span class="badge badge-severity ${sevClass}">${esc(alert.severity)}</span>
        <button class="detail-close-btn" onclick="collapseAllRows()" title="Close">×</button>
      </div>
      <div class="detail-body-text detail-advisory-description">${esc(alert.description) || '<em class="detail-no-body">No advisory description available.</em>'}</div>
      <div class="detail-meta-grid">
        <div class="detail-meta-item">
          <span class="detail-meta-label">CVE ID</span>
          <span>${cve}</span>
        </div>
        <div class="detail-meta-item">
          <span class="detail-meta-label">CVSS Score</span>
          <span>${cvss}</span>
        </div>
        <div class="detail-meta-item">
          <span class="detail-meta-label">CWEs</span>
          <span>${cwes}</span>
        </div>
        <div class="detail-meta-item">
          <span class="detail-meta-label">Package</span>
          <span>${esc(alert.package_name || "—")}</span>
        </div>
        <div class="detail-meta-item">
          <span class="detail-meta-label">Vulnerable</span>
          <span>${esc(alert.vulnerable_version_range || "—")}</span>
        </div>
        <div class="detail-meta-item">
          <span class="detail-meta-label">Patched</span>
          <span>${esc(alert.patched_version || "—")}</span>
        </div>
        <div class="detail-meta-item">
          <span class="detail-meta-label">State</span>
          <span>${esc(alert.state)}</span>
        </div>
        <div class="detail-meta-item">
          <span class="detail-meta-label">Created</span>
          <span>${shortDate(alert.created_at)}</span>
        </div>
        ${location ? `
        <div class="detail-meta-item detail-meta-full">
          <span class="detail-meta-label">Location</span>
          <code>${location}</code>
        </div>` : ""}
        ${dismissedReason ? `
        <div class="detail-meta-item">
          <span class="detail-meta-label">Dismissed Reason</span>
          <span>${dismissedReason}</span>
        </div>` : ""}
        ${dismissedComment ? `
        <div class="detail-meta-item detail-meta-full">
          <span class="detail-meta-label">Dismissed Comment</span>
          <span>${dismissedComment}</span>
        </div>` : ""}
      </div>
      <div class="detail-footer">
        <a href="${esc(alert.html_url)}" target="_blank" rel="noopener noreferrer" class="detail-open-link">Open on GitHub ↗</a>
      </div>
    </div>`;
}
