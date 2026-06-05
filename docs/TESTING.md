# Test strategy — Voz

Target: a **well-tested, production-grade** app. The bulk of the logic is in
`voz-core` (pure Rust, no UI), which is where most automated testing lives; the
thin Tauri/webview layer gets focused E2E + visual coverage.

## 1. Test pyramid

### Unit (fast, deterministic) — `voz-core`
- **Audio**: `rubato` resample correctness (known sine → expected rate/length);
  stereo→mono downmix; i16→f32 conversion; WAV writer round-trip (`hound`).
- **VAD**: segmentation boundaries on fixtures (speech/silence); end-of-utterance.
- **Attribution**: merge mic("Me") + monitor("Them") by timestamp → expected turn
  order, including overlap handling.
- **Markdown serialization**: golden/snapshot tests (`insta`) for the refined note,
  raw note, YAML front-matter, and the `[[wikilink]]` — exact bytes.
- **Config**: load/save round-trip; **schema migration** from older versions;
  invalid/hand-edited config rejected gracefully.
- **Lossless guard / drift**: synthetic raw vs refined pairs trip / don't trip at
  the expected thresholds; entity/number-drop detection.
- **Job queue state machine**: Idle→Recording→(Stop)→Queued→Transcribing→Refining→
  Done/Failed; record-while-processing; cancel; retry; bounded concurrency.
- **Filenames/paths**: title derivation, collision handling, atomic-write temp→rename.

### Integration (medium) — `voz-core` wired together
- **Pipeline on a fixture**: a checked-in short WAV (mono + a 2-speaker clip) →
  transcribe with a **tiny bundled model** → refine via a **deterministic mock
  Refiner** → assert the two notes are written, raw first, linked correctly.
  Whisper output is asserted by **normalized fuzzy match / WER ≤ threshold** (ASR
  isn't bit-identical across hardware), not exact string.
- **Refiner contract tests**: one suite run against each backend via the trait —
  mock subprocess for Claude Code/Codex (assert argv + stdin, never a shell),
  a stub HTTP server for Ollama/Claude API. Timeouts, oversize output, non-zero
  exit, and "backend missing" all handled.
- **Crash recovery**: enqueue jobs, kill the process mid-refine, restart → jobs are
  persisted and resume; raw note already on disk is never lost.
- **Offline assertion** (also a security test): refine=None/Ollama opens no outbound
  sockets.

### UI / E2E (slower) — Tauri app
- **Visual regression**: the existing Playwright render of every `panel-*.html`
  (already in `design/render.mjs`) becomes a snapshot gate so the UI can't drift
  unintentionally.
- **E2E flows** via `tauri-driver` + WebDriver: record→stop→a job appears in
  History→opens detail→note saved; switch source; change a setting and confirm it
  persists; re-refine. Audio is fed from a virtual PipeWire source/fixture.
- **Accessibility checks**: keyboard-only path, focus order, reduced-motion,
  contrast, ARIA labels on controls (axe in the webview).

### Performance (benches, non-gating) — `criterion`
- Resample throughput; transcription real-time-factor on the default model per
  backend; memory ceiling on a 60-min recording. Tracked to catch regressions.

## 2. Tooling
`cargo nextest` (runner) · `insta` (snapshots) · `proptest` (resampler/parsers) ·
`mockall`/trait stubs (Refiner, Transcriber) · `wiremock` (HTTP backends) ·
`cargo-fuzz` (WAV / front-matter / markdown sanitizer) · `tauri-driver` + Playwright
(E2E + visual) · `criterion` (benches) · `cargo-llvm-cov` (coverage) ·
`cargo-deny` + `cargo-audit` (supply chain).

## 3. Fixtures & determinism
- Small **public-domain audio** clips committed (one single-speaker, one 2-speaker
  "meeting"); a **tiny whisper model** cached in CI for the integration lane.
- ASR non-determinism handled by fuzzy/WER assertions; all *logic* tests mock the
  transcriber so they stay deterministic. Real-whisper runs in a separate **"slow"
  CI lane**, not on every PR.

## 4. CI (GitHub Actions)
- **PR lane**: `fmt` → `clippy -D warnings` → unit + integration (`nextest`) →
  coverage gate → `cargo-deny`/`cargo-audit` → Playwright visual snapshots.
- **Slow/nightly lane**: real-whisper integration, fuzz (timeboxed), benches,
  full E2E with `tauri-driver`.
- **Release lane**: build + package Flatpak / `.deb` / AppImage; smoke-launch each
  on a clean GNOME and (where feasible) COSMIC image; attach artifacts.
- **Coverage target**: ≥ 80% lines on `voz-core` business logic (audio/pipeline/
  store/refine/config), enforced; UI excluded from the number but covered by E2E.

## 5. Manual QA matrix (pre-release)
Desktops: **Pop!_OS GNOME** + **COSMIC**. Audio routes: built-in mic · USB mic ·
**Bluetooth headset** · speakers→monitor · headphones→monitor (echo check).
Backends: CPU · Vulkan · CUDA (if available). Refine: Claude Code · Codex · Ollama ·
Claude API · None. Scenarios: **clean-machine first run (zero-setup)**, no-network,
long (60-min) meeting, record-while-processing, crash-mid-job recovery, vault on an
external/synced folder. Each tracked in a checklist tied to the Definition of Done
in `PLAN.md`.
