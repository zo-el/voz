# Building Voz

## What builds today, with nothing installed
`voz-core` is pure Rust and builds + tests immediately:

```bash
cargo test -p voz-core      # 38 tests
cargo clippy -p voz-core --all-targets -- -D warnings
cargo fmt --all -- --check
```

## System prerequisites for the native layers (M1+)
The audio capture (cpal/PipeWire), transcription (whisper.cpp), CUDA acceleration,
and the Tauri app need system `-dev` packages. On Pop!_OS / Ubuntu:

```bash
sudo apt-get update
sudo apt-get install -y \
  libasound2-dev libpipewire-0.3-dev \      # audio capture (mic + monitor)
  libwebkit2gtk-4.1-dev librsvg2-dev \      # Tauri webview
  build-essential cmake clang               # whisper.cpp build
```

CUDA (your RTX 3080) — for the GPU-accelerated whisper build:

```bash
# Install the CUDA Toolkit (nvcc). Driver 580 is already present.
sudo apt-get install -y nvidia-cuda-toolkit   # or the official CUDA repo for a newer toolkit
```

Tauri CLI + frontend tooling:

```bash
cargo install tauri-cli --version '^2'        # or: npm i -D @tauri-apps/cli
```

> In a Claude Code session you can run any of these yourself with a leading `!`,
> e.g. `! sudo apt-get install -y libasound2-dev ...`, so the output lands here.

## Feature flags (voz-core)
Native backends are gated so the core stays buildable without system deps:

| feature   | pulls in                         | needs                         |
|-----------|----------------------------------|-------------------------------|
| `audio`   | cpal + rubato + pipewire         | libasound2-dev, libpipewire   |
| `whisper` | whisper-rs / whisper.cpp         | cmake, clang                  |
| `vulkan`  | whisper.cpp Vulkan backend       | + Vulkan SDK                  |
| `cuda`    | whisper.cpp CUDA backend         | + CUDA Toolkit (nvcc)         |

Default build enables none of these (used by CI's fast lane and local logic tests).

## Running the app (after prerequisites)
```bash
cargo tauri dev      # from voz-app/  (added/wired in milestones M1–M5)
```
