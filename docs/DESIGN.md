# Design — Voz

The interactive mockups live in `design/mockups/*.html` (open any in a browser),
and rendered PNGs are in `design/out/`. Re-render with `node design/render.mjs`.

These are **high-fidelity HTML/CSS** mockups deliberately — under the recommended
Tauri stack they become the actual UI almost verbatim.

---

## 1. Concept: the tray icon *is* the whole app

There is no main window and no dock entry. Clicking the tray icon (or pressing the
global hotkey) drops down a single compact **panel** (400 px wide) that contains
the entire app: source selection + record controls, the live raw transcript and
its refined note, history, and settings — all as tabs inside the same panel. It
dismisses on blur. See `design/out/tray-context.png`.

Each recording captures your **mic and/or the system audio** (so meetings record
locally, with Me/Them speaker labels) and produces **two linked notes — Raw and
Refined** — written as Obsidian-friendly Markdown to a save folder you choose.

```
┌─ top bar ───────────────────────── 🎙▾ ──┐   ← tray icon
│                                    ╔══════╗
│                                    ║ panel║   ← the whole app drops down here
│                                    ╚══════╝
```

(On GNOME-Wayland the panel is compositor-placed near the tray rather than pixel-
pinned — see `RESEARCH.md`. The arrow/tether is cosmetic there, exact on COSMIC.)

---

## 2. Visual language

- **Dark olive green**, dark-only for now (light theme is a later addition). Soft
  elevation, 16 px panel radius, hairline olive-tinted borders.
- **One accent**, olive/moss `#8a9a4e → #b3c267`, used for the logo, active states,
  primary actions, and the refine stage. A warm **terracotta** `#d9614a` (chosen to
  harmonize with olive) is reserved *only* for the record/stop control and the live
  indicator, so "recording" is unmistakable.
- **Type:** Inter (UI) + JetBrains Mono (timer, paths, hotkeys). System-font
  fallback so it still renders offline/native.
- Tokens are defined as CSS variables at the top of `design/mockups/style.css`
  (`--bg, --panel, --surface, --accent, --rec, --ok, --radius …`) — a single
  source of truth that maps directly to a theme file in the app.

Palette (dark olive):

| token | value | use |
|---|---|---|
| `--bg` | `#0a0c08` | desktop behind panel |
| `--panel` | `#11140d` | panel body |
| `--surface` | `#1e2316` | controls, chips |
| `--text` / `--text-2` / `--text-3` | `#ecefe0` / `#a9b094` / `#6e7459` | warm text ramp |
| `--accent` → `--accent-2` | `#8a9a4e` → `#b3c267` | brand, active, refine |
| `--rec` | `#d9614a` | record/stop only (terracotta) |
| `--ok` | `#86c06a` | success / done |

---

## 3. Screens (all rendered in `design/out/`)

1. **`panel-record-idle`** — Ready. A **compact source pill** (Mic / System / Both,
   active one labelled, others icon-only — it's set rarely so it stays small but
   reachable), idle timer, flat waveform, the big record button, the global-hotkey
   hint, and — when something is processing — a **background-job bar** ("Refining ·
   Planning sync → History") proving the recorder stays free. No header gear:
   Settings lives only in the bottom tab bar (Record · History · Settings).
2. **`panel-recording`** — Live. Terracotta pulsing "Recording" pill, running timer,
   live waveform, stop square flanked by Pause and Cancel (✕). The job bar shows a
   *previous* note still transcribing — you record and process at the same time.
3. **`panel-transcribing`** — A **note detail opened from History while it
   processes** (back arrow → History): the **Raw transcript** card with **Me / Alex**
   attribution above the **Refining** card (streaming structured notes + progress).
   "Copy raw now" + a reminder that it finishes in the background with a notification.
4. **`panel-result`** — A finished **note detail** (back arrow → History). **Raw /
   Refined** toggle; the Refined note shows **Summary / Decisions / Action items**
   (per-person). Actions: **Copy** (primary), **Open in Obsidian**, **Re-refine**,
   overflow. Footer: "Saved 2 linked notes to …".
5. **`panel-history`** — The activity + notes hub. A **Processing** group at the top
   shows in-flight jobs (Transcribing / Refining with progress bars); below,
   completed notes with time, **source** (Mic / Meeting), duration/voice-count, and
   the refine backend. Header shows "N processing"; search + "Open folder".
6. **`panel-settings-general`** — **Save location** (configurable; shown pointed at
   an Obsidian vault), Output (raw + refined as two linked notes), Obsidian-friendly
   toggle (front-matter + `[[wikilink]]`), keep-audio, and an **Audio sources**
   group: microphone, system-audio (loopback) toggle, default source (Mic/System/
   Both), record hotkey, theme, launch-on-login.
7. **`panel-settings-engine`** — Two numbered stages: **1 Transcription** (local
   model picker w/ size, acceleration CPU/Vulkan/CUDA, language) and **2 Refine**
   (radio cards: **Claude Code**, **Codex CLI**, **Local LLM/Ollama**, **Claude
   API**, **None**), plus an editable **Refinement style** (Adaptive) and the
   **Lossless guard** toggle. Header reminds: raw is always the source of truth.
8. **`tray-states`** — The tray icon's three states: **Idle**, **Recording** (red
   dot), **Processing** (olive dot) — what you see with the panel closed.

---

## 4. Interaction model

- **Tray click / hotkey** → panel opens on the Record tab with the last source.
- **Audio source** (Mic / System / Both) is chosen before recording; **Both**
  (mic + system loopback) is the default so meetings are captured by default.
- **Push-to-talk**: hold the hotkey to record, release to stop+transcribe (the
  fast path; nothing to click). Toggle mode also available.
- **Record button**: idle→record, record→stop. **Pause** suspends capture;
  **Cancel (✕)** discards without transcribing.
- **Stop hands off to the background.** A Stop snapshots the audio into a job
  (transcribe → refine) and returns the Record tab to Ready immediately — so you
  can start the next recording right away. Jobs appear in **History**; the tray
  badge turns olive; a **desktop notification** fires when a note is ready.
- **History is where notes live and process.** Tapping a job opens its detail
  (the transcribing/result views); tapping a finished note opens it to read.
- **Result**: every recording produces **Raw + Refined**; the toggle flips between
  them. **Copy** is one keystroke/click; **Open in Obsidian** opens the refined
  note. **Re-refine** re-runs refinement with a different style on the same raw
  transcript (no re-record). *(Paste-at-cursor / dictation insert is deferred to
  v2 — v1 is Copy + Open in Obsidian.)*
- **Refined note shape is Adaptive**: meetings get Summary / Decisions / Action
  items; solo memos get clean detailed notes — the style is editable in settings.
- **Tray icon = status light** (panel closed): idle / recording (red) / processing
  (olive). One settings entry only — the bottom-nav Settings tab.
- **Dismiss on blur**; `Esc` closes. State persists (a recording keeps running in
  the tray even with the panel closed — the icon shows the live state).

---

## 5. States the UI must represent
Two independent axes (because recording and processing are decoupled):
- **Recorder:** `Idle/Ready · Recording · Paused`.
- **Per-job:** `Queued · Transcribing · Refining · Done/Saved · Failed`.
- **Plus:** `Model-downloading · No-backend-configured`.

These combine — e.g. *Recording* while two jobs are *Refining*. The **tray badge**
collapses them to idle / recording (red) / processing (olive). (Idle, Recording,
the History processing list, a processing detail, and a finished detail are mocked;
the rest reuse the same components.)

---

## 6. Accessibility & polish details
- Hit targets ≥ 30 px; the record button is 72 px.
- Color is never the only signal (icons + text labels on every state).
- Respect `prefers-reduced-motion` (waveform + pulse animations gate on it).
- Full keyboard path: Space=record/stop, P=pause, C/⌘C=copy, Esc=close,
  Tab cycles controls.
- Light theme + high-contrast variant derive from the same tokens.

---

## 7. From mockup → app
Because the recommended frontend is a Tauri webview, `style.css` becomes the app
stylesheet and each `panel-*.html` becomes a view/component. The only additions
for the real app are state wiring (the `Event` stream from `voz-core`) and the
live waveform/streaming-token animations. Nothing about the visual design needs to
change to ship it.
