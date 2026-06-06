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

## Releasing (automated)
Releases are cut by CI — you never hand-build/upload. To ship a new version:

1. Bump `"version"` in `voz-app/src-tauri/tauri.conf.json` (and add a `CHANGELOG.md`
   entry). Optionally bump `[workspace.package] version` in `Cargo.toml` to match.
2. Merge to `main`.
3. `.github/workflows/release.yml` builds the `.deb` + AppImage, runs the core
   tests, and publishes the `v<version>` GitHub Release with `SHA256SUMS`.

Merges that don't change the version are skipped (no wasted builds). Released
bundles are the **CPU** build (CI has no GPU); CUDA/Vulkan are local opt-in builds.

## Running the app
```bash
cargo build --manifest-path voz-app/src-tauri/Cargo.toml   # debug build
DISPLAY=:1 ./voz-app/src-tauri/target/debug/voz-app        # run it
# or, from voz-app/src-tauri/:  cargo tauri dev
```
For the GPU build, add `--features cuda` (NVIDIA) or `--features vulkan`. The CPU
build is the default. (Note: whisper.cpp's CUDA backend may require the CUDA 12
toolkit; this machine has 11.5 — verify the GPU build before relying on it.)

## Packaging (.deb — verified)
```bash
cd voz-app/src-tauri && cargo tauri build --bundles deb
# -> target/release/bundle/deb/Voz_0.1.0_amd64.deb   (~7 MB)
# install:  sudo apt install ./Voz_0.1.0_amd64.deb
```
For an AppImage too: `--bundles deb appimage` (downloads appimagetool on first run).

**Models:** the app loads the configured Whisper model from
`~/.local/share/voz/models/`, falling back to `base.en`. The `.deb` does **not**
yet bundle a model (open decision in `docs/PLAN.md §6`: ship a small default in the
package vs. auto-fetch on first run). Until then, install a model once, e.g.:
`curl -L -o ~/.local/share/voz/models/ggml-base.en.bin \
  https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.en.bin`
