# Research — local recording + transcription tray app

Date: 2026-06-05 · Target desktops: GNOME (Wayland) and COSMIC, on Pop!_OS.

This is the synthesis of three research passes: (1) the competitive landscape,
(2) the Linux tray + "dropdown-is-the-app" technical reality, and (3) local
Whisper transcription. Citations are inline.

---

## 1. The single most important finding (read this first)

**The "tray icon → a full borderless mini-app drops down, anchored right under
the icon" pattern is a desktop-shell capability on Wayland, not an app
capability — and the two target desktops solve it incompatibly.**

- **COSMIC** has a first-class answer: a **native `libcosmic` applet**. The panel
  hosts the applet and `get_popup_settings()` produces a properly anchored popup.
  This is the *only* clean way to get a true icon-anchored dropdown on COSMIC.
- **GNOME (Wayland)** has **no native tray**, **no layer-shell**, and **no way for
  a client to set global window coordinates**. So a pixel-anchored popover under
  the icon is **structurally impossible**. The best achievable is a
  StatusNotifierItem icon (via the AppIndicator extension) plus a **frameless
  window the compositor places** (it lands in the top-right near the tray, which
  is good enough visually, but it is not pixel-pinned to the icon).
- **Tauri and Electron do not work around this.** On Linux/Wayland both report
  tray-click position events as *unsupported* and window positioning as
  *unsupported*. They hit the exact same wall.

Sources: Wayland xdg-shell (compositor owns placement)
<https://wayland.app/protocols/xdg-shell>; gtk4-layer-shell "incompatible with
GNOME" <https://github.com/wmww/gtk4-layer-shell>; libcosmic applet popups
<https://pop-os.github.io/libcosmic-book/panel-applets.html>; Tauri tray (Linux
events unsupported) <https://v2.tauri.app/learn/system-tray/>; Handy issue #949
(Wayland shortcut/tray gaps) <https://github.com/cjpais/Handy/issues/949>.

**Design consequence:** we either (a) accept "frameless window placed near the
tray" on GNOME and ship one codebase, or (b) build a native COSMIC applet for the
true anchored dropdown and a separate GNOME tray app. This is the central
decision in `PLAN.md`.

> Note on the current machine: this is **Pop!_OS 22.04 (GNOME-based)**, where the
> AppIndicator extension ships **preinstalled**, so SNI tray icons work out of the
> box today. System76 is migrating Pop!_OS to **COSMIC**, so both targets are real
> for this user.

---

## 2. Competitive landscape

Closest reference apps (study these repos):

| App | Platforms | Stack | Engine | Local | UI pattern | License |
|---|---|---|---|---|---|---|
| **Handy** | mac/win/**linux** | Tauri 2 + Rust + React | whisper-rs + Parakeet (transcribe-rs) | ✅ | **Tray** + push-to-talk, paste at cursor | MIT |
| **Whispering / Epicenter** | mac/win/**linux**/web | Tauri + Svelte 5 | whisper.cpp / Parakeet / Moonshine / cloud | ✅ | Hotkey + voice-activated, **custom AI transforms** | MIT |
| **Vibe** | mac/win/**linux** | Tauri + whisper.cpp | Whisper (Vulkan/CoreML) | ✅ | Window app, diarization, **Claude + Ollama summaries** | MIT |
| **Speech Note (dsnote)** | **linux** | Qt/C++/QML | whisper.cpp / faster-whisper / Vosk | ✅ | Window + **tray** + global shortcut, X11+Wayland inject | MPL-2.0 |
| **Buzz** | mac/win/**linux** | Python/PyQt6 | Whisper / faster-whisper / API | ✅ | Window, batch, watch-folder, transcript viewer | MIT |
| **VoiceInk** | macOS | Swift + whisper.cpp | Whisper + Parakeet | ✅ | Menubar, hotkey, **Power Mode** (per-app) | GPLv3 ($40) |
| **superwhisper** | macOS | native | Whisper + Parakeet | ✅ | Menubar, **Modes** system, LLM post-process | closed |
| **Wispr Flow** | mac/win (**no linux**) | cloud | cloud ASR | ❌ | Hotkey, auto-formatting | $15/mo |

Key repos with URLs:
- Handy — the closest blueprint: <https://github.com/cjpais/Handy>
- Whispering/Epicenter — local-first + LLM transforms: <https://github.com/EpicenterHQ/epicenter>
- Vibe — whisper.cpp + multi-GPU + summaries: <https://github.com/thewh1teagle/vibe>
- Speech Note — best Linux text-injection reference: <https://github.com/mkiol/dsnote>
- Buzz — transcript viewer / watch-folder: <https://github.com/chidiwilliams/buzz>
- Awesome-Whisper-Apps directory: <https://github.com/danielrosehill/Awesome-Whisper-Apps>

### Best-in-class UX patterns worth stealing
1. **Push-to-talk global hotkey** + a hands-free voice-activated mode.
2. **Modes / Power Mode** (superwhisper, VoiceInk): each mode bundles
   `{hotkey + Whisper model + LLM polish prompt + auto-activation rule}`. The most
   copied premium feature. We adopt this as **Modes** (Clean / Raw / Note / Code).
3. **Separate the voice model from the language model** — pair a fast local
   Whisper with an independently chosen polish LLM.
4. **Show raw → polished live** in the panel; let the user accept either.
5. **Lazy, consented model download** with sizes shown (never silently pull
   hundreds of MB).
6. **GPU acceleration is table stakes; Vulkan is the cross-vendor Linux answer.**
7. **Tray icon = state indicator** (idle / recording / transcribing).

### Market gap this app fills
There is **no polished, tray-native, Linux-first** local transcription app that
does **Whisper + LLM polish** in one dropdown. The best tray/hotkey apps
(superwhisper, VoiceInk) are macOS-only; the cross-platform ones either paste raw
text (Handy) or are window apps (Vibe). Using an **already-installed agentic CLI
(Claude Code / Codex) as the polish backend — no extra API key** — is essentially
unoccupied (only "Murmur" gestures at it). That is our wedge.

---

## 3. Local transcription stack

**Recommended engine: `whisper.cpp` via the `whisper-rs` Rust bindings.** It
links statically into a Rust binary with no Python runtime, is MIT-licensed
(code) over MIT-licensed (OpenAI weights), runs CPU-only, and accelerates on
CUDA / Vulkan / Metal. It's the de-facto stack for this app category (Handy, Vibe).

- whisper.cpp: <https://github.com/ggml-org/whisper.cpp> (MIT)
- whisper-rs — **active development moved to Codeberg**, GitHub mirror archived
  2025-07-30: <https://codeberg.org/tazz4843/whisper-rs> (Unlicense). Pin this.
- OpenAI Whisper weights are **MIT** (commercial use OK):
  <https://github.com/openai/whisper/blob/main/LICENSE>

**Recommended default model: `large-v3-turbo`, quantized `q5_0`** (~547 MB on
disk; `large-v2`-class accuracy at ~6–8× the speed of `large-v3`). Fallback
`small`/`base` for weak CPUs; full `large-v3` on strong GPUs.

Model sizes (GGML f16): tiny 75 MB · base 142 MB · small 466 MB · medium 1.5 GB ·
large-v3 ~3 GB · **large-v3-turbo ~1.5 GB (f16) / ~547 MB (q5_0)**.

**Audio pipeline (Linux):** `cpal` capture (PipeWire/ALSA) → downmix mono →
`rubato` resample to **16 kHz mono f32** → optional **Silero VAD** segmentation →
`whisper-rs`. This is exactly Handy's pipeline. (`cpal` does not resample, hence
`rubato`.) This machine already has PipeWire + `pw-record` + `ffmpeg`.

**Streaming:** Whisper is a 30-second-window model; "live" = VAD + chunking.
Press-to-talk → release → transcribe-segment (Handy's model) is simpler and
higher-quality than continuous streaming, and is what most local tools ship. We
default to that, with continuous streaming as an advanced mode later.

**Strong alternative to bundle alongside:** NVIDIA **Parakeet-TDT-0.6B-v3** via
`sherpa-onnx`/`transcribe-rs` — *more accurate* than Whisper on the Open ASR
Leaderboard (~6.3% WER), CPU-friendly, Handy's CPU default. Caveats: 25 EU
languages only, **CC-BY-4.0** (attribution required). `sherpa-onnx` (Apache-2.0,
Rust bindings) is the best future-proofing: one runtime that hosts Whisper +
Parakeet + Moonshine, so we can swap engines without re-architecting.

- Parakeet: <https://huggingface.co/nvidia/parakeet-tdt-0.6b-v3>
- sherpa-onnx: <https://github.com/k2-fsa/sherpa-onnx>

### Meeting capture = system-audio loopback (no integration)
Recording "the meeting" locally does **not** require joining Zoom/Meet/Teams or any
API. On PipeWire (this machine), every output sink exposes a **monitor** source —
a loopback of whatever is playing on your speakers/headphones. Capturing that
monitor **and** the mic at once gives both sides of a call, fully on-device:
- Enumerate the **default sink's monitor** node (e.g. `…analog-stereo.monitor`) and
  the mic node; open two `cpal`/PipeWire streams; resample each to 16 kHz mono.
- Because "you" arrive on the mic and "everyone else" on the monitor, a simple
  **per-stream label (Me vs Them)** yields reliable 2-party attribution with **no
  ML diarization** — cheaper and more robust than diarizing a single mixed track.
- Caveats to engineer around: monitor-node naming varies and some sinks (certain
  BT/headset profiles) expose no usable monitor → enumerate robustly, fall back to
  mic-only with a clear message; on **speakers**, the mic re-captures the meeting
  (echo) → recommend headphones or optional PipeWire echo-cancel; under **Flatpak**
  use the PipeWire portal. Prior art: meeting-notetakers (Meetily, ownscribe) and
  OBS-style desktop-audio capture all rely on the monitor source.

### The two-stage "transcribe → refine" pipeline
We keep the *raw* transcript verbatim and produce a *refined* note from it. Prior
art for local STT → LLM post-processing: **Ghost Pepper** (WhisperKit → on-device
Qwen cleanup), **Dictator** (multi-pass LLM formatting with a **deterministic
revert if the model drifts**), open-whisper (optional LLM post-processing).

Design rules we adopt (updated for the Raw + Refined model):
- The refine stage is **optional and pluggable**: `None` (raw only) · `Local`
  (Ollama/llama.cpp small model) · `Claude Code CLI` · `Codex CLI` · `Claude API`.
- **Refine reorganizes; it must not lose information.** The prompt produces
  structured, detailed notes (summary / decisions / action items) while preserving
  every concrete fact, number, name, date, and commitment — *not* a terse summary.
- **Lossless guard** (evolved from Dictator's drift guard): flag the refined note
  if entities/numbers present in the raw are missing, or length collapses past a
  floor — surfaced in the UI, never silently trusted.
- **Persist the raw transcript first** as the source of truth; store the refined
  note as a separate, linked file, so the detail is always one click away.
- Shelling out to an installed `claude` / `codex` CLI is a low-effort integration
  (spawn, pipe transcript on stdin, read the note) and reuses the user's existing
  auth — **no extra API key**. (CLIs are higher-latency than local Ollama, so
  offer both.)

Sources: Ghost Pepper <https://ai-beat.github.io/news/2026/04/ghost-pepper-local-pipeline/>;
Dictator <https://dictator.robgough.net/>.

---

## 4. Global hotkeys on Wayland
- **GNOME 48+**: supported via the `org.freedesktop.portal.GlobalShortcuts`
  portal (landed Feb 2025) — clean, user-consented.
- **COSMIC**: portal **not implemented yet** (xdg-desktop-portal-cosmic issue #4
  open). Workarounds: a user-defined custom Settings shortcut that toggles the
  app, or `evdev` (read `/dev/input`, needs the `input` group) as a power-user
  fallback — which is exactly what Handy resorted to.

Sources: GlobalShortcuts portal
<https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.GlobalShortcuts.html>;
COSMIC issue #4 <https://github.com/pop-os/xdg-desktop-portal-cosmic/issues/4>.

---

## 5. What this means for our build
1. The heavy lifting (dual-source audio + Whisper + refine pipeline + storage) is **desktop-
   and framework-agnostic Rust**. Build it once as a core crate.
2. The *only* genuinely desktop-specific, hard part is the **tray + anchored
   dropdown + global hotkey** — and it differs between GNOME and COSMIC.
3. We can ship value fast with one cross-desktop frontend now (accepting the
   GNOME positioning compromise), and add a native COSMIC applet later for the
   pixel-perfect anchored dropdown — without rewriting the core. See `PLAN.md`.
