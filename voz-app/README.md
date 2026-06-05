# voz-app — the Tauri shell (thin frontend)

This is the desktop app: a tray icon + a frameless panel webview that renders the
confirmed UI and forwards user intents to `voz-core`. It contains **no business
logic** — see `docs/ARCHITECTURE.md §2`.

- `ui/` — the web frontend. Currently the confirmed design from `design/mockups`
  copied in as static views (`index.html` = the Record panel). Milestones M1–M5
  turn these into a small single-page app wired to the engine's `Event` stream.
- `src-tauri/` — the Rust side: maps Tauri commands → `voz_core::Command`, forwards
  `voz_core::Event` to the webview, owns the tray icon state and the
  show/hide-on-blur panel window.

Excluded from the workspace build until the native deps are installed (see
`../BUILD.md`), so `cargo test` on `voz-core` stays green with zero system deps.

Run (after prerequisites): `cargo tauri dev`
