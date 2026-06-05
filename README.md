# Voz — local recorder + transcriber (planning)

A tray-resident Linux app (GNOME + COSMIC) that records your **microphone and/or
the system audio you hear** — so it captures meetings locally without ever joining
them — transcribes **locally** with Whisper, and turns each recording into **two
linked notes: a Raw verbatim transcript and a Refined note** (detailed, non-lossy
summary / decisions / action items). Refinement runs through an LLM you choose
(Claude Code / Codex CLI, a local model, or the Claude API). Everything stays on
your machine; the only optional network call is the refine step, which can be
fully local or turned off.

Notes are written as **Obsidian-friendly Markdown** (YAML front-matter + a
`[[wikilink]]` from the refined note to the raw) into a **save folder you choose**
— point it at your Obsidian vault and read the detailed docs there.

The tray icon *is* the whole app: click it (or hit the global hotkey) and a
compact olive-dark panel drops down with the source selector (Mic / System / Both),
record / pause / stop, the live raw transcript and its refined note, history, and
settings.

> **Status: planning & design — nothing is built yet.** This repo currently
> contains research, architecture, and visual mockups for review/collaboration
> before implementation begins.

## What's here

| Path | What |
|---|---|
| [`docs/RESEARCH.md`](docs/RESEARCH.md) | Competitive landscape, the Wayland tray reality, local-Whisper stack — with citations |
| [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) | The `voz-core` Rust crate, the capture→transcribe→refine pipeline, dual-source audio, data model, settings, packaging |
| [`docs/DESIGN.md`](docs/DESIGN.md) | Visual language + a walkthrough of every screen |
| [`docs/SECURITY.md`](docs/SECURITY.md) | Threat model + controls (XSS, subprocess, secrets, sandbox, supply chain) |
| [`docs/TESTING.md`](docs/TESTING.md) | Test pyramid, tooling, CI lanes, coverage gate, QA matrix |
| [`docs/PLAN.md`](docs/PLAN.md) | Scope, milestones (M0–M7), Definition of Done, decisions, open questions |
| [`design/mockups/`](design/mockups) | High-fidelity HTML/CSS mockups (open in a browser) |
| [`design/out/`](design/out) | Rendered PNGs of every screen |

## See the design

Open any `design/mockups/panel-*.html` in a browser, or look at the rendered PNGs
in `design/out/` (start with `tray-context.png`). To re-render after edits:

```bash
npm install          # one-time (Playwright + Chromium)
node design/render.mjs
```

## The one big constraint (please read)

A pixel-anchored tray dropdown is achievable natively on **COSMIC** (a libcosmic
applet) but **structurally impossible on GNOME-Wayland** for *any* framework
(no layer-shell, no client-set global coordinates, no native tray). The plan is to
ship one cross-desktop Tauri app first (frameless panel placed near the tray on
GNOME, which is fine on Pop!_OS), and add a native COSMIC applet later for the
true anchored dropdown — reusing the same Rust core. Details and alternatives in
[`docs/PLAN.md`](docs/PLAN.md).

## Headline choices (locked)

- **Capture:** mic + system-audio loopback (PipeWire monitor); **Both** is the
  default for local meeting capture, with Me/Them speaker labels. No meeting
  integration — it only records what's already playing.
- **Transcription:** whisper.cpp via `whisper-rs`, default `large-v3-turbo q5_0`,
  CPU + Vulkan/CUDA. **Bundled with the app** — zero manual setup, works offline.
- **Record while processing:** Stop hands the recording to a **background job
  queue**, so the recorder is instantly free for the next one. Jobs live in the
  **History** tab; the **tray icon shows state** (recording = red, processing =
  olive) with the panel closed; a **notification** fires when a note is ready.
- **Output:** **Raw + Refined** two linked notes; Obsidian-friendly Markdown to a
  save folder you choose.
- **Refine:** pluggable — **Claude Code CLI** by default (no extra API key), or
  Codex CLI / Ollama / Claude API / None.
- **Frontend (v1):** Tauri v2 on **both GNOME and COSMIC** (the mockups become the
  UI 1:1). **Theme:** dark olive (dark only for now).

Remaining open questions are at the end of [`docs/PLAN.md`](docs/PLAN.md).
