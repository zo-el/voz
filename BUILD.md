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

## GPU build (CUDA / Vulkan)
The default build is **CPU** (runs anywhere). For GPU acceleration, build with a
backend feature — the app's `Acceleration` setting (`Auto`/`CPU`/`Vulkan`/`CUDA`)
then controls `use_gpu` at runtime.

**CUDA (NVIDIA, fastest).** Needs CUDA **12.x** (the toolkit whisper.cpp compiles
against must match the `libcudart` it links). On a system that *also* has the old
distro `nvidia-cuda-toolkit` (11.5) installed, the linker can grab the wrong
`libcudart` and fail with `undefined symbol: cudaGetDeviceProperties_v2` (a CUDA-12
symbol). Force the 12.x libs first:

```bash
export CUDA_PATH=/usr/local/cuda-12.6
export PATH="$CUDA_PATH/bin:$PATH"
export CUDACXX="$CUDA_PATH/bin/nvcc"
export LD_LIBRARY_PATH="$CUDA_PATH/lib64"          # runtime: find libcudart.so.12
export RUSTFLAGS="-L native=$CUDA_PATH/lib64"      # link-time: 12.x before /usr/lib's 11.5
cargo tauri build --bundles deb --features cuda    # or: cargo build --features cuda
```

The resulting `.deb` only runs on machines with an NVIDIA GPU + the CUDA 12 runtime.

**Vulkan (any GPU, portable).** Cross-vendor (NVIDIA/AMD/Intel), CPU fallback; the
best general-purpose GPU build. Needs the Vulkan SDK/headers and `glslc`:

```bash
sudo apt-get install -y libvulkan-dev glslc spirv-tools
cargo tauri build --bundles deb --features vulkan
```

Verify the GPU is actually used: run a transcription and check `nvidia-smi`
(`--query-compute-apps`) shows the process holding GPU memory.

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
3. `.github/workflows/release.yml` runs the core tests and builds **three** backend
   variants, publishing the `v<version>` GitHub Release with `SHA256SUMS`:
   - **CPU** — `Voz_<v>_amd64.deb` + `.AppImage` (runs everywhere; the default)
   - **Vulkan** — `Voz_<v>_amd64-vulkan.deb` + `.AppImage` (portable GPU, CPU fallback)
   - **CUDA** — `Voz_<v>_amd64-cuda.deb` (NVIDIA, fastest; needs the CUDA-12 runtime)

Merges that don't change the version are skipped (no wasted builds). CI has no GPU,
but the CUDA/Vulkan *kernels* compile on the runner (no device needed to build); the
CUDA variant targets sm_75/86/89 (RTX 20/30/40-series) via `CUDAARCHS`. Each variant
still runs only where its runtime is present — CUDA needs CUDA-12 installed.

## Running the app
```bash
cargo build --manifest-path voz-app/src-tauri/Cargo.toml   # debug build
DISPLAY=:1 ./voz-app/src-tauri/target/debug/voz-app        # run it
# or, from voz-app/src-tauri/:  cargo tauri dev
```
For the GPU build, add `--features cuda` (NVIDIA) or `--features vulkan`. The CPU
build is the default. whisper.cpp's CUDA backend needs the CUDA **12.x** toolkit; this
machine has 12.6 at `/usr/local/cuda-12.6` (use the env exports in the GPU-build
section above so the linker picks 12.x over any older distro `libcudart`).

## Packaging (.deb — verified)
```bash
cd voz-app/src-tauri && cargo tauri build --bundles deb
# -> target/release/bundle/deb/Voz_0.2.0_amd64.deb   (~7 MB, CPU build)
# install:  sudo apt install ./Voz_0.2.0_amd64.deb
```
For an AppImage too: `--bundles deb appimage` (downloads appimagetool on first run).

**Models:** the app loads the configured Whisper model from
`~/.local/share/voz/models/`, falling back to `base.en`. The `.deb` does **not**
yet bundle a model (open decision in `docs/PLAN.md §6`: ship a small default in the
package vs. auto-fetch on first run). Until then, install a model once, e.g.:
`curl -L -o ~/.local/share/voz/models/ggml-base.en.bin \
  https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.en.bin`
