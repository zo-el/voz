# Voz — product roadmap

From the working v0.1.0 (local record → transcribe → refine → Obsidian notes, tray
GUI) to a polished product a stranger can install and use with zero friction, and
beyond. Each item lists **why**, rough **effort** (S/M/L), and **dependencies**.

Status legend: ✅ done · 🚧 in progress · ⬜ planned.

---

## Where we are (v0.1.0)
✅ `voz-core` engine (capture → transcribe → attribute → refine → store → history),
59 tests, clippy-clean · ✅ dual-source PipeWire capture · ✅ local whisper.cpp ·
✅ refine backends (Claude Code / Codex / Ollama / Claude API) · ✅ two linked
Obsidian notes + SQLite history · ✅ Tauri tray GUI (movable window, tray menu,
interactive+persisted settings, openable history) · ✅ crash-recovery spool ·
✅ zero-setup model auto-fetch · ✅ global hotkey (X11) · ✅ `.deb` package.

**The honest gap to "a stranger installs it and it just works":** onboarding,
graceful fallbacks when optional tools are missing, robust packaging across distros,
every error state handled, and the Wayland/COSMIC story. That's Phase 1.

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
3. ⬜ **Bundle a model / bulletproof first-run download** (M) — ship a small default
   model in the package *or* download on first run with a real progress bar, resume
   on failure, checksum, and a model picker (Fast/Balanced/Accurate with sizes).
   *Why:* transcription must work offline immediately, no manual `curl`.
4. 🚧 **Every failure has a UI state** (M) — no mic, no monitor (Bluetooth/headset
   without a monitor), disk full / unwritable vault, backend down/timeout, model
   missing, no network. Each shows a clear message + the fix. *Why:* strangers hit
   edge cases; silence = "broken".
5. ⬜ **Packaging** (L) — declare runtime deps in the `.deb` (PipeWire, `xdg-utils`);
   ship **AppImage** (portable) and a **Flatpak** (Flathub-ready, sandboxed);
   publish checksums and a signed **GitHub Release** via CI. *Why:* one trusted
   install path per distro. *Dep:* CI release pipeline.
6. 🚧 **CI release pipeline** (M) — ✅ `.github/workflows/release.yml`: a push to
   `main` with a bumped version auto-builds the `.deb` + AppImage, gates on the core
   tests, and publishes a `v<version>` GitHub Release with checksums. ⬜ Remaining:
   add Flatpak to the matrix, a slow test lane, and artifact signing.
7. ⬜ **GNOME tray reliability + docs** (S) — verify the AppIndicator path across
   GNOME versions; detect a missing/disabled extension and guide the user; keep the
   app fully usable via window + hotkey without the tray.
8. 🚧 **Accessibility pass** (M) — full keyboard path, focus order, ARIA labels,
   screen-reader test, contrast/AA, reduced-motion (✅ already), large-text. *Why:*
   production apps are accessible.
9. ⬜ **Logging, diagnostics, crash safety** (S) — local rotated logs (no telemetry),
   a "copy diagnostics" button (redacted), panic-safe worker threads. *Why:*
   supportable without phoning home.
10. ⬜ **Cross-distro QA matrix** (M) — Pop!_OS GNOME (X11), Ubuntu, Fedora
    Workstation, COSMIC; audio routes (built-in/USB/Bluetooth/speakers/headphones);
    CPU + GPU. *Why:* "works on my machine" ≠ shippable.
11. ⬜ **Updates** (M) — Flathub auto-updates; for `.deb`/AppImage, an in-app update
    check against the GitHub Release feed (signed; never executes fetched code).

**Exit criterion for "production-ready":** items 1–10 done, the Definition of Done
in `PLAN.md §7` met, and a clean-VM install→record→note succeeds with no terminal.

---

## Phase 2 — Core UX polish
12. ✅ **In-app note detail view** (M) — open a History note inside the panel: Raw/
    Refined toggle, Copy, Open in Obsidian, **Re-refine** with another style, delete.
    (Today: opens the file externally.)
13. ⬜ **True tray-anchored dropdown** (L) — X11: position the frameless panel under
    the tray icon; Wayland/COSMIC: a native applet (see Phase 4). *Why:* the original
    "the tray icon *is* the app" vision. *Dep:* per-desktop positioning.
14. ⬜ **Global hotkey everywhere** (M) — GNOME 48 GlobalShortcuts portal; COSMIC
    custom-shortcut fallback; push-to-talk *and* toggle. (Today: X11 only.)
15. ⬜ **Live streaming partials** (L) — VAD (Silero) + chunking so text appears while
    you speak, finalizing on silence. *Why:* feels instant. *Dep:* VAD integration.
16. ⬜ **Real waveform + capture polish** (S) — drive the waveform from real per-source
    levels; smooth pause/resume; elapsed/words live.
17. 🚧 **Editable refinement prompt + style presets UI** (S) — Adaptive/Meeting/Memo/
    Custom, editable in Settings, with a live preview. *Dep:* none.
18. ✅ **Model management UI** (M) — list/download/switch/delete models with sizes and
    a CPU/Vulkan/CUDA backend selector; show disk usage. *Dep:* #3.
19. ✅ **Settings: full coverage** (M) — hotkey rebind, theme (incl. light), language,
    audio-device picker, keep-audio, output format (md/txt/srt), Obsidian options.

---

## Phase 3 — Power features
20. ⬜ **GPU acceleration — auto-detect + choose** (M) — the app should use the best
    backend available and let the user override.
    - **Auto-detect at runtime:** on launch, probe for a usable GPU and pick the
      fastest backend, else CPU. Surface what was chosen in Settings.
    - **Manual override (already designed):** the *Acceleration* control
      (Auto / CPU / Vulkan / CUDA) maps to `config.transcription.accel` and the
      whisper context params; "Auto" = the detection above. Wire it in the live app.
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
22. ⬜ **Paste-at-cursor dictation** (M) — type into the focused app (Wayland
    wtype/ydotool, X11 xdotool); per-app modes.
23. ⬜ **Modes / profiles** (M) — per-context bundles {hotkey + model + refine prompt +
    auto-activation by app}. The superwhisper/VoiceInk pattern.
24. ⬜ **Watch-folder / file import / batch** (M) — drop audio/video files or watch a
    folder; transcribe meetings recorded elsewhere.
25. ⬜ **Full-text search + history power tools** (S) — search all transcripts, filter
    by source/speaker/date, bulk export (SRT/VTT/TXT/MD).
26. ⬜ **Deeper Obsidian integration** (M) — templates, tags, daily-note appends,
    backlinks, an optional companion plugin.

---

## Phase 4 — Platform & ecosystem
27. ⬜ **COSMIC native applet** (L) — the true Wayland tray-anchored dropdown using
    libcosmic, reusing `voz-core`. *Why:* COSMIC is a primary target.
28. ⬜ **macOS / Windows** (L) — Tauri is cross-platform; the core is portable
    (whisper.cpp CoreML/Metal on macOS). Big market expansion.
29. ⬜ **Flathub + distro packages** (M) — AUR, Fedora COPR, official Flathub listing.
30. ⬜ **Localization** (M) — translate the UI; the engine already does ~99 languages.
31. ⬜ **Optional local-first sync/backup** (L) — encrypted, user-controlled; never a
    server requirement.

---

## Cross-cutting (every phase)
- ⬜ **Security**: fuzz parsers (WAV/front-matter/markdown), `cargo-audit`/`deny` gate,
  pre-release `SECURITY.md` checklist, dependency review. Keep the "transcript is
  data, never code" invariant.
- ⬜ **Testing**: coverage gate ≥ 80% on `voz-core`; slow lane (real whisper, E2E via
  `tauri-driver`, visual regression); perf budgets (RTF, memory on long meetings).
- ⬜ **Privacy**: stays local-first; no telemetry; "offline mode opens no sockets"
  asserted in CI; clear data/consent docs.
- ⬜ **Docs & site**: user guide, troubleshooting, a simple landing page, screenshots.

---

## Suggested near-term order
1. Phase 1 #2 (backend fallback) + #4 (error states) + #5 (`.deb` deps) — *make the
   current build not-scary for a stranger.*  ← starting now
2. Phase 1 #1 (onboarding) + #3 (model UX).
3. Phase 1 #5/#6 (AppImage/Flatpak + CI release) → first public release.
4. Phase 2 #12 (note detail) + #14 (portal hotkey) + #19 (settings coverage).
5. Phase 3/4 as the audience grows.
