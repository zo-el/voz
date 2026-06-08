# Voz — local recorder + transcriber

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

> **Status: working app.** The full local pipeline is built and verified —
> dual-source capture → local Whisper → Me/Them attribution → LLM refine → two
> linked Obsidian notes → SQLite full-text history → tray GUI. On top of that:
> first-run **onboarding**, in-app **note detail** (re-refine / export / dictate),
> **live streaming partials**, audio/video **file import**, **rebindable hotkey**,
> **GPU auto-detect** (CUDA verified on an RTX 3080), tiered **model manager**,
> **logging/diagnostics**, **in-app update check**, friendly error states, and a
> keyboard/ARIA accessibility pass. 71 `voz-core` tests; clippy `-D warnings` clean;
> `.deb` + AppImage + **CI auto-release**. Remaining work is scoped in
> [`docs/ROADMAP.md`](docs/ROADMAP.md) (COSMIC applet, cross-distro QA, Flathub).

## Try it

```bash
# build + run (debug)
cargo build --manifest-path voz-app/src-tauri/Cargo.toml
DISPLAY=:1 ./voz-app/src-tauri/target/debug/voz-app

# or install the package
cd voz-app/src-tauri && cargo tauri build --bundles deb
sudo apt install ./target/release/bundle/deb/Voz_0.1.0_amd64.deb
```
First run with no model auto-downloads `base.en` (verified). Run `voz-core`'s tests
with `cargo test -p voz-core --features engine`. Full build notes: `BUILD.md`.

## What's here

| Path | What |
|---|---|
| [`docs/USER_GUIDE.md`](docs/USER_GUIDE.md) | How to record, review history, configure, and the privacy guarantee |
| [`docs/TROUBLESHOOTING.md`](docs/TROUBLESHOOTING.md) | No tray, no audio, raw-only fallback, import, GPU, hotkey, logs |
| [`BUILD.md`](BUILD.md) | Build from source, system deps, and the CPU / CUDA / Vulkan recipes |
| [`docs/ROADMAP.md`](docs/ROADMAP.md) | Living roadmap: what's done, in progress, and the disposition of the rest |
| [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) | The `voz-core` crate, the capture→transcribe→refine pipeline, data model, packaging |
| [`docs/SECURITY.md`](docs/SECURITY.md) | Threat model + controls (XSS, subprocess, secrets, sandbox, supply chain) |
| [`docs/TESTING.md`](docs/TESTING.md) | Test pyramid, tooling, CI lanes, coverage gate, QA matrix |
| [`docs/DESIGN.md`](docs/DESIGN.md) / [`docs/RESEARCH.md`](docs/RESEARCH.md) | Visual language + screen walkthrough; competitive landscape + citations |
| [`docs/PLAN.md`](docs/PLAN.md) | Original scope, milestones, and Definition of Done (historical — see ROADMAP for current state) |
| [`CHANGELOG.md`](CHANGELOG.md) | Release notes |
| [`design/mockups/`](design/mockups) · [`design/out/`](design/out) | High-fidelity HTML/CSS mockups + rendered PNGs (original design) |

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
- **Transcription:** whisper.cpp via `whisper-rs`, CPU + Vulkan/CUDA. The default
  model is **auto-fetched on first run** (zero manual setup) and a tiered model
  manager (Fast / Balanced / Accurate) lets you switch; it works fully offline after.
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
