# Security model — Voz

Voz handles sensitive data (private audio, meeting recordings of other people,
transcripts, and possibly an API key) and runs untrusted text through an LLM. This
document is the threat model and the controls we build to, so the app is
production-grade and auditable.

## 1. Assets & trust boundaries

| Asset | Sensitivity | Where it lives |
|---|---|---|
| Microphone + system audio | High (private; captures others) | RAM during capture; WAV on disk if "keep audio" |
| Raw / refined notes | High | user-chosen folder (e.g. Obsidian vault) |
| API key (Claude API backend only) | Critical | OS secret service (libsecret) — never in TOML/logs |
| Config | Medium | `~/.config/voz/config.toml` |
| Bundled/downloaded models | Integrity-sensitive | `~/.local/share/voz/models/` |

**Untrusted inputs** (treat as data, never as code/instructions):
1. **Transcript text** — derived from arbitrary speech; may contain prompt-injection
   ("ignore previous instructions…"), shell metacharacters, or HTML.
2. **LLM (refine) output** — model-generated; may contain anything, incl. HTML/JS.
3. **Model files** downloaded from the network.
4. Config files a user could hand-edit.

**Trust boundaries:** webview UI ↔ `voz-core` (Tauri IPC) · `voz-core` ↔ refine
subprocess/API · `voz-core` ↔ network (model download / cloud refine) · app ↔ OS
(audio, secret service, filesystem).

## 2. Controls

### 2.1 Webview / UI (XSS is the top risk)
- The UI **renders transcript and LLM output as text, never as HTML** — set via
  text nodes / framework escaping; no `innerHTML` of untrusted strings; markdown is
  rendered with a sanitizing renderer (allowlist, no raw HTML, no inline scripts).
- **Strict Tauri CSP**: `default-src 'self'`, no remote origins, no inline script
  except hashed; the only external fetch at runtime is the chosen backend, done in
  Rust, not the webview.
- **Minimal Tauri capability allowlist**: the webview can call only the explicit
  `Command`s in `voz-core`; no shell, fs, or http plugin is exposed to the
  frontend. All privileged work happens in Rust behind typed commands.

### 2.2 Refine subprocess (Claude Code / Codex CLI)
- Spawn with an **argv array, never a shell string** (`Command::new("claude").args([...])`)
  — no `sh -c`, so transcript content can't be interpreted as a command.
- Transcript is passed on **stdin** (or a temp file), not as an argument; the
  **prompt is fixed** and the transcript is clearly delimited as data.
- **Resource bounds**: timeout, output size cap, kill on overrun; the subprocess
  inherits no secrets via env beyond what's required.
- **Output is untrusted**: it's only ever written to a note and shown as text
  (§2.1); it is never executed, never used to build a command, path, or URL.
- **Prompt-injection stance**: injection can at worst produce a bad *note*, never
  code execution or data exfiltration — because the output has no privileged sink.
  The lossless guard + raw-as-truth keep a clean fallback.

### 2.3 Secrets
- Claude API key stored via the **OS secret service** (`keyring`/libsecret), never
  in `config.toml`. Redacted from all logs and error messages. Only read at the
  moment of an API call.

### 2.4 Network & model integrity
- Outbound connections only to: the selected refine backend, and (optional) model
  downloads. **None/Ollama ⇒ zero outbound** after install — this is asserted in a
  test.
- Model downloads: **HTTPS only**, pinned host (Hugging Face), **SHA-256 checksum
  verified** before use; partial/failed downloads discarded; resume supported.
- TLS via the platform/rustls defaults; no certificate bypass.

### 2.5 Filesystem & sandbox (Flatpak)
- **Least privilege**: request only `--socket=pipewire` (audio), `--share=network`
  (refine/download), notifications, autostart, and **secret service**.
- **No blanket `--filesystem=home`.** The save folder is granted via the **file-
  chooser/document portal** (user picks it once; access persisted by the portal),
  so Voz can only write where the user pointed it.
- Notes are written with **atomic writes** (temp file + rename) so a crash can't
  corrupt a vault file.

### 2.6 System-audio consent (legal/ethical)
- Recording system audio captures other people. Recording-consent law varies by
  jurisdiction — **this is the user's responsibility**, and the app makes the state
  unmistakable: explicit Mic/System/Both choice, a hard-to-miss "Recording" pill,
  and a **red tray dot even when the panel is closed**. Onboarding shows a one-time
  consent/legal note. No silent or background-without-indicator recording, ever.

### 2.7 Supply chain & build
- `Cargo.lock` committed; **`cargo-deny`** (license + advisory policy) and
  **`cargo-audit`** run in CI; deny on new advisories.
- whisper.cpp pinned to a vetted submodule commit; `whisper-rs` pinned to a
  Codeberg revision. Dependency review on bumps.
- Reproducible release builds where feasible; checked-in third-party license
  manifest (whisper.cpp MIT, model MIT, fonts, etc.).

### 2.8 Updates
- Updates flow through **signed channels**: Flathub (signed repo) / a signed apt
  repo for `.deb` / AppImage with zsync + signature. The app never fetches and
  executes arbitrary code to update itself.

### 2.9 Logging / privacy
- No telemetry, no crash phone-home. Local logs only, rotated, **transcript content
  is not logged** at normal levels; secrets/keys redacted. A "privacy mode" assert:
  with refine=None/Ollama the process opens no sockets (verified in test).

## 3. Security testing
- `cargo-audit` + `cargo-deny` gate in CI.
- **Fuzz** the parsers that touch untrusted bytes: WAV reader, YAML front-matter,
  markdown sanitizer (`cargo-fuzz`).
- **Injection tests**: a transcript containing shell metacharacters / "ignore
  instructions" / `<script>` must (a) not alter subprocess invocation, (b) be
  escaped in the UI, (c) never execute.
- **Offline assertion**: refine=None/Ollama ⇒ no outbound sockets (network namespace
  or socket-hook test).
- **CSP / capability** review test: webview cannot reach undeclared commands or
  remote origins.
- Pre-release manual security checklist (this doc) signed off.
