// SPDX-License-Identifier: Apache-2.0
// Voz frontend — thin: forwards intents to the engine, renders its events.
// Untrusted text (transcripts, titles) is set via textContent only (no innerHTML).
const T = window.__TAURI__ || {};
const invoke = (T.core && T.core.invoke) || (async () => {});
const listen = (T.event && T.event.listen) || (async () => () => {});

const $ = (id) => document.getElementById(id);
let source = 'both';
let recording = false;
let timerStart = 0, timerTimer = null, levelTimer = null;

// ---- waveform ----
const wave = $('wave');
const BARS = 30;
for (let i = 0; i < BARS; i++) wave.appendChild(document.createElement('i'));
const bars = [...wave.querySelectorAll('i')];
// Force the transient elements hidden immediately (before any event), so the idle
// view is correct regardless of load order.
function hideTransient() {
  const pb = $('procbar'), r = $('result'), p = $('btn-pause'), c = $('btn-cancel');
  if (pb) pb.style.display = 'none';
  if (r) r.style.display = 'none';
  if (p) p.style.visibility = 'hidden';
  if (c) c.style.visibility = 'hidden';
}
hideTransient();

function flatWave() { wave.classList.add('idle'); bars.forEach(b => b.style.height = '6px'); }
function animateWave(level) {
  wave.classList.remove('idle');
  bars.forEach((b, i) => {
    const base = 6 + level * 70;
    const jitter = Math.sin(Date.now() / 90 + i) * 0.5 + 0.5;
    b.style.height = Math.max(4, Math.min(44, base * (0.4 + jitter))) + 'px';
  });
}
flatWave();

// ---- state pill ----
function setPill(text, cls) {
  const p = $('statepill');
  p.className = 'statepill' + (cls ? ' ' + cls : '');
  $('statetext').textContent = text;
}

// ---- timer ----
function fmt(s) { const m = Math.floor(s / 60), ss = s % 60; return `${String(m).padStart(2,'0')}:${String(ss).padStart(2,'0')}`; }
function startTimer() {
  timerStart = Date.now();
  $('timer').classList.remove('idle');
  timerTimer = setInterval(() => $('timer').textContent = fmt(Math.floor((Date.now() - timerStart) / 1000)), 500);
}
function stopTimer() { clearInterval(timerTimer); $('timer').textContent = '00:00'; $('timer').classList.add('idle'); }

// ---- record controls ----
const recIcon = '<svg class="mic" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M12 1a3 3 0 0 0-3 3v8a3 3 0 0 0 6 0V4a3 3 0 0 0-3-3z"/><path d="M19 10v2a7 7 0 0 1-14 0v-2M12 19v4M8 23h8"/></svg>';
const stopIcon = '<span class="stop"></span>';

function enterRecording() {
  recording = true;
  setPill('Recording', 'live');
  $('recbtn').classList.add('recording');
  $('recbtn').innerHTML = stopIcon;
  $('btn-pause').style.visibility = 'visible';
  $('btn-cancel').style.visibility = 'visible';
  $('hint').innerHTML = '<b>Stop</b> hands off to the background — you stay free to record';
  $('result').style.display = 'none';
  startTimer();
  levelTimer = setInterval(async () => {
    try { const [m, s] = await invoke('get_level'); animateWave(Math.max(m, s)); } catch (e) {}
  }, 90);
}
function leaveRecording() {
  recording = false;
  setPill('Ready');
  $('recbtn').classList.remove('recording');
  $('recbtn').innerHTML = recIcon;
  $('btn-pause').style.visibility = 'hidden';
  $('btn-cancel').style.visibility = 'hidden';
  $('hint').textContent = 'Click the mic, or the tray icon, to record';
  stopTimer();
  clearInterval(levelTimer);
  flatWave();
}

$('recbtn').onclick = async () => {
  try { recording ? await invoke('stop') : await invoke('start', { source }); }
  catch (e) { setPill('Error'); console.error(e); }
};
$('btn-cancel').onclick = () => invoke('cancel').catch(() => {});
$('btn-pause').onclick = () => invoke('pause').catch(() => {});

// ---- source pill ----
$('srcpill').querySelectorAll('button').forEach(btn => {
  btn.onclick = () => {
    if (recording) return;
    source = btn.dataset.src;
    $('srcpill').querySelectorAll('button').forEach(b => b.classList.toggle('on', b === btn));
  };
});

// ---- nav ----
function showView(name) {
  document.querySelectorAll('.view').forEach(v => v.classList.toggle('on', v.id === 'view-' + name));
  document.querySelectorAll('.nav a').forEach(a => a.classList.toggle('active', a.dataset.view === name));
  if (name === 'history') loadHistory();
  if (name === 'settings') loadSettings();
}
document.querySelectorAll('.nav a').forEach(a => a.onclick = () => showView(a.dataset.view));
$('procbar').onclick = () => showView('history');

// ---- window controls (frameless: minimize / hide-to-tray) ----
function appWindow() { try { return window.__TAURI__.window.getCurrentWindow(); } catch (e) { return null; } }
$('btn-min').onclick = () => { const w = appWindow(); if (w) w.minimize(); };
$('btn-hide').onclick = () => { const w = appWindow(); if (w) w.hide(); };

// ---- history ----
async function loadHistory() {
  const list = $('histlist');
  list.textContent = '';
  let rows = [];
  try { rows = await invoke('get_history'); } catch (e) {}
  if (!rows.length) { const d = document.createElement('div'); d.className = 'hint-line'; d.textContent = 'No recordings yet.'; list.appendChild(d); return; }
  for (const r of rows) {
    const item = document.createElement('div'); item.className = 'hitem';
    item.title = 'Open in your editor / Obsidian';
    item.onclick = () => { if (r.refined_path) invoke('open_path', { path: r.refined_path }).catch(() => {}); };
    const when = document.createElement('div'); when.className = 'when'; when.textContent = (r.created || '').slice(11, 16) || '—';
    const body = document.createElement('div'); body.className = 'h-body';
    const t = document.createElement('div'); t.className = 'h-t'; t.textContent = r.title || '(untitled)';
    const m = document.createElement('div'); m.className = 'h-m';
    const b1 = document.createElement('span'); const bb = document.createElement('b'); bb.textContent = r.source || ''; b1.appendChild(bb);
    const b2 = document.createElement('span'); b2.textContent = `${r.words || 0}w`;
    const b3 = document.createElement('span'); b3.textContent = r.backend || '';
    m.append(b1, b2, b3); body.append(t, m); item.append(when, body); list.appendChild(item);
  }
}

// ---- settings ----
let currentSettings = null;
const BACKENDS = ['claude_code', 'codex', 'ollama', 'claude_api', 'none'];
const BACKEND_LABEL = { claude_code: 'Claude Code', codex: 'Codex CLI', ollama: 'Local LLM (Ollama)', claude_api: 'Claude API', none: 'None (raw only)' };

async function persistSettings() {
  if (!currentSettings) return;
  try { await invoke('update_settings', { settings: currentSettings }); } catch (e) { console.error(e); }
  loadSettings();
}

async function loadSettings() {
  try { currentSettings = await invoke('get_settings'); } catch (e) { return; }
  const s = currentSettings;
  $('set-savedir').textContent = s.general?.save_dir ?? '—';
  $('set-model').textContent = s.transcription?.model ?? '—';
  $('set-backend').textContent = BACKEND_LABEL[s.refine?.backend] ?? (s.refine?.backend ?? '—');
  document.querySelectorAll('#set-source-seg span').forEach(sp =>
    sp.classList.toggle('on', sp.dataset.src === s.sources?.default_source));
  // reflect the default source on the Record screen too (while idle)
  if (!recording && s.sources?.default_source) {
    source = s.sources.default_source;
    $('srcpill').querySelectorAll('button').forEach(b => b.classList.toggle('on', b.dataset.src === source));
  }
}

// change save folder via the native directory picker
$('set-savedir-btn').onclick = async () => {
  try {
    const dir = await window.__TAURI__.dialog.open({ directory: true, title: 'Choose save folder (e.g. your Obsidian vault)' });
    if (dir && currentSettings) { currentSettings.general.save_dir = dir; await persistSettings(); }
  } catch (e) { console.error(e); }
};
// cycle the refine backend
$('set-backend-btn').onclick = async () => {
  if (!currentSettings) return;
  const i = BACKENDS.indexOf(currentSettings.refine.backend);
  currentSettings.refine.backend = BACKENDS[(i + 1) % BACKENDS.length];
  await persistSettings();
};
// pick the default source
document.querySelectorAll('#set-source-seg span').forEach(sp => sp.onclick = async () => {
  if (!currentSettings) return;
  currentSettings.sources.default_source = sp.dataset.src;
  await persistSettings();
});

// ---- engine events ----
listen('voz://event', (e) => {
  const p = e.payload || {};
  switch (p.type) {
    case 'recState':
      if (p.state === 'Recording') enterRecording();
      else if (p.state === 'Idle') leaveRecording();
      else if (p.state === 'Paused') setPill('Paused');
      break;
    case 'jobState':
      if (p.state === 'Transcribing') { $('procbar').style.display = 'flex'; $('proctext').innerHTML = '<b>Transcribing</b>'; }
      else if (p.state === 'Refining') { $('procbar').style.display = 'flex'; $('proctext').innerHTML = '<b>Refining</b>'; }
      else if (p.state === 'Done') { $('procbar').style.display = 'none'; }
      else if (p.state === 'Failed') { $('procbar').style.display = 'none'; setPill('Error'); }
      break;
    case 'refineDone': {
      $('result').style.display = 'block';
      $('result-backend').textContent = p.lossless_ok ? 'lossless' : 'review';
      $('result-text').textContent = p.refined || '';
      break;
    }
    case 'saved': setPill('Saved', ''); break;
    case 'modelProgress': {
      const pct = Math.round((p.pct || 0) * 100);
      setPill(pct >= 100 ? 'Ready' : 'Model ' + pct + '%');
      break;
    }
    case 'jobFailed': setPill('Error'); break;
    case 'notify': /* desktop notification handled natively later */ break;
  }
});

// initial state — force a clean idle UI, then reconcile with the engine.
function resetIdleUI() {
  leaveRecording();
  $('procbar').style.display = 'none';
  $('result').style.display = 'none';
}
resetIdleUI();
invoke('get_state').then(s => { if (String(s).includes('Recording')) enterRecording(); }).catch(() => {});
loadSettings();
