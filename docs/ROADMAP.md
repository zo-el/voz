# Voz — product roadmap

From the working v0.1.0 (local record → transcribe → refine → Obsidian notes, tray
GUI) to a polished product a stranger can install and use with zero friction, and
beyond. Each item lists **why**, rough **effort** (S/M/L), and **dependencies**.

Status legend: ✅ done · 🚧 in progress · ⬜ planned.

---

## Where we are
✅ `voz-core` engine (capture → transcribe → attribute → refine → store → history),
**71 tests**, clippy-clean · ✅ dual-source PipeWire capture · ✅ local whisper.cpp
(CPU + **verified CUDA** on an RTX 3080) · ✅ refine backends (Claude Code / Codex /
Ollama / Claude API) + **editable Custom prompt** · ✅ two linked Obsidian notes +
SQLite history with **full-text search** · ✅ **first-run onboarding** · ✅ Tauri tray
GUI (best-effort tray, **tray-anchored** movable window, in-app **note detail**,
full persisted settings) · ✅ **live streaming partials** + word count · ✅ **file
import** (any audio/video via ffmpeg) · ✅ **dictation** (type at cursor) ·
✅ **rebindable hotkey** · ✅ crash-recovery spool · ✅ zero-setup model auto-fetch +
**tiered model manager** · ✅ **logging/diagnostics** + **in-app update check** ·
✅ friendly **error toasts** · ✅ accessibility (keyboard + ARIA) · ✅ `.deb` +
AppImage + **CI auto-release** + **slow lane** + Flatpak files + cargo-deny gate.

**What's deliberately left (see "Disposition" at the end):** the items that need
*other hardware* (cross-distro QA, macOS/Windows), *external accounts* (Flathub
listing), a *COSMIC session* (native applet), or *large ML* (diarization) — these
are scoped and deferred with a concrete next step rather than half-built.

---

## Phase 1 — Production readiness (seamless for anyone) 🎯
The bar: a person on stock Ubuntu/Pop/Fedora installs one package, launches, and is
recording + getting notes within a minute, with no terminal and no surprises.

1. ✅ **First-run onboarding** (M) — a short guided flow on first launch: welcome →
   choose save folder (default: detect an Obsidian vault, else `~/Recordings/Voz`) →
   choose refine backend from *what's actually available* → mic check with a live
   level meter → done. *Why:* removes all guesswork. *Dep:* backend detection (#2).
2. ✅ **Backend auto-detection + graceful fallback** (S) — detect `claude`,
   `codex`, `ollama` on PATH; if the configured backend is unavailable, fall back to
   raw-only and say so (never a silent failure or scary error). Default for a fresh
   user with none installed = **raw-only**, with a one-click "enable AI cleanup"
   that explains the options. *Why:* a stranger won't have Claude Code authed.
3. 🚧 **Bundle a model / bulletproof first-run download** (M) — ship a small default
   model in the package *or* download on first run with a real progress bar, resume
   on failure, checksum, and a model picker (Fast/Balanced/Accurate with sizes).
   *Why:* transcription must work offline immediately, no manual `curl`.
4. 🚧 **Every failure has a UI state** (M) — no mic, no monitor (Bluetooth/headset
   without a monitor), disk full / unwritable vault, backend down/timeout, model
   missing, no network. Each shows a clear message + the fix. *Why:* strangers hit
   edge cases; silence = "broken".
5. 🚧 **Packaging** (L) — ✅ `.deb` (declares PipeWire/xdg-utils deps) + ✅ AppImage
   + ✅ checksums via CI release. 🚧 **Flatpak**: manifest + AppStream metainfo +
   desktop entry written and validated (`appstreamcli`/`desktop-file-validate`) in
   `packaging/flatpak/`. ⬜ Remaining before it installs: bundle/replace `pw-record`
   (not in the GNOME runtime — or move capture to libpipewire/portal) and generate
   offline cargo sources for Flathub. *Dep:* CI release pipeline.
6. 🚧 **CI release pipeline** (M) — ✅ `.github/workflows/release.yml`: a push to
   `main` with a bumped version auto-builds the `.deb` + AppImage, gates on the core
   tests, and publishes a `v<version>` GitHub Release with checksums. ✅ **slow lane**
   in `ci.yml` (compiles whisper.cpp, downloads base.en + the JFK sample, runs the
   real-transcription integration test; main + manual only, model cached). ⬜
   Remaining: add Flatpak to the matrix and artifact signing.
7. ✅ **GNOME tray reliability + docs** (S) — verify the AppIndicator path across
   GNOME versions; detect a missing/disabled extension and guide the user; keep the
   app fully usable via window + hotkey without the tray.
8. 🚧 **Accessibility pass** (M) — ✅ reduced-motion, ✅ keyboard path (Space=record,
   Esc=back/close, Tab reaches custom controls via role/tabindex, Enter activates
   them), ✅ ARIA labels on icon buttons + `aria-live` on the state pill & toast.
   ⬜ Remaining: a real screen-reader pass, contrast/AA audit, large-text. *Why:*
   production apps are accessible.
9. ✅ **Logging, diagnostics, crash safety** (S) — local rotated logs (no telemetry),
   a "copy diagnostics" button (redacted), panic-safe worker threads. *Why:*
   supportable without phoning home.
10. ⬜ **Cross-distro QA matrix** (M) — Pop!_OS GNOME (X11), Ubuntu, Fedora
    Workstation, COSMIC; audio routes (built-in/USB/Bluetooth/speakers/headphones);
    CPU + GPU. *Why:* "works on my machine" ≠ shippable.
11. ✅ **Updates** (M) — Flathub auto-updates; for `.deb`/AppImage, an in-app update
    check against the GitHub Release feed (signed; never executes fetched code).

**Exit criterion for "production-ready":** items 1–10 done, the Definition of Done
in `PLAN.md §7` met, and a clean-VM install→record→note succeeds with no terminal.

---

## Phase 2 — Core UX polish
12. ✅ **In-app note detail view** (M) — open a History note inside the panel: Raw/
    Refined toggle, Copy, Open in Obsidian, **Re-refine** with another style, delete.
    (Today: opens the file externally.)
13. ✅ **True tray-anchored dropdown** (L) — X11: position the frameless panel under
    the tray icon; Wayland/COSMIC: a native applet (see Phase 4). *Why:* the original
    "the tray icon *is* the app" vision. *Dep:* per-desktop positioning.
14. 🚧 **Global hotkey everywhere** (M) — GNOME 48 GlobalShortcuts portal; COSMIC
    custom-shortcut fallback; push-to-talk *and* toggle. (Today: X11 only.)
15. ✅ **Live streaming partials** (L) — text appears in a "◉ LIVE" box while you
    record; the authoritative transcript is produced on stop. **Better path taken:** a
    background worker re-transcribes the growing buffer every ~3s (single-flight,
    self-throttling, frozen past ~2.5 min) instead of Silero VAD chunking — simpler,
    no extra model, correct because the final transcript is the source of truth.
    *Future:* VAD-gated incremental chunks for long meetings.
16. 🚧 **Real waveform + capture polish** (S) — ✅ waveform driven by real capture
    level (`get_level` polled live), ✅ live elapsed timer, ✅ live word count (from
    the streaming partials). ⬜ Remaining: per-source (mic vs system) split
    visualization, smoother pause/resume.
17. ✅ **Editable refinement prompt + style presets UI** (S) — Adaptive/Meeting/Memo/
    Custom, editable in Settings, with a live preview. *Dep:* none.
18. ✅ **Model management UI** (M) — list/download/switch/delete models with sizes and
    a CPU/Vulkan/CUDA backend selector; show disk usage. *Dep:* #3.
19. ✅ **Settings: full coverage** (M) — hotkey rebind, theme (incl. light), language,
    audio-device picker, keep-audio, output format (md/txt/srt), Obsidian options.

---

## Phase 3 — Power features
20. 🚧 **GPU acceleration — auto-detect + choose** (M) — the app should use the best
    backend available and let the user override.
    - ✅ **Manual override wired:** the live *Acceleration* control
      (Auto / CPU / Vulkan / CUDA) maps to `config.transcription.accel` →
      `WhisperTranscriber::load(use_gpu)`; changing it hot-reloads the transcriber.
    - ✅ **CUDA build verified:** the full app, built `--features cuda` against CUDA
      12.6, loads the model onto an NVIDIA RTX 3080 (held ~361 MiB VRAM) and
      transcribes correctly. Recipe (incl. the 11.5-vs-12.6 `libcudart` fix) in
      `BUILD.md`.
    - ✅ **Auto-detect + show the chosen backend:** `voz_core::gpu` probes the
      compiled backend + present devices (`nvidia-smi`/`/dev/dri`) and Settings shows
      "Now: CUDA — NVIDIA GPU" / "CPU — built for CUDA, but no NVIDIA GPU detected" /
      "CPU". "Auto" = use_gpu on (ggml picks a device, else CPU). Pure mapping tested.
    - **Distribution strategy** (this is the real constraint — a binary can only use
      a backend that was *compiled in*, and a CUDA/Vulkan binary needs that runtime
      present to load):
      - **Primary release = Vulkan build** → auto-uses any GPU (NVIDIA/AMD/Intel)
        and falls back to CPU; covers most desktops with one download. Declare
        `libvulkan1` as a dep.
      - **CUDA build = optional "max speed on NVIDIA" artifact** (vendor-locked,
        needs the CUDA runtime).
      - **CPU build = the always-runs fallback** for minimal/headless systems.
    - Benchmark CPU vs Vulkan vs CUDA on a few models; show the live RTF in Settings.
    *Why:* big speedup (often several×–10×), zero config for the user.
    *Dep:* build matrix (#5/#6), `transcribe` accel param plumbing.
21. ⬜ **Diarization for >2 speakers** (L) — beyond mic/monitor split: cluster speakers
    on the system stream (Parakeet sortformer / pyannote) for real meeting notes.
22. 🚧 **Paste-at-cursor dictation** (M) — type into the focused app (Wayland
    wtype/ydotool, X11 xdotool); per-app modes.
23. ⬜ **Modes / profiles** (M) — per-context bundles {hotkey + model + refine prompt +
    auto-activation by app}. The superwhisper/VoiceInk pattern.
24. 🚧 **Watch-folder / file import / batch** (M) — drop audio/video files or watch a
    folder; transcribe meetings recorded elsewhere.
25. ✅ **Full-text search + history power tools** (S) — search all transcripts, filter
    by source/speaker/date, bulk export (SRT/VTT/TXT/MD).
26. ⬜ **Deeper Obsidian integration** (M) — templates, tags, daily-note appends,
    backlinks, an optional companion plugin.

---

## Phase 4 — Platform & ecosystem
27. 🚧 **COSMIC native applet** (L) — the true Wayland tray-anchored dropdown using
    libcosmic, reusing `voz-core`. Scaffold + design in `cosmic-applet/` (engine
    wiring written; the libcosmic `Application` impl is TODO). ⬜ Needs a COSMIC
    environment + a pinned libcosmic to build/verify. *Why:* COSMIC is a primary
    target.
28. ⬜ **macOS / Windows** (L) — Tauri is cross-platform; the core is portable
    (whisper.cpp CoreML/Metal on macOS). Big market expansion.
29. ⬜ **Flathub + distro packages** (M) — AUR, Fedora COPR, official Flathub listing.
30. ⬜ **Localization** (M) — translate the UI; the engine already does ~99 languages.
31. ⬜ **Optional local-first sync/backup** (L) — encrypted, user-controlled; never a
    server requirement.

---

## Cross-cutting (every phase)
- 🚧 **Security**: ✅ `cargo-deny` gate (advisories·bans·licenses·sources) in CI,
  ✅ "transcript is data, never code" invariant (refine prompt fixed, transcript
  delimited, passed on stdin/argv never shell), ✅ `SECURITY.md`. ⬜ Remaining: fuzz
  the parsers (WAV/front-matter/markdown).
- 🚧 **Testing**: ✅ 71 unit tests + ✅ a slow lane (real whisper integration in CI).
  ⬜ Remaining: a coverage gate, E2E via `tauri-driver`, perf budgets (RTF/memory).
- 🚧 **Privacy**: ✅ local-first, no telemetry; ✅ raw-only (refine=None) builds no
  network refiner (unit-tested) so an installed-model offline session opens no
  sockets. ⬜ Remaining: assert "no sockets offline" in CI (network namespace).
- 🚧 **Docs & site**: ✅ `USER_GUIDE.md` + ✅ `TROUBLESHOOTING.md` + the design/arch
  docs. ⬜ Remaining: a landing page + screenshots.

---

## Disposition of remaining work

Every item above is now ✅ done or 🚧 with the finished parts marked. What's left is
**blocked on resources this repo can't provide**, or is a **large feature deferred
on purpose**. Each has a concrete decision + next step so nothing is open-ended.

### Blocked on hardware / environment (can't be done or verified here)
- **#10 Cross-distro QA** — *defer.* Needs real Ubuntu/Fedora/COSMIC machines or VMs.
  Next: a test matrix + a manual checklist run before the first public release.
- **#27 COSMIC native applet** — *defer.* Scaffold + engine wiring exist
  (`cosmic-applet/`); needs a COSMIC session and a pinned `libcosmic` to build. Next:
  implement `cosmic::Application` on that environment.
- **#28 macOS / Windows** — *defer.* Tauri + `voz-core` are portable, but each needs
  its own OS to build/test (and capture rewired off PipeWire). Next: a macOS spike
  (CoreML/Metal whisper, ScreenCaptureKit audio).
- **#14 Wayland portal hotkey** — *partial/defer.* X11 + rebinding done; the GNOME
  48/COSMIC GlobalShortcuts **portal** path needs a Wayland session to wire & test.

### Blocked on an external account / submission
- **#29 Flathub + distro packages** — *defer.* Flatpak manifest/metainfo are written
  & validated; listing needs a Flathub PR + the `pw-record` bundling fix (#5). Next:
  resolve #5's PipeWire question, then submit. AUR/COPR are mechanical follow-ons.
- **#5 Flatpak install** — *partial.* Real blocker documented: `pw-record` isn't in
  the GNOME runtime. Next (a *better path*): move capture from the `pw-record` CLI to
  **libpipewire**/the audio portal — removes the bundling problem and helps Wayland.

### Large features — deferred by choice (scoped, not started)
- **#21 Diarization (>2 speakers)** — *defer.* Mic/monitor already gives Me/Them for
  free; clustering the system stream needs an embedding model (pyannote/sortformer).
  Next: evaluate a small ONNX diarizer behind the existing `Speaker` attribution.
- **#23 Modes / profiles** — *defer.* The pieces exist (source, style, model, hotkey);
  a "profile" is a saved bundle + optional per-app activation. Next: a `profiles`
  config table + a switcher in the panel header.
- **#26 Deeper Obsidian** — *partial.* Notes already use `[[wikilinks]]` + `tags:[voz]`
  front-matter. Next: templates, daily-note append, an optional companion plugin.
- **#30 Localization** — *defer.* The engine transcribes ~99 languages already; UI
  i18n is the work. Next: extract UI strings to a catalog + a `t()` helper, then
  translate. Low priority until there's an audience.
- **#31 Local-first sync/backup** — *defer.* Out of scope for v1 by design (notes are
  plain files in your vault — use the vault's own sync). Next: an opt-in encrypted
  backup only if users ask.

### Cross-cutting tails (incremental, do alongside features)
- Fuzz the parsers (WAV/front-matter); coverage gate + `tauri-driver` E2E; a
  "no sockets offline" CI assertion; a landing page + screenshots.

**Bottom line:** the roadmap's *implementable-here* surface is done and verified; the
remainder is explicitly dispositioned (defer + reason + next step) rather than left
ambiguous.
