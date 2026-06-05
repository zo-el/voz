# Architecture — Voz

Working name **Voz** (easily rebrandable). A local-first, tray-resident audio
recorder + transcriber that captures mic and/or system audio and turns each
recording into a Raw transcript plus a Refined note (optional LLM refine stage).

Read `RESEARCH.md` first — it establishes the one hard constraint that shapes
everything here: a pixel-anchored tray dropdown is achievable natively on COSMIC
but **structurally impossible on GNOME-Wayland**, for *any* framework.

---

## 1. Guiding principles

1. **Local-first.** Audio never leaves the machine. The *only* optional network
   call is the LLM refine stage, and that is user-selected and can be fully local
   (Ollama) or off entirely.
2. **Capture both sides, never join the meeting.** A recording can capture the
   **microphone** (you) and the **system output monitor** (everyone you hear —
   the meeting, a video, a call) simultaneously, all locally via PipeWire. Voz
   never connects to Zoom/Meet/Teams or sends a bot — it only records the audio
   already playing on your machine. Because "you" arrive on the mic stream and
   "them" on the monitor stream, we get free, reliable **Me vs. Them** speaker
   labels with no ML diarization.
3. **Raw + Refined, raw is truth.** Every recording yields **two linked
   documents**: the **Raw** verbatim transcript (the source of truth, always
   saved first) and a **Refined** note (organized, detailed notes — summary,
   decisions, action items). Refinement reorganizes and condenses *without losing
   information*; if you ever need more, the raw transcript is one click away.
4. **Framework-agnostic core.** All the real work (capture, transcribe, refine,
   store) lives in a plain Rust library with no UI and no desktop dependencies.
   The frontend(s) are thin — this lets us add a native COSMIC applet later
   without a rewrite.
5. **Obsidian-native output.** Notes are Markdown with YAML front-matter; the
   refined note links to the raw note with a `[[wikilink]]`, so pointing the save
   path at an Obsidian vault "just works."
6. **Recording and processing are independent.** Hitting Stop hands the recording
   to a **background job queue** (transcribe → attribute → refine) and immediately
   frees the Record tab — you can start the next recording while previous ones are
   still processing. Jobs surface in the **History** tab and finish with a desktop
   notification; nothing blocks the recorder.
7. **The tray icon is a status light.** Even with the panel closed, the icon shows
   what Voz is doing: idle, **recording** (red dot), or **processing** (olive dot).
8. **Zero-setup install.** Installing the app installs everything it needs —
   whisper.cpp and a default model are bundled (see §10). The user never hand-
   installs a runtime, model, or library to get a working app.
9. **Honest about Wayland.** We don't pretend to pixel-anchor on GNOME; we place a
   frameless panel near the tray and make it feel like a dropdown.

---

## 2. Recommended stack (v1)

| Layer | Choice | Why |
|---|---|---|
| Core logic | **Rust crate `voz-core`** | One place for audio + Whisper + refine + storage; reusable by any frontend |
| Transcription | **whisper.cpp via `whisper-rs`** (Codeberg) | Static link, no Python, CPU + Vulkan/CUDA, MIT/MIT |
| Audio | **PipeWire `pw-record`** (+ optional Silero VAD) | Dual-source: mic **and** system-output **monitor** (loopback) → 16 kHz mono f32. (Chosen over cpal: its ALSA backend doesn't reliably expose PipeWire monitor sources.) |
| Refine | pluggable: **Claude Code CLI / Codex CLI / Ollama / Claude API / None** | Reuses installed agentic CLIs — no extra key |
| Frontend (v1) | **Tauri v2** (Rust backend + web UI) | The confirmed mockups are HTML/CSS → they become the UI 1:1; one cross-desktop codebase; modern look |
| Tray (GNOME) | StatusNotifierItem via `tray-icon` (needs AppIndicator ext, preinstalled on Pop!_OS) | Only supported tray path on GNOME |
| Frontend (v2, optional) | **Native `libcosmic` applet** reusing `voz-core` | The *true* icon-anchored dropdown on COSMIC |

**Why Tauri for v1 and not the native split:** on GNOME the anchored popover is
impossible regardless of toolkit, so the native split buys its main advantage
(anchored dropdown) only on COSMIC. Meanwhile the design you confirmed is already
HTML/CSS — Tauri renders it exactly, on both desktops, from one codebase, and lets
us iterate on the UI fast. We keep `voz-core` clean so a native COSMIC applet is a
*later addition*, not a prerequisite. (If pixel-perfect COSMIC anchoring is a v1
must-have, see the alternative in `PLAN.md §6`.)

The frontend is replaceable precisely because it does almost nothing: it renders
state and forwards button presses. All decisions live in `voz-core`.

---

## 3. Component diagram

```
                          ┌──────────────────────────────────────────────┐
                          │                FRONTEND (thin)                │
                          │   Tauri v2 webview  — the confirmed panel UI  │
                          │   • renders state   • emits user intents      │
                          │   • tray icon (SNI) • frameless panel window  │
                          └───────────────▲───────────────┬──────────────┘
                              events (state, dual level,    │ commands
                              raw + refine tokens)          │ (start{source}/stop/
                                          │                │   pause, re-refine, settings)
                          ┌───────────────┴────────────────▼──────────────┐
                          │                   voz-core (Rust)             │
                          │                                               │
   mic ─────PipeWire──▶ ┌──┴───────┐  f32 16k   ┌──────────┐              │
   sink monitor ──────▶ │  audio   │──mono──────▶│   VAD    │──┐           │
   (loopback = "Them")  │ 2 sources│  (rubato)   │ (silero, │  │ segments  │
   mic = "Me"           │ cpal+rub.│             │ optional)│  ▼           │
                        └────┬─────┘             └──────────┘ ┌─────────────┐
                          │ dual level / WAV writer          │ transcribe  │
                          ▼                                   │ whisper-rs  │
                     ┌──────────┐                             │ + attribute │ Me/Them
                     │ recorder │  state machine              └─────┬───────┘
                     │ Idle/Rec/│                                   │ speaker-tagged RAW (persisted first)
                     │ Paused/  │                                   ▼
                     │ Transcr. │                             ┌─────────────┐
                     └──────────┘                             │   refine    │  backend trait
                          │                                   │  None |     │  ├─ ClaudeCode (spawn `claude`)
                          ▼                                   │  Local |    │  ├─ Codex (spawn `codex`)
                     ┌──────────┐   2 linked .md (+ .wav)     │  ClaudeAPI  │  ├─ Ollama (HTTP localhost)
                     │  store   │◀────────────────────────────│  + lossless │  └─ ClaudeAPI (https)
                     │ config + │   refined ⟶ [[raw]]          │   guard     │
                     │ history  │                             └─────────────┘
                     └──────────┘
```

---

## 4. `voz-core` module layout

```
voz-core/
  src/
    lib.rs            // public API: Engine handle, command enum, event stream
    config.rs         // Settings struct, XDG paths, TOML load/save, migrations
    audio/
      capture.rs      // cpal streams, device enum, ring buffers, RMS level
      sources.rs      // Source { Mic | System | Both }; resolve PipeWire mic
                      //   node + default-sink MONITOR (loopback) node
      resample.rs     // rubato → 16 kHz mono f32, per source
      vad.rs          // Silero VAD (optional, feature-gated) end-of-utterance
      wav.rs          // WAV writer (mixdown, or two-track mic/system) for "keep audio"
    recorder.rs       // state machine: Idle→Recording→Paused→Transcribing→Done
    transcribe/
      mod.rs          // trait Transcriber { transcribe(&[f32]) -> Transcript }
      whisper.rs      // whisper-rs impl; accel selection (cpu/vulkan/cuda)
      models.rs       // model registry, sizes, download+checksum, on-disk cache
      attribute.rs    // tag turns Me (mic) vs Them (monitor); merge into one raw
    refine/
      mod.rs          // trait Refiner { refine(raw, style) -> Refined }
      claude_code.rs  // spawn `claude -p` (or stdin pipe), read stdout
      codex.rs        // spawn `codex` CLI
      ollama.rs       // POST localhost:11434/api/generate
      claude_api.rs   // Anthropic Messages API (BYO key)
      lossless.rs     // completeness check: flag if refined drops detail from raw
    pipeline.rs       // orchestrates capture→transcribe→attribute→refine, emits Events
    store/
      mod.rs          // save two linked notes (raw + refined), list/search History
      formats.rs      // Markdown (+ YAML front-matter, [[wikilink]]) / TXT / SRT
    event.rs          // Event enum (see §6)
```

Public surface (sketch):

```rust
pub enum Source { Mic, System, Both }      // Both = mic + system monitor

// The RECORDER is a singleton; PROCESSING is a queue of independent JOBS.
pub enum RecState { Idle, Recording, Paused }
pub enum JobState { Queued, Transcribing, Refining, Done, Failed }
pub struct JobId(u64);

pub enum Command {
    // recorder
    Start { source: Source }, Pause, Resume, Stop, Cancel,
    // jobs (operate on a specific note, in the background)
    Rerefine { job: JobId, style: Option<String> },
    RetryJob(JobId), DismissJob(JobId),
    UpdateSettings(Settings), DownloadModel(ModelId),
}

pub enum Event {
    // recorder — independent of any job; you can record while jobs run
    RecState(RecState),
    Level { mic: f32, system: f32 },     // 0.0–1.0 RMS per source, ~30 Hz → waveform
    // per-job lifecycle (job started when a recording is Stopped)
    JobState { job: JobId, state: JobState },
    RawTranscript { job: JobId, text: Transcript }, // attributed; persisted FIRST
    RefineToken  { job: JobId, token: String },     // streaming refined note
    RefineDone   { job: JobId, refined: String, lossless_ok: bool },
    Saved        { job: JobId, refined: PathBuf, raw: PathBuf },
    JobFailed    { job: JobId, error: String },
    // global status the tray icon reflects (derived: recorder state + active jobs)
    Tray(TrayState),                     // Idle | Recording | Processing(n) | RecordingAndProcessing
    ModelProgress { id: ModelId, pct: f32 },
    Notify { title: String, body: String, job: JobId }, // desktop notification on completion
}

// the frontend holds a handle and a receiver:
let (engine, mut events) = voz_core::start(settings)?;
engine.send(Command::Start { source: Source::Both })?; // returns immediately
engine.send(Command::Stop)?;                            // enqueues a job, recorder is free again
while let Some(ev) = events.recv().await { /* update UI / tray / notify */ }
```

The Tauri layer is ~a few hundred lines: map Tauri commands → `Command`, forward
`Event`s to the webview over a Tauri channel, drive the **tray icon state** from
`Event::Tray`, raise a desktop notification on `Event::Notify`, and own the
frameless window show/hide-on-blur. No business logic.

**Jobs & concurrency.** `Stop` snapshots the captured audio into a `Job`, pushes it
on a bounded queue, and resets the recorder to `Idle` — so recording and processing
never block each other. A small worker pool drains the queue (Whisper is the heavy
step; default concurrency 1–2 so a long meeting doesn't starve the machine while
you record the next one). The **History** tab renders the queue (Queued /
Transcribing / Refining) above completed notes; the tray badge shows
`Processing(n)`; completion fires `Notify`.

---

## 5. The capture → transcribe → attribute → refine pipeline

```
[Start source=Both]
   ├─ MIC stream     @48k ─▶ downmix+resample ─▶ 16k mono f32  (ring buffer A = "Me")
   └─ MONITOR stream @48k ─▶ downmix+resample ─▶ 16k mono f32  (ring buffer B = "Them")
        each ├─▶ RMS level ──▶ Event::Level { mic, system }  (dual waveform)
             └─▶ (if "keep audio") WAV writer (mixdown, or 2-track)
[Stop]
   ─▶ transcribe A and B with whisper-rs (each labelled by source)
   ─▶ attribute.merge(A→Me, B→Them) by timestamp ─▶ speaker-tagged RAW transcript
                                          ──▶ Event::RawTranscript ──▶ STORE raw note FIRST (source of truth)
   ─▶ if refine.backend != None:
        refined = refiner.refine(raw, style) ── stream ──▶ Event::RefineToken
        lossless_ok = lossless.check(raw, refined)   // did the note keep the key facts?
                                          ──▶ Event::RefineDone { refined, lossless_ok }
   ─▶ store.save_linked(raw_note, refined_note)        // refined ⟶ [[raw]] wikilink
                                          ──▶ Event::Saved { refined, raw }
```

(Mic-only or System-only recordings skip the second stream and the Me/Them merge.)

Refine prompt (default **Adaptive** style) — reorganize, **do not lose
information**, and fit the shape to the content:

> "You are turning a verbatim transcript into a clean, well-structured note.
> **Choose the structure that fits the content:** if it's a meeting/conversation
> (multiple speakers), produce a short **Summary**, then **Decisions** and
> **Action items** (with the responsible person when stated); if it's a single
> speaker memo, produce clean, detailed notes / bullets without forcing those
> headings. In all cases, preserve every concrete fact, number, name, date, and
> commitment — when in doubt, keep it. Do **not** invent anything not in the
> transcript. Organize and condense wording, but never drop information; the
> verbatim transcript is kept separately as the source of truth."

The refinement style/prompt is editable in settings (one configurable style, not a
per-recording mode picker). "Adaptive" is the default; users who want a fixed shape
can pin one.

Lossless guard: a lightweight completeness check (entities/numbers/names present
in `raw` but absent from `refined`, plus a length-ratio floor). If it trips, the
refined note is flagged (`lossless_ok=false`) and surfaced in the UI — never
silently trusted — and the raw note is, as always, retained intact.

---

## 6. Events & threading
- `voz-core` runs its own async runtime (tokio). Audio capture is on a dedicated
  high-priority thread (cpal callback) writing into a lock-free ring buffer.
- Whisper inference runs on a blocking thread pool (it's CPU/GPU heavy).
- The frontend gets a single ordered `Event` stream (mpsc) — easy to render.
- Back-pressure: `Level` events are throttled to ~30 Hz; transcript tokens are
  forwarded as they arrive.

---

## 7. Data model & on-disk layout

Every recording saves **two linked Markdown notes** (refined + raw) so an Obsidian
vault is a first-class target.

```
~/.config/voz/config.toml          # settings (see §8)
~/.local/share/voz/models/         # downloaded GGML models (cached, checksummed)
~/.local/share/voz/history.sqlite  # index of recordings (fast search)

<save_dir>/                        # user-chosen, default an Obsidian vault e.g. ~/Obsidian/Vault/Voz/
  2026-06-05 Planning sync.md            # REFINED note (front-matter + notes + [[raw]] link)
  raw/2026-06-05 Planning sync (raw).md  # RAW verbatim transcript (source of truth)
  audio/2026-06-05 Planning sync.wav     # audio (if "keep audio" on)
```

Refined note (`.md`) — what you read in Obsidian:

```markdown
---
created: 2026-06-05T14:07:11
duration: 23:04
source: Both          # Mic | System | Both
voices: [Me, Alex]
model: whisper large-v3-turbo q5_0
refine: Claude Code
lossless_ok: true
raw: "[[2026-06-05 Planning sync (raw)]]"
tags: [voz, meeting]
---

## Summary
Reviewed next-week priorities. Ship the settings panel first, then wire up the
local model picker; diarization deprioritised.

## Decisions
- Settings panel ships before the model picker
- Diarization deferred — revisit after launch

## Action items
- **Me** — build the settings panel
- **Alex** — scope the model-picker work

> Full transcript: [[2026-06-05 Planning sync (raw)]]
```

Raw note (`(raw).md`) — speaker-attributed verbatim, the ground truth:

```markdown
---
created: 2026-06-05T14:07:11
refined: "[[2026-06-05 Planning sync]]"
---

**Me:** so the plan for next week is to ship the settings panel first and then …
**Alex:** sounds good can we leave the diarization piece for later …
```

`history.sqlite` mirrors the front-matter for instant search/filter in the History
tab; the Markdown files remain the portable source of truth.

---

## 8. Settings schema (`config.toml`)

```toml
[general]
save_dir        = "~/Obsidian/Vault/Voz"   # point at any folder / Obsidian vault
format          = "md"          # md | txt | srt
keep_audio      = true
obsidian_links  = true          # front-matter + [[wikilink]] refined ⟷ raw
theme           = "dark"        # dark (olive); light theme is a later addition
launch_on_login = true

[sources]
mic_device      = "default"     # PipeWire mic node, or "default"
system_audio    = true          # capture the default-sink MONITOR (loopback)
default_source  = "both"        # mic | system | both  (new recordings)
attribute       = true          # tag Me (mic) vs Them (monitor) in the raw note

[hotkey]
record          = "Ctrl+Super+Space"  # GNOME 48 portal; COSMIC: custom shortcut
mode            = "push_to_talk"      # push_to_talk | toggle

[transcription]
model           = "large-v3-turbo-q5_0"
accel           = "auto"        # auto | cpu | vulkan | cuda
language        = "auto"

[refine]
backend         = "claude_code" # none | claude_code | codex | ollama | claude_api
ollama_model    = "qwen2.5:3b"
claude_api_key  = ""            # only for claude_api backend (stored via secret service)
style           = "adaptive"   # adaptive | meeting | memo | <custom prompt> (editable)
lossless_guard  = true          # flag refined notes that drop detail from the raw
```

Secrets (API key) go to the OS secret service (libsecret) via `keyring`, never
plaintext in TOML.

---

## 9. Security / privacy posture
- No telemetry. The default model is **bundled**, so a fresh install transcribes
  fully offline; the only network is the chosen refine backend (and *optional*
  extra-model downloads, which are consented and show the URL/size).
- Claude Code / Codex refine = a local subprocess; the transcript is piped on
  stdin. Claude API = HTTPS to Anthropic with the user's key. Ollama = localhost.
- **System-audio capture is loopback only.** Voz records the audio already playing
  on your machine via the PipeWire sink monitor; it never connects to, logs into,
  or sends a bot to any meeting service. There is no integration to compromise.
- **Consent note (surfaced in the UI):** recording system audio captures other
  participants' voices. Recording-consent laws vary by jurisdiction; the tray icon
  shows an unmistakable "recording" state and the System/Both choice is explicit.
- Mic + monitor capture on bare Pop!_OS needs no portal; under Flatpak we request
  the PipeWire portal and the recording is sandboxed.
- "Local only" guarantee is enforceable: choosing refine = `None` or `Ollama`
  means zero outbound connections after model download.

---

## 10. Packaging & distribution — zero-setup install

**Goal: installing the app installs everything it needs.** The user never manually
installs a runtime, model, codec, or library to get a working app.

- **Primary: Flatpak (Flathub)** — sandboxed, runs on Pop!_OS GNOME and COSMIC.
  Bundles everything statically/in-runtime:
  - **whisper.cpp** compiled into the binary (via `whisper-rs`) — no separate install.
  - **A default model shipped inside the package** so transcription works on first
    launch **offline, with nothing to download**. To keep the download reasonable we
    bundle a mid model (e.g. `small` ~466 MB *or* `large-v3-turbo q5_0` ~547 MB) and
    offer larger/smaller models as optional, in-app, consented downloads. (Open
    question: which model to bundle vs. auto-fetch — see `PLAN.md §6`.)
  - **Audio** via the Freedesktop runtime's PipeWire + the **PipeWire portal**;
    `ffmpeg`/codecs come from the runtime. No system packages required.
- Also: **`.deb`** for Pop!_OS and an **AppImage** fallback — same principle, deps
  vendored; the `.deb` declares PipeWire as a dependency (already present on Pop).
- **GPU acceleration auto-detected at runtime** (CPU → Vulkan → CUDA) with manual
  override in Settings. We ship a CPU+Vulkan build by default (Vulkan is the
  cross-vendor Linux path); CUDA is an optional variant. The user installs one app;
  it picks the fastest backend it finds.
- **Refine backends are the one external piece** and are handled gracefully, not by
  manual setup: Claude Code / Codex are the user's *own* existing CLIs (detected on
  `PATH`); for **Ollama**, Settings offers a one-click "install & pull model" rather
  than a docs link; **None** always works offline with zero deps.
- **GNOME tray caveat:** on vanilla GNOME the AppIndicator extension is required
  (preinstalled on Pop!_OS). If absent, onboarding offers to enable it; meanwhile the
  app stays usable via the global hotkey and notifications.

### Note on "latest tools" (recording stack)
The capture/transcribe stack is deliberately current and Linux-native:
**PipeWire** (the modern audio server on Pop!_OS and COSMIC) for both mic and
system-monitor capture via **`cpal`**/`pipewire-rs`; **whisper.cpp** (actively
maintained, ~v1.8.x) via **`whisper-rs`** pinned to its **Codeberg** source (the
GitHub repo is archived); models from the official Hugging Face GGML repo; GPU via
**Vulkan/CUDA**. Engines sit behind a trait so we can adopt newer ones
(`sherpa-onnx`, Parakeet) without re-architecting. See `RESEARCH.md`.

---

## 11. Resilience & operations
Production-grade behavior under failure (full detail of testing in `TESTING.md`,
security in `SECURITY.md`):

- **No data loss.** The raw transcript is written **before** refine runs; all note
  writes are **atomic** (temp file + `rename`) so a crash or a synced vault can't
  corrupt a file. Audio is streamed to the WAV writer during capture, not held only
  in RAM.
- **Crash recovery.** The **job queue is persisted** (in `history.sqlite` /
  job journal). On restart, jobs that were Queued/Transcribing/Refining resume; a
  job whose raw note already exists never re-records, only re-refines.
- **Graceful degradation.** Missing system monitor → fall back to mic-only with a
  message. Refine backend unavailable/timeout/error → keep the raw note, mark the
  job Failed with a Retry; never block the recorder. Disk full / unwritable save dir
  → surfaced as an error state, audio retained in the app dir as a fallback.
- **Config migration.** `config.toml` carries a schema version; migrations run on
  load; an unreadable config is backed up and replaced with defaults (never a hard
  crash).
- **Observability.** Local, rotated logs (no telemetry); transcript content not
  logged at normal levels; secrets redacted. A built-in "copy diagnostics" produces
  a redacted bundle for bug reports.
- **Updates.** Shipped via signed channels (Flathub / signed apt repo / AppImage +
  signature); the app never fetches and executes arbitrary code. See `SECURITY.md §2.8`.
