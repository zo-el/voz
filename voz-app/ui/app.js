// SPDX-License-Identifier: Apache-2.0
// Voz frontend — thin: forwards intents to the engine, renders its events.
// Untrusted text (transcripts, titles) is set via textContent only (no innerHTML).
const T = window.__TAURI__ || {};
const invoke = (T.core && T.core.invoke) || (async () => {});
const listen = (T.event && T.event.listen) || (async () => () => {});

const $ = (id) => document.getElementById(id);
let source = 'both';
let recording = false, paused = false;
let timerStart = 0, elapsedBase = 0, timerTimer = null, levelTimer = null;
let waveAmp = 0; // smoothed 0..1 wave amplitude, eased toward the live input level

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

function setBars(px) { bars.forEach(b => b.style.height = px + 'px'); }
function flatWave() { wave.classList.remove('paused'); wave.classList.add('idle'); waveAmp = 0; setBars(6); }
function pausedWave() { wave.classList.remove('idle'); wave.classList.add('paused'); waveAmp = 0; setBars(6); }
// Drive the bars from the live input level. RMS is small for speech, so apply a
// sqrt curve + gain: true silence stays flat (clearly "nothing"), normal speech
// is lively, loud input fills the meter. Ease toward the target so it rises/falls
// smoothly, and scale the per-bar jitter by the amplitude so a quiet/idle signal
// doesn't wiggle.
function animateWave(level) {
  wave.classList.remove('idle', 'paused');
  const target = Math.min(1, Math.sqrt(Math.max(0, level)) * 2.2);
  waveAmp += (target - waveAmp) * 0.3;
  const now = Date.now();
  bars.forEach((b, i) => {
    const jitter = Math.sin(now / 120 + i * 0.7) * 0.5 + 0.5; // 0..1
    const h = 4 + waveAmp * (10 + jitter * 30);
    b.style.height = Math.max(3, Math.min(44, h)) + 'px';
  });
}
flatWave();

// ---- state pill + toast ----
function setPill(text, cls) {
  const p = $('statepill');
  p.className = 'statepill' + (cls ? ' ' + cls : '');
  $('statetext').textContent = text;
}
function toast(msg) {
  const t = $('toast'); if (!t) return;
  t.textContent = msg; t.classList.add('on');
  clearTimeout(t._timer); t._timer = setTimeout(() => t.classList.remove('on'), 4500);
}
function friendlyError(m) {
  m = m || 'Something went wrong.';
  if (/no model/i.test(m)) return 'No transcription model installed — open Settings ▸ Model.';
  if (/pw-record|default sink|monitor|source|audio/i.test(m)) return 'Couldn’t access audio — check your microphone / output device.';
  if (/refine|claude|codex|ollama/i.test(m)) return 'AI cleanup unavailable — saved the raw transcript.';
  if (/storage|disk|write|permission|read-only/i.test(m)) return 'Couldn’t save — check the save folder is writable.';
  return m;
}

// ---- timer ----
function fmt(s) { const m = Math.floor(s / 60), ss = s % 60; return `${String(m).padStart(2,'0')}:${String(ss).padStart(2,'0')}`; }
function renderTimer() { $('timer').textContent = fmt(Math.floor(elapsedBase + (Date.now() - timerStart) / 1000)); }
function startTimer() {
  clearInterval(timerTimer); // never stack intervals across re-entry
  elapsedBase = 0; timerStart = Date.now();
  $('timer').classList.remove('idle');
  renderTimer();
  timerTimer = setInterval(renderTimer, 500);
}
function pauseTimer() {
  clearInterval(timerTimer);
  elapsedBase += (Date.now() - timerStart) / 1000; // bank the active span; display freezes
  $('timer').textContent = fmt(Math.floor(elapsedBase));
}
function resumeTimer() {
  clearInterval(timerTimer);
  timerStart = Date.now();
  renderTimer();
  timerTimer = setInterval(renderTimer, 500);
}
function stopTimer() { clearInterval(timerTimer); elapsedBase = 0; $('timer').textContent = '00:00'; $('timer').classList.add('idle'); }

// ---- record controls ----
const recIcon = '<svg class="mic" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M12 1a3 3 0 0 0-3 3v8a3 3 0 0 0 6 0V4a3 3 0 0 0-3-3z"/><path d="M19 10v2a7 7 0 0 1-14 0v-2M12 19v4M8 23h8"/></svg>';
const stopIcon = '<span class="stop"></span>';
const pauseIcon = '<svg viewBox="0 0 24 24" fill="currentColor"><rect x="6" y="5" width="4" height="14" rx="1.5"/><rect x="14" y="5" width="4" height="14" rx="1.5"/></svg>';
const playIcon = '<svg viewBox="0 0 24 24" fill="currentColor"><path d="M8 5v14l11-7z"/></svg>';
function setPauseButton(isPaused) {
  const b = $('btn-pause');
  b.innerHTML = isPaused ? playIcon : pauseIcon;
  b.title = isPaused ? 'Resume' : 'Pause';
}

// The level meter polls the live input and drives the waveform. Kept as its own
// lifecycle (not stacked) so pause can stop it and resume can restart it without
// orphaning intervals — a leaked one would animate the wave forever and peg a core.
function startLevelMeter() {
  clearInterval(levelTimer);
  levelTimer = setInterval(async () => {
    try { const [m, s] = await invoke('get_level'); animateWave(Math.max(m, s)); } catch (e) {}
  }, 90);
}
function stopLevelMeter() { clearInterval(levelTimer); }

function enterRecording() {
  // Idempotent: `start` emits RecState(Recording); the guard stops a duplicate
  // start from stacking timers. Resume is handled separately (resumeRecording).
  if (recording) return;
  recording = true;
  paused = false;
  setPill('Recording', 'live');
  $('recbtn').classList.add('recording');
  $('recbtn').innerHTML = stopIcon;
  setPauseButton(false);
  $('btn-pause').style.visibility = 'visible';
  $('btn-cancel').style.visibility = 'visible';
  $('hint').innerHTML = '<b>Stop</b> hands off to the background — you stay free to record';
  $('result').style.display = 'none';
  startTimer();
  startLevelMeter();
}
function pauseRecording() {
  paused = true;
  pauseTimer();       // freeze elapsed time (backend excludes the paused span too)
  stopLevelMeter();   // freeze the waveform — no live input is being captured
  pausedWave();
  setPill('Paused', 'paused');
  setPauseButton(true);
  $('hint').innerHTML = '<b>Paused</b> — tap ▶ to resume, or Stop to finish';
}
function resumeRecording() {
  paused = false;
  resumeTimer();
  setPill('Recording', 'live');
  setPauseButton(false);
  $('hint').innerHTML = '<b>Stop</b> hands off to the background — you stay free to record';
  startLevelMeter();
}
function leaveRecording() {
  recording = false;
  paused = false;
  setPill('Ready');
  $('recbtn').classList.remove('recording');
  $('recbtn').innerHTML = recIcon;
  $('btn-pause').style.visibility = 'hidden';
  $('btn-cancel').style.visibility = 'hidden';
  setPauseButton(false);
  $('hint').textContent = 'Click the mic, or the tray icon, to record';
  stopTimer();
  stopLevelMeter();
  flatWave();
  const pt = $('partial'); pt.style.display = 'none'; pt.textContent = '';
}
function showPartial(text) {
  if (!recording || !text) return;
  const pt = $('partial');
  pt.textContent = text;
  pt.style.display = 'block';
  pt.scrollTop = pt.scrollHeight;
  const words = (text.trim().match(/\S+/g) || []).length;
  $('hint').textContent = `${words} word${words === 1 ? '' : 's'} transcribed so far`;
}

$('recbtn').onclick = async () => {
  try { recording ? await invoke('stop') : await invoke('start', { source }); }
  catch (e) {
    setPill('Ready');
    toast('Couldn’t start recording — check your audio device, or pick Mic in the source selector.');
    console.error(e);
  }
};
$('btn-cancel').onclick = () => invoke('cancel').catch(() => {});
$('btn-pause').onclick = () => invoke(paused ? 'resume' : 'pause').catch(() => {});

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
  // note/models are sub-views; keep their parent tab highlighted
  const tab = name === 'note' ? 'history' : name === 'models' ? 'settings' : name;
  document.querySelectorAll('.nav a').forEach(a => a.classList.toggle('active', a.dataset.view === tab));
  if (name === 'history') loadHistory();
  if (name === 'settings') loadSettings();
  if (name === 'models') loadModels();
}
document.querySelectorAll('.nav a').forEach(a => a.onclick = () => showView(a.dataset.view));
$('procbar').onclick = () => showView('history');

// ---- window controls (frameless: minimize / hide-to-tray) ----
function appWindow() { try { return window.__TAURI__.window.getCurrentWindow(); } catch (e) { return null; } }
$('btn-min').onclick = () => { const w = appWindow(); if (w) w.minimize(); };
$('btn-hide').onclick = () => { const w = appWindow(); if (w) w.hide(); };

// ---- keyboard shortcuts + accessibility ----
const A11Y_SEL = '.ctrl-sm, .seg span, .srcpill button, .toggle, .back, .nav a, .hitem';
// Make custom (non-native) controls reachable by Tab and announced as buttons.
function a11yInit() {
  document.querySelectorAll(A11Y_SEL).forEach(el => {
    if (el.tagName !== 'BUTTON' && !el.getAttribute('role')) el.setAttribute('role', 'button');
    if (!el.hasAttribute('tabindex')) el.tabIndex = 0;
  });
}
a11yInit();
document.addEventListener('keydown', (e) => {
  if (e.target && (e.target.tagName === 'INPUT' || e.target.tagName === 'TEXTAREA')) return;
  if ($('onboard').classList.contains('on')) return;
  // Enter activates whatever custom control is focused (Space stays = record).
  if (e.key === 'Enter' && e.target?.matches?.(A11Y_SEL)) { e.preventDefault(); e.target.click(); return; }
  const current = document.querySelector('.view.on')?.id;
  if (e.key === 'Escape') {
    if (current === 'view-note') showView('history');
    else if (current === 'view-models') showView('settings');
    else { const w = appWindow(); if (w) w.hide(); }
  } else if (e.code === 'Space' && current === 'view-record' && e.target === document.body) {
    e.preventDefault(); $('recbtn').click();
  }
});

// ---- history ----
async function loadHistory(query) {
  const list = $('histlist');
  list.textContent = '';
  let rows = [];
  try { rows = await invoke('get_history', { query: query || null }); } catch (e) {}
  if (!rows.length) {
    const d = document.createElement('div'); d.className = 'hint-line';
    d.textContent = (query && query.trim()) ? `No matches for “${query.trim()}”.` : 'No recordings yet.';
    list.appendChild(d); return;
  }
  for (const r of rows) {
    const item = document.createElement('div'); item.className = 'hitem';
    item.title = 'Open note';
    item.onclick = () => { if (r.refined_path) openNote(r.refined_path); };
    const when = document.createElement('div'); when.className = 'when'; when.textContent = (r.created || '').slice(11, 16) || '—';
    const body = document.createElement('div'); body.className = 'h-body';
    const t = document.createElement('div'); t.className = 'h-t'; t.textContent = r.title || '(untitled)';
    const m = document.createElement('div'); m.className = 'h-m';
    const b1 = document.createElement('span'); const bb = document.createElement('b'); bb.textContent = r.source || ''; b1.appendChild(bb);
    const b2 = document.createElement('span'); b2.textContent = `${r.words || 0}w`;
    const b3 = document.createElement('span'); b3.textContent = r.backend || '';
    m.append(b1, b2, b3); body.append(t, m); item.append(when, body); list.appendChild(item);
  }
  a11yInit();
}
// search box → full-text search over titles + transcript bodies (debounced)
let searchTimer = null;
$('searchbox').oninput = () => {
  clearTimeout(searchTimer);
  searchTimer = setTimeout(() => loadHistory($('searchbox').value), 180);
};
// import an existing audio/video file -> same transcribe pipeline
$('hist-import').onclick = async () => {
  try {
    const path = await window.__TAURI__.dialog.open({
      multiple: false,
      filters: [{ name: 'Audio / Video', extensions: ['wav', 'mp3', 'm4a', 'aac', 'ogg', 'opus', 'flac', 'mp4', 'mkv', 'webm', 'mov'] }],
    });
    if (path) { toast('Importing… transcript will appear here'); await invoke('import_audio', { path }); }
  } catch (e) { toast('Import failed'); }
};

// ---- settings ----
let currentSettings = null;
const BACKENDS = ['claude_code', 'codex', 'ollama', 'claude_api', 'none'];
const BACKEND_LABEL = { claude_code: 'Claude Code', codex: 'Codex CLI', ollama: 'Local LLM (Ollama)', claude_api: 'Claude API', none: 'None (raw only)' };

async function persistSettings() {
  if (!currentSettings) return;
  try { await invoke('update_settings', { settings: currentSettings }); } catch (e) { console.error(e); }
  loadSettings();
}

function segSet(id, value) {
  document.querySelectorAll('#' + id + ' span').forEach(sp =>
    sp.classList.toggle('on', Object.values(sp.dataset)[0] === value));
}

async function loadSettings() {
  try { currentSettings = await invoke('get_settings'); } catch (e) { return; }
  const s = currentSettings;
  $('set-savedir').textContent = s.general?.save_dir ?? '—';
  $('set-model').textContent = s.transcription?.model ?? '—';
  $('set-backend').textContent = BACKEND_LABEL[s.refine?.backend] ?? (s.refine?.backend ?? '—');
  segSet('set-source-seg', s.sources?.default_source);
  segSet('set-accel-seg', s.transcription?.accel);
  invoke('get_acceleration').then(d => { if (d) $('set-accel-now').textContent = 'Now: ' + d; }).catch(() => {});
  reflectStyle(s.refine?.style);
  $('set-keepaudio').classList.toggle('on', !!s.general?.keep_audio);
  $('set-hotkey-keys').textContent = s.general?.hotkey || 'Ctrl+Super+Space';
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
$('set-backend-btn').onclick = async () => {
  if (!currentSettings) return;
  const i = BACKENDS.indexOf(currentSettings.refine.backend);
  currentSettings.refine.backend = BACKENDS[(i + 1) % BACKENDS.length];
  await persistSettings();
};
document.querySelectorAll('#set-source-seg span').forEach(sp => sp.onclick = async () => {
  if (!currentSettings) return; currentSettings.sources.default_source = sp.dataset.src; await persistSettings();
});
document.querySelectorAll('#set-accel-seg span').forEach(sp => sp.onclick = async () => {
  if (!currentSettings) return; currentSettings.transcription.accel = sp.dataset.a; await persistSettings();
  // Reflect what's actually in effect, so picking e.g. CUDA on a CPU build is honest.
  invoke('get_acceleration').then(d => { if (d) $('set-accel-now').textContent = 'Now: ' + d; }).catch(() => {});
});
const DEFAULT_CUSTOM_PROMPT = 'Summarize the transcript as clear bullet points, keeping every name, number, and decision.';
function reflectStyle(style) {
  const isCustom = style && typeof style === 'object' && 'custom' in style;
  segSet('set-style-seg', isCustom ? 'custom' : (typeof style === 'string' ? style : 'adaptive'));
  $('set-custom-row').style.display = isCustom ? 'block' : 'none';
  if (isCustom) $('set-custom-prompt').value = style.custom || '';
}
document.querySelectorAll('#set-style-seg span').forEach(sp => sp.onclick = async () => {
  if (!currentSettings) return;
  if (sp.dataset.st === 'custom') {
    const text = ($('set-custom-prompt').value || '').trim() || DEFAULT_CUSTOM_PROMPT;
    currentSettings.refine.style = { custom: text };
  } else {
    currentSettings.refine.style = sp.dataset.st;
  }
  reflectStyle(currentSettings.refine.style);
  await persistSettings();
});
$('set-custom-prompt').onchange = async () => {
  if (!currentSettings) return;
  const text = ($('set-custom-prompt').value || '').trim();
  if (!text) return; // keep the last good prompt rather than saving an empty one
  currentSettings.refine.style = { custom: text };
  await persistSettings();
};
$('set-keepaudio').onclick = async () => {
  if (!currentSettings) return; currentSettings.general.keep_audio = !currentSettings.general.keep_audio; await persistSettings();
};
// --- record hotkey rebind ---
function accelFromEvent(e) {
  if (['Control', 'Alt', 'Shift', 'Meta'].includes(e.key)) return null; // wait for the main key
  const mods = [];
  if (e.ctrlKey) mods.push('Ctrl');
  if (e.altKey) mods.push('Alt');
  if (e.shiftKey) mods.push('Shift');
  if (e.metaKey) mods.push('Super');
  if (!mods.length) return null; // require at least one modifier
  let key = e.key === ' ' ? 'Space' : (e.key.length === 1 ? e.key.toUpperCase() : e.key);
  return mods.concat(key).join('+');
}
let capturingHotkey = false;
$('set-hotkey-btn').onclick = () => {
  if (capturingHotkey) return; // a second click while waiting would stack key listeners
  capturingHotkey = true;
  const el = $('set-hotkey-keys'); const orig = el.textContent;
  el.textContent = 'Press keys…';
  const onKey = async (e) => {
    if (['Control', 'Alt', 'Shift', 'Meta'].includes(e.key)) return; // modifier alone — keep waiting
    e.preventDefault();
    document.removeEventListener('keydown', onKey, true);
    capturingHotkey = false;
    const accel = accelFromEvent(e);
    if (accel && currentSettings) {
      currentSettings.general.hotkey = accel; el.textContent = accel; await persistSettings();
    } else { el.textContent = orig; toast('Use a modifier + key, e.g. Ctrl+Super+Space'); }
  };
  document.addEventListener('keydown', onKey, true);
};
$('set-model-btn').onclick = () => showView('models');
$('set-diag-btn').onclick = async () => {
  try { const d = await invoke('get_diagnostics'); await navigator.clipboard.writeText(d); toast('Diagnostics copied'); }
  catch (e) { toast('Couldn’t copy diagnostics'); }
};
$('set-log-btn').onclick = () => invoke('open_log').catch(() => {});

// ---- update check ----
let updateUrl = null;
async function checkUpdate(announce) {
  try {
    const r = await invoke('check_update');
    if (r.available) {
      updateUrl = r.url;
      $('set-update-s').textContent = `Update available: ${r.latest} — tap to view`;
      $('set-update-btn').textContent = 'View';
      toast(`Update available: ${r.latest}`);
    } else if (announce) {
      $('set-update-s').textContent = `Up to date (${r.current})`;
      toast(`You're on the latest version (${r.current})`);
    }
  } catch (e) { if (announce) toast('Couldn’t check for updates (offline?)'); }
}
$('set-update-btn').onclick = () => {
  if (updateUrl) invoke('open_path', { path: updateUrl }).catch(() => {});
  else checkUpdate(true);
};
setTimeout(() => checkUpdate(false), 2500); // quiet check shortly after launch

// ---- models manager ----
function modelTier(sizeMb) {
  if (sizeMb < 150) return 'Fast';
  if (sizeMb < 800) return 'Balanced';
  return 'Accurate';
}
async function loadModels() {
  const list = $('models-list'); list.textContent = '';
  let models = [];
  try { models = await invoke('list_models'); } catch (e) {}
  for (const m of models) {
    const row = document.createElement('div'); row.className = 'row'; row.style.borderRadius = 'var(--radius-sm)';
    row.dataset.model = m.id;
    const ico = document.createElement('div'); ico.className = 'ico'; ico.textContent = m.installed ? '✓' : '↓';
    const txt = document.createElement('div'); txt.className = 'txt';
    const t = document.createElement('div'); t.className = 't'; t.textContent = m.display;
    const sub = document.createElement('div'); sub.className = 's';
    sub.textContent = `${modelTier(m.size_mb)} · ${m.size_mb} MB${m.installed ? ' · installed' : (m.pinned ? '' : ' · unverified')}`;
    txt.append(t, sub);
    const btn = document.createElement('div'); btn.className = 'ctrl-sm';
    const cur = currentSettings?.transcription?.model === m.id;
    btn.textContent = m.installed ? (cur ? 'In use' : 'Use') : (m.pinned ? 'Download' : '—');
    btn.onclick = async () => {
      if (m.installed) {
        if (!cur && currentSettings) { currentSettings.transcription.model = m.id; await persistSettings(); loadModels(); }
      } else if (m.pinned) {
        btn.textContent = '0%'; sub.textContent = `Downloading… (${m.size_mb} MB, resumes on retry)`;
        invoke('download_model', { id: m.id }).catch(() => { btn.textContent = 'Download'; });
      }
    };
    row.append(ico, txt, btn); list.appendChild(row);
  }
  a11yInit();
}

// ---- note detail ----
let currentNote = null, noteTab = 'refined';
async function openNote(refinedPath) {
  try { currentNote = await invoke('read_note', { refinedPath }); currentNote.refined_path = refinedPath; }
  catch (e) { toast('Couldn’t open the note'); return; }
  $('note-title').textContent = currentNote.title || 'Note';
  $('note-sub').textContent = `${currentNote.voices || ''}${currentNote.lossless_ok === false ? ' · review: detail may differ' : ''}`;
  noteTab = 'refined'; renderNote();
  showView('note');
}
function renderNote() {
  if (!currentNote) return;
  $('note-body').textContent = noteTab === 'refined' ? (currentNote.refined || '') : (currentNote.raw || '');
  document.querySelectorAll('#note-toggle span').forEach(sp => sp.classList.toggle('on', sp.dataset.tab === noteTab));
}
document.querySelectorAll('#note-toggle span').forEach(sp => sp.onclick = () => { noteTab = sp.dataset.tab; renderNote(); });
$('note-back').onclick = () => showView('history');
$('models-back').onclick = () => showView('settings');
$('note-copy').onclick = () => {
  const body = noteTab === 'refined' ? currentNote?.refined : currentNote?.raw;
  if (body) navigator.clipboard.writeText(body).then(() => toast('Copied')).catch(() => toast('Copy failed'));
};
$('note-open').onclick = () => {
  if (currentNote?.refined_path) invoke('open_path', { path: currentNote.refined_path }).catch(() => toast('Couldn’t open the file'));
};
$('note-type').onclick = async () => {
  if (!currentNote) return;
  // Dictation: type the plain transcript (strip "**Speaker:**" markers) into the
  // app that was focused before Voz. The panel hides itself first.
  const src = noteTab === 'refined' ? (currentNote.refined || '') : (currentNote.raw || '');
  const plain = src.replace(/^\*\*[^:*]+:\*\*\s*/gm, '').replace(/^>\s.*$/gm, '').trim();
  try { await invoke('type_at_cursor', { text: plain }); }
  catch (e) { toast(String(e)); }
};
$('note-export').onclick = async () => {
  if (!currentNote) return;
  const body = noteTab === 'refined' ? currentNote.refined : currentNote.raw;
  const safe = (currentNote.title || 'transcript').replace(/[\\/:*?"<>|]/g, ' ').trim() || 'transcript';
  const ext = noteTab === 'refined' ? 'md' : 'txt';
  try {
    const path = await window.__TAURI__.dialog.save({ defaultPath: `${safe}.${ext}`, filters: [{ name: 'Text', extensions: ['txt', 'md'] }] });
    if (path) { await invoke('save_text_file', { path, content: body || '' }); toast('Exported'); }
  } catch (e) { toast('Export failed'); }
};
$('note-delete').onclick = async () => {
  if (!currentNote?.refined_path) return;
  // Only navigate away if the delete actually succeeded — don't pretend it's gone.
  try { await invoke('delete_note', { refinedPath: currentNote.refined_path }); toast('Note deleted'); showView('history'); }
  catch (e) { toast('Couldn’t delete the note'); }
};
const STYLES = ['adaptive', 'meeting', 'memo'];
$('note-restyle').onclick = async () => {
  if (!currentNote?.refined_path) return;
  const cur = typeof currentSettings?.refine?.style === 'string' ? currentSettings.refine.style : 'adaptive';
  const next = STYLES[(STYLES.indexOf(cur) + 1) % STYLES.length];
  $('note-body').textContent = 'Re-refining (' + next + ')…';
  invoke('rerefine', { refinedPath: currentNote.refined_path, style: next }).catch(() => {});
};

// ---- engine events ----
listen('voz://event', (e) => {
  const p = e.payload || {};
  switch (p.type) {
    case 'recState':
      // `Recording` fires on both start and resume — resume keeps the session
      // (and elapsed timer) and just restarts the meter; start sets up fresh.
      if (p.state === 'Recording') recording ? resumeRecording() : enterRecording();
      else if (p.state === 'Paused') pauseRecording();
      else if (p.state === 'Idle') leaveRecording();
      break;
    case 'partial': showPartial(p.text); break;
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
      // update the specific model row in the manager, if open
      const row = document.querySelector(`#models-list .row[data-model="${p.id}"]`);
      if (row) {
        const btn = row.querySelector('.ctrl-sm');
        if (btn) btn.textContent = pct >= 100 ? 'Installed' : pct + '%';
        if (pct >= 100) setTimeout(loadModels, 500); // refresh installed/Use state
      }
      break;
    }
    case 'jobFailed': toast(friendlyError(p.error)); setPill('Ready'); break;
    case 'noteUpdated':
      if (currentNote && p.refined_path === currentNote.refined_path) openNote(p.refined_path);
      if (document.getElementById('view-history').classList.contains('on')) loadHistory();
      break;
    case 'notify': toast(p.title || 'Note ready'); break;
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

// ---- first-run onboarding ----
async function maybeOnboard() {
  if (!currentSettings) { try { currentSettings = await invoke('get_settings'); } catch (e) { return; } }
  if (currentSettings.general?.onboarded) return;
  $('ob-savedir').textContent = currentSettings.general.save_dir;
  document.querySelectorAll('#ob-backend span').forEach(sp =>
    sp.classList.toggle('on', sp.dataset.b === currentSettings.refine.backend));
  $('onboard').classList.add('on');
}
$('ob-savedir-btn').onclick = async () => {
  try {
    const dir = await window.__TAURI__.dialog.open({ directory: true, title: 'Choose save folder' });
    if (dir && currentSettings) { currentSettings.general.save_dir = dir; $('ob-savedir').textContent = dir; }
  } catch (e) {}
};
document.querySelectorAll('#ob-backend span').forEach(sp => sp.onclick = () => {
  if (!currentSettings) return;
  currentSettings.refine.backend = sp.dataset.b;
  document.querySelectorAll('#ob-backend span').forEach(s => s.classList.toggle('on', s === sp));
});
$('ob-start').onclick = async () => {
  if (!currentSettings) return;
  currentSettings.general.onboarded = true;
  await persistSettings();
  $('onboard').classList.remove('on');
};
maybeOnboard();
