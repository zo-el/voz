# Voz — record, transcribe, and own your notes. Locally.

**Voz records your microphone and the meeting audio you hear, transcribes it on your
own machine with Whisper, and saves the result as plain Markdown in a folder you
choose.** No account, no upload of your audio, no lock-in. It lives in your system
tray — click it (or hit a hotkey) and a small panel drops down to record, review,
search, and manage everything.

It's built for one job: **capture spoken information privately and turn it into notes
you actually own** — meetings, calls, voice memos, interviews — without handing your
recordings to someone else's cloud.

---

## Why Voz

Most "AI notetakers" upload your audio to their servers. Voz doesn't. Recording and
transcription happen **entirely on your computer**, and the notes are **plain `.md`
files in your vault** (Obsidian-friendly) — not a proprietary database. You can read,
edit, grep, sync, or delete them with any tool, forever. If this project vanished
tomorrow, your notes would still be sitting there as readable text.

## What you get — the good

- **Fully local recording + transcription.** whisper.cpp runs on your CPU (or GPU).
  Your **audio never leaves the machine**, and it works offline.
- **Records meetings without joining them.** Captures your mic *and* the system audio
  you hear (PipeWire monitor) — both sides of a call, locally, with simple Me/Them
  labels, no bot in the meeting.
- **Two notes per recording, both yours.** A **Raw** verbatim transcript (the source
  of truth) and a **Refined** clean summary, linked, as Markdown with front-matter,
  saved wherever you point it.
- **AI cleanup is your choice.** Summarize with a local model (Ollama), a CLI you
  already have (Claude Code / Codex), the Claude API — or **turn it off** and keep
  only the raw transcript. A "lossless guard" flags anything the summary dropped, and
  the raw is always kept.
- **Find and reuse anything.** Full-text search across every transcript; open notes
  in-app to copy, export (.txt/.md), re-summarize in another style, dictate at the
  cursor, import an existing audio/video file, or open in Obsidian.
- **Stays out of your way.** Recording and transcription run in the background and
  survive a crash; the model auto-downloads on first run; a live preview shows the
  transcript as you talk.
- **No telemetry, ever.** Local logs only; the "copy diagnostics" button redacts your
  transcripts and paths. Open source (Apache-2.0).

## What it doesn't do — the honest part

- **Linux only, and X11 is the smooth path.** Runs great on GNOME/X11 today. On
  GNOME-**Wayland** the tray-anchored dropdown and global hotkey are limited by the
  platform (see the constraint note below); a native **COSMIC** applet is scaffolded
  but not built. **No macOS/Windows yet.**
- **The default summary uses a cloud model.** Out of the box, AI cleanup is **Claude
  Code**, which sends the **transcript text** (never the audio) to Anthropic. Want
  *everything* local? Pick **Ollama** or set cleanup to **None** — both keep all text
  on your machine. This is the one place data can leave, and it's your choice.
- **GPU is opt-in by download, not auto-switching.** The default `.deb`/AppImage is
  **CPU** (runs everywhere). Every release *also* ships a **Vulkan** build (portable
  GPU — any vendor, with CPU fallback) and a **CUDA** build (fastest, NVIDIA + the
  CUDA-12 runtime only) — grab the one for your hardware. No single binary hot-swaps
  backends; pick at install time.
- **As good as the model + the audio.** Small models are fast but err; accurate models
  are slower and larger. Speaker labels are mic-vs-system only — it won't separate
  multiple people on the *same* stream (no diarization yet).
- **It's young.** A working, tested app (71 core tests) — but not yet hardened across
  many distros, and not on Flathub. Install is `.deb` / AppImage / from source.

## Where your data lives — the security model, plainly

| Thing | Where it goes |
|---|---|
| **Your audio** | Recorded + transcribed **on your machine**, never uploaded. Optionally kept as a `.wav` next to the notes, or discarded. |
| **Raw transcript** | A Markdown file in **your** folder. Never sent anywhere by Voz. |
| **Refined summary** | Produced by the backend **you choose**: Ollama / None = stays local; Claude Code / Claude API / Codex = the **text** is sent to that provider (opt-in). |
| **History index** | A local SQLite cache (your Markdown notes remain the real copy). |
| **Logs / diagnostics** | A local file. No telemetry. Diagnostics are redacted (no transcripts, no paths). |

**The only network connections Voz makes:** (1) the optional cloud refine backend
*if you pick one*; (2) a one-time model download on first run; (3) an update check
against the GitHub releases feed (read-only — it never executes fetched code). Choose
**Ollama or None** for cleanup and Voz is fully offline after that first download.

Under the hood, the transcript is treated as **untrusted data** — handed to refine
backends over stdin/argv, never built into a shell command (nothing you *say* can
inject a command), with a fixed prompt and a delimited transcript. The webview runs
under a strict CSP + a minimal capability allowlist, and a `cargo-deny` gate watches
the dependency supply chain. Details: [`docs/SECURITY.md`](docs/SECURITY.md).

## Is it for you?

**Good fit if** you take a lot of spoken notes or meetings, you value privacy and want
your data as plain files, you're on Linux, and you're fine installing a `.deb`/AppImage
(or building once for GPU).

**Not yet, if** you need macOS/Windows, a one-click app-store install, a polished
Wayland/COSMIC experience, or built-in diarization for many speakers.

## Try it

```bash
# build + run (debug)
cargo build --manifest-path voz-app/src-tauri/Cargo.toml
./voz-app/src-tauri/target/debug/voz-app

# …or build an installable package
cd voz-app/src-tauri && cargo tauri build --bundles deb
sudo apt install ./target/release/bundle/deb/Voz_0.3.0_amd64.deb
```

First launch walks you through a save folder + cleanup choice and auto-downloads a
model. **Usage:** [`docs/USER_GUIDE.md`](docs/USER_GUIDE.md) · **Build + GPU recipes:**
[`BUILD.md`](BUILD.md) · **Fixes:** [`docs/TROUBLESHOOTING.md`](docs/TROUBLESHOOTING.md).

## The Wayland/GNOME tray reality (one honest constraint)

A pixel-anchored tray dropdown is achievable natively on **COSMIC** (a libcosmic
applet) but **structurally impossible on GNOME-Wayland** for *any* framework (no
layer-shell, no client-set global coordinates, no native tray). So Voz ships one
cross-desktop app first — a frameless panel placed near the tray (fine on Pop!_OS /
X11) — and a native COSMIC applet comes later for the true anchored dropdown, reusing
the same Rust core. The window + global hotkey keep it fully usable everywhere.

## Docs

| Path | What |
|---|---|
| [`docs/USER_GUIDE.md`](docs/USER_GUIDE.md) | How to record, review history, configure, and the privacy guarantee |
| [`docs/TROUBLESHOOTING.md`](docs/TROUBLESHOOTING.md) | No tray, no audio, raw-only fallback, import, GPU, hotkey, logs |
| [`BUILD.md`](BUILD.md) | Build from source, system deps, and the CPU / CUDA / Vulkan recipes |
| [`docs/ROADMAP.md`](docs/ROADMAP.md) | Living roadmap: what's done, in progress, and the disposition of the rest |
| [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) | The `voz-core` crate, the capture→transcribe→refine pipeline, data model, packaging |
| [`docs/SECURITY.md`](docs/SECURITY.md) | Threat model + controls (XSS, subprocess, secrets, sandbox, supply chain) |
| [`docs/TESTING.md`](docs/TESTING.md) | Test pyramid, tooling, CI lanes, coverage gate, QA matrix |
| [`docs/DESIGN.md`](docs/DESIGN.md) · [`docs/RESEARCH.md`](docs/RESEARCH.md) | Visual language + screen walkthrough; competitive landscape + citations |
| [`docs/PLAN.md`](docs/PLAN.md) | Original scope / milestones / Definition of Done (historical — see ROADMAP for current state) |
| [`CHANGELOG.md`](CHANGELOG.md) | Release notes |
| [`design/mockups/`](design/mockups) · [`design/out/`](design/out) | Original high-fidelity HTML/CSS mockups + rendered PNGs |

## License

[Apache-2.0](LICENSE). Your notes are plain Markdown in your own folder — they're
yours regardless.
