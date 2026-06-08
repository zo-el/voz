# Changelog

All notable changes to Voz. Format: [Keep a Changelog](https://keepachangelog.com/),
[Semantic Versioning](https://semver.org/).

## [Unreleased]

## [0.3.0] — 2026-06-08

The first broadly-compatible release of the substantial UX + power-feature pass on
top of 0.1.0. (The 0.2.x builds were superseded before a working release shipped —
0.2.0's binaries required glibc 2.38+ and wouldn't start on 22.04.)

### Added
- **First-run onboarding** — welcome → choose save folder → pick AI-cleanup backend.
- **In-app note detail** — open a History note in-panel: Raw/Refined toggle, Copy,
  Open in Obsidian, **Re-refine** with another style, **Export** (.txt/.md),
  **Type at cursor** (dictation), Delete.
- **Full-text search** over titles *and* transcript bodies (one-time backfill of
  existing notes).
- **Live streaming partials** — a "◉ LIVE" transcript preview + word count while
  recording (the final transcript stays authoritative).
- **Audio/video file import** — transcribe an existing file via ffmpeg.
- **Editable Custom refine prompt** (alongside Adaptive/Meeting/Memo).
- **Rebindable global hotkey**; **tray-anchored** window placement on X11.
- **GPU acceleration**: Acceleration selector + runtime auto-detect ("Now: CUDA —
  NVIDIA GPU" / "CPU"); CUDA build verified on an RTX 3080.
- **Model manager** with Fast/Balanced/Accurate tiers + live download progress.
- **Logging & diagnostics** (local, no telemetry) + **in-app update check**.
- **Friendly error states** (toasts) and an **accessibility** pass (keyboard + ARIA).
- **CI auto-release** workflow + a slow lane (real-whisper integration);
  `docs/USER_GUIDE.md`, `docs/TROUBLESHOOTING.md`, Flatpak packaging files, and a
  COSMIC applet scaffold.
- **Multi-backend release artifacts** — each release now ships **three** Linux
  x86-64 variants built by CI: CPU (`.deb` + AppImage, the default), Vulkan
  (`-vulkan`, portable GPU with CPU fallback), and CUDA (`-cuda.deb`, NVIDIA,
  sm_75/86/89). Pick the asset that matches your hardware.

### Changed
- Tray is now **best-effort** — a missing StatusNotifier host no longer prevents the
  app from launching (window + hotkey remain usable).

### Fixed
- **Release packaging runs across distros.** Bundles are built on Ubuntu 22.04
  (glibc 2.35) so they run on Pop!_OS / Ubuntu 22.04 and everything newer — a 24.04
  build needed glibc 2.38+ and wouldn't start. CI also installs the appindicator dev
  lib the tray app needs to bundle, and the three GPU variants build in parallel
  (Vulkan's `glslc` from the LunarG SDK; CUDA from the `ubuntu2204` repo).

### Engineering
- `voz-core`: **71 tests**, `clippy -D warnings` clean, fmt clean.

## [0.1.0] — 2026-06-05

First working version: a local-first Linux recorder + transcriber with a tray GUI.

### Added
- **Dual-source capture** — microphone and/or system audio (PipeWire monitor
  loopback), so meetings are recorded locally without joining them. Free Me/Them
  speaker attribution from the two streams.
- **Local transcription** — whisper.cpp via `whisper-rs`; model registry with
  SHA-256-verified downloads; CPU build (CUDA/Vulkan behind feature flags).
- **Refine pipeline** — turn the raw transcript into an adaptive, non-lossy note
  via a pluggable backend: **Claude Code CLI** (default), Codex CLI, Ollama, or the
  Claude API. The transcript is passed as data (stdin, never a shell argument).
  A "lossless guard" flags any dropped detail.
- **Two linked Obsidian notes** per recording (Raw + Refined, with YAML front-matter
  and a `[[wikilink]]`) saved to a folder you choose, plus a SQLite history index.
- **Tray GUI** (Tauri) — olive dark theme, source selector, record/pause/stop,
  movable window with minimize/hide, a tray menu, and interactive, persisted
  settings (save folder picker, refine backend, default source).
- **Recording ⊥ processing** — Stop hands off to a background job queue; the
  recorder is free immediately. Crash-recovery: spooled audio is re-processed on the
  next launch.
- **Zero-setup model** — first run with no model auto-downloads `base.en` (verified).
- **Graceful fallback** — if the chosen refine CLI isn't installed (e.g. a fresh
  machine without Claude Code), Voz starts in raw-only mode instead of failing.
- **Global hotkey** — `Ctrl+Super+Space` push-to-toggle (X11/GNOME today).
- **Packaging** — `.deb` (declares PipeWire + xdg-utils); AppStream metadata.

### Engineering
- `voz-core` (Apache-2.0): 60 tests, `clippy -D warnings` clean, fmt clean.
- CI (fmt/clippy/test across features + `cargo-deny`); `docs/SECURITY.md`,
  `docs/TESTING.md`, `docs/ARCHITECTURE.md`, `docs/ROADMAP.md`.

### Known limitations (see `docs/ROADMAP.md` Phase 1–2)
- Tray-anchored dropdown and global hotkey are X11/GNOME only; COSMIC/Wayland is
  planned (native applet + portal).
- No first-run onboarding yet; model is fetched, not bundled.
- GPU build is opt-in (`--features cuda` / `vulkan`); CPU by default.

[Unreleased]: https://github.com/zo-el/voz/compare/v0.1.0...develop
[0.1.0]: https://github.com/zo-el/voz/releases/tag/v0.1.0
