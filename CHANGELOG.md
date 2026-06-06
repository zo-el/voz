# Changelog

All notable changes to Voz. Format: [Keep a Changelog](https://keepachangelog.com/),
[Semantic Versioning](https://semver.org/).

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

[0.1.0]: https://github.com/USER/voz/releases/tag/v0.1.0
