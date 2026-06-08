# Build plan — Voz

Status: **all milestones M0–M7 delivered** (v0.1.0 shipped, then a large feature
pass). This is the original build plan and Definition of Done — kept for the scope,
decisions, and DoD rationale. **For the current, living state of the project see
[`ROADMAP.md`](ROADMAP.md)** (what's done, in progress, and the disposition of the
rest).

Delivered (each committed on `develop`, clippy `-D warnings` clean, fmt clean):
- **M0** scaffold + tested `voz-core` · **M1** dual-source capture (pw-record;
  verified live) · **M2** whisper.cpp transcription (verified on jfk.wav, + CUDA on
  an RTX 3080) · **M3** refine backends (Claude Code verified end-to-end) · **M4**
  two linked notes + WAV + SQLite history · **M5a** engine (record→bg job→save→index)
  · **M5b** Tauri app + tray + SPA · **M6** packaging (.deb/AppImage + CI release) ·
  **M7** hardening (logging/diagnostics, accessibility, CI slow lane, supply-chain
  gate). 71 `voz-core` tests.

**Decisions locked (2026-06-05):**
- **Name:** Voz.
- **Frontend:** Tauri v2 (path A) — one cross-desktop codebase; native COSMIC
  applet deferred to v2. **Targets both GNOME and COSMIC.**
- **Default refine backend:** Claude Code CLI (all other backends remain available
  in settings).
- **Theme:** dark olive green (dark only for now; light theme later).
- **No modes.** Every recording yields **Raw + Refined** (two linked notes); the
  refined note is detailed, non-lossy notes; raw is the source of truth.
- **Audio:** capture mic + system loopback; **Both** is the default (local meeting
  capture, no meeting integration); Me/Them speaker attribution. **Compact source
  selector.**
- **Output:** configurable save path (default an Obsidian vault); Obsidian-friendly
  Markdown (front-matter + `[[wikilink]]`). Refined note shape = **Adaptive**.
- **Recording ≠ processing:** Stop hands off to a background job queue; you can
  record while notes process. **History** is the activity+notes hub; the **tray
  icon shows state** (idle/recording/processing); **notification** on completion.
  Paste-at-cursor deferred to v2.
- **Zero-setup install:** bundle whisper.cpp + a default model; nothing manual.
- **One settings entry** (bottom nav; no header gear).
- **Next:** continue refining design + docs together before writing app code.

---

## 1. Scope of v1 (MVP)

A tray-resident app that can:
1. Show a **state-aware tray icon** (idle / recording=red / processing=olive) and
   open the panel (frameless, dismiss-on-blur). One settings entry (bottom nav).
2. Record from **mic, system audio (loopback), or both** via a **compact source
   selector** (start / pause / stop / cancel) with a live level/waveform. **Both**
   is the default so meetings record locally — Voz never connects to the meeting.
3. **Decouple recording from processing:** Stop enqueues a background job and frees
   the recorder immediately, so you can **record while previous notes transcribe**.
   Jobs show in History; a **desktop notification** fires when a note is ready.
4. Transcribe locally with whisper.cpp (`large-v3-turbo q5_0` default), tagging
   **Me (mic) vs Them (system)**.
5. Always produce **two linked notes — Raw + Refined**. Refine via a chosen backend
   (**Claude Code CLI default**, plus Codex CLI / Ollama / Claude API / None) into
   adaptive, non-lossy notes, with the lossless guard. Raw is saved first.
6. Save to a **user-chosen folder** (default an Obsidian vault) as Obsidian-friendly
   Markdown (front-matter + `[[wikilink]]` refined⟷raw) + optional WAV.
7. **History tab = activity + notes hub:** in-flight jobs (Transcribing/Refining)
   on top, completed notes below; tap to read or re-refine.
8. Settings: save path, output/Obsidian, audio sources, hotkey, model, acceleration,
   refine backend + style, theme.
9. Global push-to-talk hotkey (GNOME 48 portal; documented COSMIC fallback).
10. **Zero-setup install** — bundles whisper.cpp + a default model; nothing to
    install manually (see `ARCHITECTURE.md §10`).
11. Run on **both GNOME and COSMIC** (Tauri; native COSMIC applet is a v2 follow-up).

Out of v1 (later): light theme, paste-at-cursor dictation insert, real-time
streaming partials, ML diarization for >2 speakers, watch-folder / file import,
multi-language UI.

---

## 2. The one decision to make before coding: the frontend

| Path | GNOME UX | COSMIC UX | Effort | Look matches mockups |
|---|---|---|---|---|
| **A. Tauri v2 (recommended v1)** | tray icon + frameless panel placed near tray (not pixel-pinned) | same frameless panel | **Low** — 1 codebase | **Exact** (HTML/CSS → UI) |
| **B. Native split** (`voz-core` + libcosmic applet + GTK4/libadwaita) | tray + normal libadwaita window | **true anchored dropdown** | High — 2 frontends, 2 toolkits | Rebuilt twice, approximate |
| **C. libcosmic everywhere** | normal window (COSMIC-styled on GNOME) | anchored applet dropdown | Medium | Rebuilt in iced, approximate |

**Recommendation: A (Tauri) for v1, with `voz-core` kept clean so a native COSMIC
applet (B's COSMIC half) can be added in v2 for the pixel-perfect anchored
dropdown.** Rationale: on GNOME the anchored popover is impossible for *every*
framework, so B/C's headline advantage only materializes on COSMIC; Tauri gives
the confirmed look on both desktops today from one codebase and the fastest path
to something you can use. The core stays reusable either way.

Pick A unless "pixel-perfect anchored dropdown on COSMIC in v1" outranks "ship the
confirmed UI fast on GNOME+COSMIC from one codebase." This is the question I'll ask
before building.

---

## 3. Milestones

**M0 — Repo + skeleton (0.5 day)**
- Cargo workspace: `voz-core` (lib) + `voz-app` (Tauri). CI (fmt/clippy/test).
- Wire the confirmed mockups into the Tauri webview as static views.

**M1 — Dual-source audio capture + recorder (3–4 days)**
- `cpal` capture of the **mic** and the **default-sink monitor** (PipeWire
  loopback); device/source enumeration; ring buffers; per-source RMS → live
  waveform.
- `rubato` resample to 16 kHz mono f32 per source; WAV writer (mixdown / 2-track).
- Source selector (Mic / System / Both, default Both); recorder state machine;
  start/pause/stop/cancel wired to the panel buttons.
- *Demo:* play a video + talk, watch both levels move, save a WAV.

**M2 — Local transcription + attribution (2–3 days)**
- `whisper-rs` integration; **bundled default model** + optional extra-model
  manager. Acceleration detection (CPU/Vulkan/CUDA) + manual override.
- Transcribe each source; **merge into a Me/Them speaker-tagged raw transcript**;
  persist the raw note first as the source of truth.
- *Demo:* speak → get attributed text, fully offline on a fresh install.

**M3 — Background job queue + refine pipeline (3–4 days)**
- **Job queue:** Stop → enqueue job → recorder returns to Idle (record-while-process);
  worker pool drains it; `JobState` events. This is the keystone of the new UX.
- `Refiner` trait + Claude Code CLI (default), Codex CLI, Ollama, Claude API, None.
- Adaptive refinement style/prompt (editable); lossless guard; streaming tokens.
- *Demo:* start a recording while a previous one is still refining.

**M4 — History hub, storage, Obsidian output (2–3 days)**
- **History tab** = Processing group (live jobs w/ progress) + completed notes;
  detail views (processing + finished); tap to read / re-refine.
- **Two linked Markdown notes** (refined + raw), YAML front-matter + `[[wikilink]]`;
  optional WAV; `history.sqlite` index + search; Copy + Open in Obsidian.
  *(Paste-at-cursor dictation insert is deferred to v2.)*

**M5 — Tray state, notifications, settings, hotkey (2–3 days)**
- **State-aware tray icon** (idle / recording / processing badge) driven by
  `Event::Tray`; **desktop notification** on note-ready; frameless panel
  show/hide-on-blur; launch-on-login. Single settings entry (bottom nav).
- Settings screens fully wired (save path/Obsidian, audio sources, model, accel,
  refine backend + style, theme).
- Global hotkey via GlobalShortcuts portal (GNOME 48); COSMIC fallback documented.

**M6 — Zero-setup packaging (2–3 days)**
- **Flatpak (Flathub) bundling whisper.cpp + the default model** (works offline on
  first launch) + PipeWire portal; `.deb` + AppImage; CPU+Vulkan build, CUDA variant.
  One-click Ollama setup for that backend.
- Onboarding (source pick, refine backend, AppIndicator check on non-Pop GNOME;
  system-audio consent note). Reduced-motion, keyboard paths, error states.

**M7 — Hardening, testing, CI/release, security (3–4 days)** — *the "production"
milestone; runs alongside M1–M6, not just after*
- Test suite per **`TESTING.md`** (unit + integration + Refiner contract + crash-
  recovery + E2E + visual-regression); coverage gate ≥ 80% on `voz-core`.
- Security controls per **`SECURITY.md`** (CSP + capability allowlist, no-shell
  subprocess, secret-service key, model checksums, minimal Flatpak perms,
  `cargo-deny`/`audit`, injection + offline-mode tests, fuzzing).
- **GitHub Actions** CI (PR / nightly-slow / release lanes); resilience (atomic
  writes, persisted job queue, graceful degradation); local logging + diagnostics;
  config migration; accessibility pass; signed update channels.

**Definition of Done (production readiness):** see §7 below — M7 gates the release.

**v2 candidates:** native COSMIC applet (anchored dropdown) reusing `voz-core`;
light theme; paste-at-cursor dictation; real-time streaming; ML diarization for
>2 speakers; watch-folder / file import.

Rough estimate: ~4.5–6 focused weeks for M0–M7 (dual-source capture, the job queue,
zero-setup packaging, and the full test/security pass each add time — this is the
cost of "production, secure, well-tested" rather than a prototype).

---

## 4. Key dependencies (Rust)
`tauri` v2 · `whisper-rs` (Codeberg) · `cpal` · `rubato` · `hound` (WAV) ·
`voice_activity_detector`/`sherpa-rs` (VAD, optional) · `tokio` · `serde`/`toml` ·
`rusqlite` · `keyring` (secrets) · `ashpd` (xdg portals: global shortcuts) ·
`reqwest` (Ollama/Claude API/model download) · `tray-icon`.
Build needs a C/C++ toolchain + ALSA dev headers (for cpal/whisper.cpp).

---

## 5. Risks & mitigations
- **whisper-rs moved to Codeberg / build complexity** → pin Codeberg source; CPU
  build first, gate GPU features behind flags; vendor the whisper.cpp submodule.
- **GNOME anchored-dropdown impossible** → accept frameless-near-tray; set the
  expectation in onboarding; deliver true anchoring via the v2 COSMIC applet.
- **Global hotkey gaps on COSMIC** → document custom-shortcut fallback; consider
  opt-in evdev later (as Handy did).
- **AppIndicator extension absent on vanilla GNOME** (fine on Pop) → detect, guide,
  keep hotkey usable without the tray.
- **LLM refine hallucination / info loss** → "reorganize, never drop info" prompt +
  lossless guard + raw is always the recoverable source of truth.
- **System-audio loopback varies** (sink monitor naming, Bluetooth/headset routing,
  no monitor on some sinks) → enumerate PipeWire monitors robustly; fall back to
  mic-only with a clear message; test across speakers/headphones/BT.
- **Echo when on speakers** (your mic re-captures the meeting) → prefer headphones;
  optionally apply PipeWire echo-cancel; Me/Them attribution still separates streams.
- **Bundled-model size vs zero-setup** → bundling the model makes the *installer*
  large (~0.5 GB). Mitigation: bundle one good default, offer other sizes as
  optional in-app downloads; or ship a smaller default and auto-fetch the bigger one
  on first launch with a progress UI (still "no manual install"). Decide in §6.
- **Long meetings vs the job queue** → cap worker concurrency (1–2) so processing a
  long meeting doesn't starve a new recording; show queue position in History.

---

## 6. Decisions & remaining questions

**Resolved:** Tauri v2 · Name = Voz · default refine = Claude Code CLI · dark olive
theme (dark only) · no modes → Raw + Refined · dual-source capture, **Both is the
default** · Me/Them attribution · Obsidian-friendly output · target both GNOME and
COSMIC · **refined note = Adaptive** · **paste-at-cursor deferred to v2** ·
**recording decoupled from processing** (background job queue, History hub) ·
**state-aware tray icon + completion notification** · **compact source selector** ·
**one settings entry** · **zero-setup install** (bundle whisper.cpp + a model).

**Blocking decisions for autonomous build** (only you can set these):
1. **Goal & scope** — build full v1 (M0–M7) to the Definition of Done below,
   pausing only when truly blocked? (vs. a vertical slice first.)
2. **License** — MIT / Apache-2.0 / GPLv3 / proprietary-private. Affects every file
   header and distribution.
3. **Distribution** — personal/local only (`.deb`/AppImage/local Flatpak) or a
   public **Flathub** release? Public raises the security/QA and review bar.
4. **GPU** — do you have an NVIDIA GPU (CUDA), AMD/Intel (Vulkan), or CPU-only? Sets
   the default acceleration and which build variants we test.

**Non-blocking — I'll proceed with these defaults unless you override** (all
changeable later in-app):
- **Obsidian vault path:** detect a vault under `~`; fall back to `~/Recordings/Voz`.
- **File layout:** refined note + raw in a `raw/` subfolder, linked by `[[wikilink]]`.
- **Filename:** `YYYY-MM-DD HH-MM <derived title>.md`, flat per save folder.
- **Bundled model:** ship `small` (~466 MB) for instant offline first-run, and
  **auto-fetch `large-v3-turbo q5_0`** on first launch in the background (still no
  manual install); user can pick either in Settings.
- **Telemetry:** none. **Secrets:** OS secret service. **Theme:** dark olive.

---

## 7. Definition of Done (production readiness)
A release ships only when **all** of these hold (tracked against `TESTING.md` /
`SECURITY.md`):

- [ ] M0–M7 complete; all tests green; **coverage ≥ 80%** on `voz-core`.
- [ ] **No data loss:** raw written before refine; atomic writes; crash mid-job
      resumes; verified by tests.
- [ ] Every failure mode has a **defined UI error state** (no monitor, backend down,
      disk full, model missing, no network).
- [ ] **Security:** `cargo-deny`/`audit` clean; CSP + capability allowlist; no-shell
      subprocess; secrets in keyring; model checksums; minimal Flatpak perms;
      injection + offline-mode tests pass; `SECURITY.md` checklist signed off.
- [ ] **Zero-setup install verified** on a clean GNOME and COSMIC image (works
      offline on first launch).
- [ ] **Accessibility:** keyboard-complete, reduced-motion, contrast, labels.
- [ ] **Privacy:** offline mode opens no sockets; no telemetry; logs redacted.
- [ ] Manual QA matrix (desktops × audio routes × backends) passed.
- [ ] User docs (README, onboarding, permissions, consent note) complete; third-party
      licenses bundled.
