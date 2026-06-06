# Troubleshooting

Voz is designed to keep working even when a part of the desktop isn't cooperating.
Most issues fall into one of these.

## No tray icon
The tray is **optional** — Voz is fully usable without it via the window and the
global hotkey (shown in **Settings ▸ Record hotkey**, default `Ctrl+Super+Space`).

- **GNOME** renders tray icons through an *AppIndicator / StatusNotifier* extension.
  If you see no icon, install & enable one (e.g. "AppIndicator and KStatusNotifier
  Support" / "Ubuntu AppIndicators"), then log out/in. Voz logs a hint when no tray
  host is found (see **Logs** below) and keeps running regardless.
- **COSMIC / other Wayland**: a native applet is planned (`cosmic-applet/`); until
  then use the window + hotkey.

## Nothing is recorded / "Couldn't access audio"
Capture uses **PipeWire** (`pw-record`).
- Make sure PipeWire is running (`systemctl --user status pipewire`).
- Pick **Mic** in the source selector if you only want your microphone; **System**
  / **Both** also capture the meeting audio you hear via the default sink's monitor.
- Bluetooth headsets sometimes expose no monitor source — switch the source to Mic.

## AI cleanup didn't run (only the raw note)
The refined note needs a backend (Claude Code / Codex / Ollama / Claude API). If
none is installed, Voz falls back to **raw-only** on purpose — no error. Install one
and pick it in **Settings ▸ Refine backend**, or leave it raw-only.

## Importing a non-WAV file fails
File import decodes via **ffmpeg** (for mp3/m4a/mp4/opus/flac/…). Install it
(`sudo apt install ffmpeg`); WAV import works without it.

## Transcription isn't using my GPU
The published build is **CPU** (runs everywhere). GPU needs a build compiled with a
backend — see `BUILD.md` (CUDA/Vulkan). **Settings ▸ Acceleration** shows what the
running app is actually using ("Now: CUDA — NVIDIA GPU" / "CPU").

## Global hotkey doesn't work
- **X11**: should work out of the box; rebind it in Settings if it clashes.
- **Wayland (GNOME 48+/COSMIC)**: the compositor must grant global shortcuts via the
  portal — use the tray/window, or bind a custom shortcut in your desktop settings.

## Logs & diagnostics
- Log file: `~/.local/share/voz/voz.log` (local only, no telemetry) — **Settings ▸
  Support ▸ Log file ▸ Open**.
- **Settings ▸ Support ▸ Diagnostics ▸ Copy** copies a redacted summary (versions,
  GPU, settings — never your transcripts or save path) for bug reports.
