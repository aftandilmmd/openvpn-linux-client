"use strict";

const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;
const $ = (id) => document.getElementById(id);

// StatusMinor values from openvpn3-linux src/dbus/constants.hpp
const S = {
  UNSET: 0, CFG_REQUIRE_USER: 4, CONN_INIT: 5, CONNECTING: 6, CONNECTED: 7,
  DISCONNECTING: 8, DISCONNECTED: 9, FAILED: 10, AUTH_FAILED: 11, RECONNECTING: 12,
  PAUSING: 13, PAUSED: 14, RESUMING: 15, DONE: 16, SESS_NEW: 17,
  BACKEND_COMPLETED: 18, REMOVED: 19, AUTH_USERPASS: 20, AUTH_CHALLENGE: 21, AUTH_URL: 22,
};
const NEEDS_INPUT = [S.CFG_REQUIRE_USER, S.AUTH_USERPASS, S.AUTH_CHALLENGE];
const ENDED = [S.DISCONNECTED, S.DONE, S.BACKEND_COMPLETED, S.REMOVED, S.FAILED, S.AUTH_FAILED];

let profiles = [];
let sessions = [];
const cards = new Map();          // profile path -> card elements
const traffic = new Map();        // session path -> { t, bytesIn, bytesOut, rateIn, rateOut }
const connectedSince = new Map(); // session path -> ms timestamp
const busy = new Set();           // profile paths with a request in flight
const prompted = new Set();       // "session:minor" already asked for credentials
const openedUrls = new Set();
let modalOpen = false;

// ---------- helpers ----------

function view(s) {
  if (!s) return { kind: "off", label: "Disconnected" };
  const { minor, message } = s.status;
  switch (minor) {
    case S.CONNECTED: return { kind: "on", label: "Connected" };
    case S.PAUSED: return { kind: "paused", label: "Paused" };
    case S.RECONNECTING: return { kind: "busy", label: "Reconnecting…" };
    case S.RESUMING: return { kind: "busy", label: "Resuming…" };
    case S.PAUSING: return { kind: "busy", label: "Pausing…" };
    case S.DISCONNECTING: return { kind: "busy", label: "Disconnecting…" };
    case S.AUTH_URL: return { kind: "busy", label: "Waiting for web sign-in" };
    case S.AUTH_FAILED: return { kind: "err", label: "Authentication failed" };
    case S.FAILED: return { kind: "err", label: message || "Connection failed" };
    default:
      if (NEEDS_INPUT.includes(minor)) return { kind: "busy", label: "Waiting for credentials" };
      if (ENDED.includes(minor)) return { kind: "off", label: "Disconnected" };
      return { kind: "busy", label: "Connecting…" };
  }
}

function sessionFor(profilePath) {
  const mine = sessions.filter((s) => s.config_path === profilePath);
  return mine.find((s) => !ENDED.includes(s.status.minor)) || mine[mine.length - 1];
}

function bytes(n) {
  if (!n) return "0 B";
  const u = ["B", "KB", "MB", "GB", "TB"];
  const i = Math.min(u.length - 1, Math.floor(Math.log(n) / Math.log(1024)));
  return `${(n / 1024 ** i).toFixed(i ? 1 : 0)} ${u[i]}`;
}

function duration(ms) {
  const t = Math.max(0, Math.floor(ms / 1000));
  const h = Math.floor(t / 3600), m = Math.floor((t % 3600) / 60), s = t % 60;
  return [h, m, s].map((x) => String(x).padStart(2, "0")).join(":");
}

function el(tag, cls, text) {
  const e = document.createElement(tag);
  if (cls) e.className = cls;
  if (text !== undefined) e.textContent = text;
  return e;
}

let toastTimer;
function toast(msg) {
  const t = $("toast");
  t.textContent = String(msg);
  t.classList.remove("hidden");
  clearTimeout(toastTimer);
  toastTimer = setTimeout(() => t.classList.add("hidden"), 5000);
}

// ---------- modal ----------

function modal({ title, text = "", fields = [], ok = "OK" }) {
  return new Promise((resolve) => {
    const dlg = $("modal");
    $("modal-title").textContent = title;
    $("modal-text").textContent = text;
    $("modal-ok").textContent = ok;
    const box = $("modal-fields");
    box.replaceChildren();
    const inputs = fields.map((f, i) => {
      const label = el("label", null, f.label);
      const input = el("input");
      input.type = f.hidden ? "password" : "text";
      input.id = `field-${i}`;
      input.autocomplete = "off";
      label.htmlFor = input.id;
      box.append(label, input);
      return input;
    });

    const done = (value) => {
      dlg.close();
      modalOpen = false;
      $("modal-form").onsubmit = null;
      $("modal-cancel").onclick = null;
      dlg.oncancel = null;
      resolve(value);
    };
    $("modal-form").onsubmit = (e) => { e.preventDefault(); done(inputs.map((i) => i.value)); };
    $("modal-cancel").onclick = () => done(null);
    dlg.oncancel = (e) => { e.preventDefault(); done(null); };

    modalOpen = true;
    dlg.showModal();
    (inputs[0] || $("modal-ok")).focus();
  });
}

async function askCredentials(fields, profileName) {
  const answers = await modal({
    title: "Sign in",
    text: profileName ? `"${profileName}" needs your credentials.` : "This connection needs your credentials.",
    fields: fields.map((f) => ({ label: f.description || f.name, hidden: f.hidden })),
    ok: "Connect",
  });
  if (!answers) return null;
  return fields.map((f, i) => ({ kind: f.kind, group: f.group, id: f.id, value: answers[i] }));
}

// ---------- actions ----------

async function handleOutcome(outcome, profileName) {
  while (outcome.result === "needs_input") {
    const values = await askCredentials(outcome.fields, profileName);
    if (!values) {
      await invoke("disconnect", { session: outcome.session }).catch(() => {});
      return;
    }
    outcome = await invoke("submit_input", { session: outcome.session, values });
  }
}

async function run(profilePath, fn) {
  if (busy.has(profilePath)) return;
  busy.add(profilePath);
  render();
  try {
    await fn();
  } catch (e) {
    toast(e);
  } finally {
    busy.delete(profilePath);
    refresh();
  }
}

function toggle(p) {
  run(p.path, async () => {
    const s = sessionFor(p.path);
    const kind = view(s).kind;
    if (s && kind !== "off" && kind !== "err") {
      await invoke("disconnect", { session: s.path });
      return;
    }
    if (s) await invoke("disconnect", { session: s.path }).catch(() => {});
    await handleOutcome(await invoke("connect", { configPath: p.path }), p.name);
  });
}

async function removeProfile(p) {
  const yes = await modal({
    title: "Remove profile?",
    text: `"${p.name}" will be deleted from this computer.`,
    ok: "Remove",
  });
  if (!yes) return;
  run(p.path, async () => {
    const s = sessionFor(p.path);
    if (s && view(s).kind !== "off") await invoke("disconnect", { session: s.path }).catch(() => {});
    await invoke("remove_profile", { path: p.path });
    await loadProfiles();
  });
}

async function importPaths(paths) {
  const files = paths.filter((p) => /\.(ovpn|conf)$/i.test(p));
  if (!files.length) return toast("Only .ovpn and .conf files can be imported.");
  for (const path of files) {
    try {
      await invoke("import_profile", { path });
    } catch (e) {
      toast(e);
    }
  }
  await loadProfiles();
}

// ---------- rendering ----------

const ICON_TRASH = '<svg viewBox="0 0 24 24"><path d="M4 7h16M10 11v6m4-6v6M6 7l1 12a2 2 0 0 0 2 2h6a2 2 0 0 0 2-2l1-12M9 7V4h6v3" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round"/></svg>';

function makeCard(p) {
  const li = el("li", "profile");
  const row = el("div", "row");
  const sw = el("button", "switch");
  sw.setAttribute("role", "switch");
  sw.setAttribute("aria-label", "Connect");
  sw.onclick = () => toggle(p);
  const info = el("div", "info");
  const name = el("div", "name");
  const state = el("div", "state");
  info.append(name, state);
  const del = el("button", "icon-btn");
  del.title = "Remove profile";
  del.innerHTML = ICON_TRASH;
  del.onclick = () => removeProfile(p);
  row.append(sw, info, del);

  const stats = el("div", "stats hidden");
  const stat = (label, wide) => {
    const box = el("div", wide ? "stat wide" : "stat");
    const v = el("div", "v");
    box.append(el("div", "k", label), v);
    stats.append(box);
    return v;
  };
  const fields = {
    duration: stat("Duration"), server: stat("Server"),
    down: stat("Download"), up: stat("Upload"),
    received: stat("Received"), sent: stat("Sent"),
    device: stat("Interface", true),
  };

  const actions = el("div", "session-actions hidden");
  const pauseBtn = el("button", "ghost", "Pause");
  const restartBtn = el("button", "ghost", "Reconnect");
  actions.append(pauseBtn, restartBtn);

  li.append(row, stats, actions);
  return { li, sw, name, state, stats, fields, actions, pauseBtn, restartBtn, profile: p };
}

function render() {
  const list = $("profiles");
  const seen = new Set();
  let anyOn = false, anyBusy = false;

  for (const p of profiles) {
    seen.add(p.path);
    let c = cards.get(p.path);
    if (!c) {
      c = makeCard(p);
      cards.set(p.path, c);
    }
    c.profile = p;
    list.append(c.li); // keeps DOM order in sync with `profiles`

    const s = sessionFor(p.path);
    const v = busy.has(p.path) && (!s || view(s).kind === "off" || view(s).kind === "err")
      ? { kind: "busy", label: "Starting…" }
      : view(s);
    anyOn ||= v.kind === "on";
    anyBusy ||= v.kind === "busy";

    c.li.className = `profile ${v.kind}`;
    c.name.textContent = p.name;
    c.state.textContent = v.label;
    c.sw.setAttribute("aria-checked", String(v.kind !== "off" && v.kind !== "err"));

    const live = s && (v.kind === "on" || v.kind === "paused");
    c.stats.classList.toggle("hidden", !live);
    c.actions.classList.toggle("hidden", !live);
    if (live) {
      const tr = traffic.get(s.path) || {};
      const since = connectedSince.get(s.path) || s.created * 1000;
      c.fields.duration.textContent = duration(Date.now() - since);
      c.fields.server.textContent = s.server ? `${s.server.ip}:${s.server.port}/${s.server.proto}` : "—";
      c.fields.down.textContent = `${bytes(tr.rateIn || 0)}/s`;
      c.fields.up.textContent = `${bytes(tr.rateOut || 0)}/s`;
      c.fields.received.textContent = bytes(s.stats.BYTES_IN || 0);
      c.fields.sent.textContent = bytes(s.stats.BYTES_OUT || 0);
      c.fields.device.textContent = s.device || "—";
      const paused = v.kind === "paused";
      c.pauseBtn.textContent = paused ? "Resume" : "Pause";
      c.pauseBtn.onclick = () => run(p.path, () => invoke(paused ? "resume" : "pause", { session: s.path }));
      c.restartBtn.onclick = () => run(p.path, () => invoke("restart", { session: s.path }));
    }
  }

  for (const [path, c] of cards) {
    if (!seen.has(path)) {
      c.li.remove();
      cards.delete(path);
    }
  }

  $("empty").classList.toggle("hidden", profiles.length > 0 || !$("backend-error").classList.contains("hidden"));
  const pill = $("summary");
  pill.className = `pill${anyOn ? " on" : anyBusy ? " busy" : ""}`;
  pill.textContent = anyOn ? "Connected" : anyBusy ? "Connecting" : "Disconnected";
}

// ---------- polling ----------

function trackTraffic() {
  const now = Date.now();
  for (const s of sessions) {
    const bytesIn = s.stats.BYTES_IN || 0, bytesOut = s.stats.BYTES_OUT || 0;
    const prev = traffic.get(s.path);
    const next = { t: now, bytesIn, bytesOut, rateIn: 0, rateOut: 0 };
    if (prev && now > prev.t) {
      const dt = (now - prev.t) / 1000;
      next.rateIn = Math.max(0, (bytesIn - prev.bytesIn) / dt);
      next.rateOut = Math.max(0, (bytesOut - prev.bytesOut) / dt);
    }
    traffic.set(s.path, next);
    if (s.status.minor === S.CONNECTED) {
      // The app may open on an already running tunnel; fall back to session start.
      if (!connectedSince.has(s.path)) connectedSince.set(s.path, s.created ? s.created * 1000 : now);
    } else if (s.status.minor !== S.PAUSED) {
      connectedSince.delete(s.path);
    }
  }
}

async function attend() {
  for (const s of sessions) {
    const p = profiles.find((x) => x.path === s.config_path);
    const { minor, message } = s.status;

    if (minor === S.AUTH_URL && /^https?:\/\//.test(message) && !openedUrls.has(message)) {
      openedUrls.add(message);
      toast("Finish signing in with your browser.");
      invoke("open_url", { url: message }).catch(toast);
    }

    const key = `${s.path}:${minor}`;
    if (NEEDS_INPUT.includes(minor) && !modalOpen && !prompted.has(key) && !(p && busy.has(p.path))) {
      prompted.add(key);
      const path = p ? p.path : s.path;
      run(path, async () => {
        const fields = await invoke("pending_input", { session: s.path });
        if (fields.length) await handleOutcome({ result: "needs_input", session: s.path, fields }, p && p.name);
      });
    }
  }
}

let refreshing = false;
async function refresh() {
  if (refreshing) return;
  refreshing = true;
  try {
    sessions = await invoke("list_sessions");
    trackTraffic();
    render();
    attend();
  } catch (e) {
    // Shown once by the backend check.
  } finally {
    refreshing = false;
  }
}

async function loadProfiles() {
  try {
    profiles = await invoke("list_profiles");
  } catch (e) {
    toast(e);
  }
  render();
}

// ---------- log ----------

const MAX_LOG_LINES = 2000;
const logLines = [];
function log(line) {
  const time = new Date().toLocaleTimeString();
  logLines.push(`${time}  ${line}`);
  if (logLines.length > MAX_LOG_LINES) logLines.splice(0, logLines.length - MAX_LOG_LINES);
  const pre = $("log");
  const atBottom = pre.parentElement.scrollTop + pre.parentElement.clientHeight >= pre.parentElement.scrollHeight - 30;
  pre.textContent = logLines.join("\n");
  if (atBottom) pre.parentElement.scrollTop = pre.parentElement.scrollHeight;
}

function sessionLabel(path) {
  const s = sessions.find((x) => x.path === path);
  return s ? s.config_name : "";
}

// ---------- updates ----------

const UPDATE_CHECK_MS = 6 * 60 * 60 * 1000;
let installing = false;

async function checkUpdate() {
  if (installing) return;
  let info;
  try {
    info = await invoke("check_update");
  } catch {
    return; // offline, or a build without updater support
  }
  if (!info) return;
  $("update-text").textContent = `Version ${info.version} is available`;
  $("update-btn").textContent = "Update";
  $("update-btn").disabled = false;
  $("update").classList.remove("hidden");
}

async function installUpdate() {
  installing = true;
  const btn = $("update-btn");
  btn.disabled = true;
  btn.textContent = "Downloading…";
  try {
    // Restarts the app when done; VPN connections stay up.
    await invoke("install_update");
  } catch (e) {
    toast(`Update failed: ${e}`);
    btn.disabled = false;
    btn.textContent = "Retry";
    installing = false;
    checkUpdate();
  }
}

// ---------- startup ----------

async function checkBackend() {
  const box = $("backend-error");
  try {
    const version = await invoke("backend_version");
    $("backend-version").textContent = `openvpn3 ${version}`;
    box.classList.add("hidden");
    return true;
  } catch (e) {
    await showSetup(String(e));
    return false;
  }
}

// Shown when the openvpn3 services are missing: install them in one click.
async function showSetup(error) {
  const box = $("backend-error");
  const info = await invoke("setup_info").catch(() => ({ automatic: false, distro: "this system", help_url: "" }));
  const status = el("div", "setup-status");
  const actions = el("div", "setup-actions");

  if (info.automatic) {
    const btn = el("button", "primary", "Install OpenVPN 3");
    btn.onclick = async () => {
      btn.disabled = true;
      btn.textContent = "Installing…";
      status.textContent = "Enter your password when asked. This can take a minute.";
      try {
        await invoke("install_openvpn3");
        status.textContent = "";
        if (await checkBackend()) startPolling();
      } catch (e) {
        status.textContent = String(e);
        btn.disabled = false;
        btn.textContent = "Try again";
      }
    };
    actions.append(btn);
  }
  if (info.help_url) {
    const link = el("button", "ghost", info.automatic ? "Install manually" : "Installation guide");
    link.onclick = () => invoke("open_url", { url: info.help_url }).catch(toast);
    actions.append(link);
  }

  box.replaceChildren(
    el("strong", null, "OpenVPN 3 is needed"),
    el("div", null, info.automatic
      ? "This app connects through OpenVPN 3, which isn't installed yet. It can be installed for you."
      : `This app connects through OpenVPN 3. Automatic installation isn't available for ${info.distro}; install the openvpn3 package, then reopen the app.`),
    actions,
    status,
    el("code", null, error),
  );
  box.className = "notice setup";
  render();
}

let polling = false;
async function startPolling() {
  if (polling) return;
  polling = true;
  await loadProfiles();
  await refresh();
  setInterval(refresh, 1000);
  // Profiles can also be added with the openvpn3 command line tool.
  setInterval(loadProfiles, 10000);
}

function setupTabs() {
  for (const tab of document.querySelectorAll(".tab")) {
    tab.onclick = () => {
      for (const t of document.querySelectorAll(".tab")) t.classList.toggle("active", t === tab);
      $("tab-profiles").classList.toggle("hidden", tab.dataset.tab !== "profiles");
      $("tab-log").classList.toggle("hidden", tab.dataset.tab !== "log");
      $("import").classList.toggle("hidden", tab.dataset.tab !== "profiles");
    };
  }
}

async function setupDragDrop() {
  const webview = window.__TAURI__.webview?.getCurrentWebview?.();
  if (!webview) return;
  await webview.onDragDropEvent((e) => {
    const { type } = e.payload;
    $("drop").classList.toggle("hidden", !(type === "enter" || type === "over"));
    if (type === "drop") importPaths(e.payload.paths || []);
  });
}

async function main() {
  setupTabs();
  $("import").onclick = async () => {
    try {
      if (await invoke("pick_and_import")) await loadProfiles();
    } catch (e) {
      toast(e);
    }
  };
  $("log-clear").onclick = () => { logLines.length = 0; $("log").textContent = ""; };
  $("log-copy").onclick = () => navigator.clipboard.writeText(logLines.join("\n")).then(() => toast("Log copied."));

  const repo = await invoke("repo_url");
  $("repo-link").title = repo;
  $("repo-link").onclick = (e) => { e.preventDefault(); invoke("open_url", { url: repo }).catch(toast); };

  await listen("ovpn-signal", ({ payload: ev }) => {
    const who = sessionLabel(ev.path);
    const prefix = who ? `[${who}] ` : "";
    if (ev.member === "Log") log(prefix + ev.message.trimEnd());
    else if (ev.member === "StatusChange") {
      if (ev.message) log(`${prefix}${ev.message}`);
      refresh();
    } else refresh();
  });
  setupDragDrop().catch(() => {});

  $("update-btn").onclick = installUpdate;
  await listen("update-progress", ({ payload: p }) => {
    $("update-btn").textContent = p.total
      ? `Downloading ${Math.round((p.downloaded / p.total) * 100)}%`
      : `Downloading ${bytes(p.downloaded)}`;
    if (p.total && p.downloaded >= p.total) $("update-btn").textContent = "Installing…";
  });
  checkUpdate();
  setInterval(checkUpdate, UPDATE_CHECK_MS);

  if (await checkBackend()) startPolling();
}

main();
