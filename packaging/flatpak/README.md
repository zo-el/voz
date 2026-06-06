# Flatpak packaging

Files here build Voz as a Flatpak (`org.voz.Voz`):

- `org.voz.Voz.yml` — the build manifest
- `org.voz.Voz.desktop` — the desktop entry (validated with `desktop-file-validate`)
- `org.voz.Voz.metainfo.xml` — AppStream metadata (validated with `appstreamcli validate --no-net`)

## Local build

```bash
flatpak install -y flathub org.gnome.Platform//47 org.gnome.Sdk//47 \
    org.freedesktop.Sdk.Extension.rust-stable//24.08 \
    org.freedesktop.Sdk.Extension.llvm18//24.08
flatpak-builder --user --install --force-clean build-dir \
    packaging/flatpak/org.voz.Voz.yml
flatpak run org.voz.Voz
```

## Status / what's verified

- ✅ `metainfo.xml` validates (`appstreamcli validate --no-net`) and the `.desktop`
  passes `desktop-file-validate`.
- ⬜ **Not yet built end-to-end.** The dev box this was authored on has `flatpak`
  but not `flatpak-builder` or the runtimes, so the manifest hasn't been run. Two
  real issues to resolve before this installs cleanly (tracked in `docs/ROADMAP.md`
  #5/#29):

  1. **PipeWire capture.** Voz shells out to `pw-record`, which is **not** in the
     GNOME runtime. Either add a module that builds/bundles the PipeWire tools, or
     (better, long-term) move capture to `libpipewire` / the audio portal so the
     sandbox stays clean. `--filesystem=xdg-run/pipewire-0` exposes the socket but
     not the binary.

  2. **Offline cargo sources (Flathub).** Flathub builds have no network. Generate
     `cargo-sources.json` with
     [`flatpak-cargo-generator`](https://github.com/flatpak/flatpak-builder-tools),
     add it to the `voz` module's `sources:`, and append `--offline` to the
     `cargo build` command. The committed manifest targets the **networked local**
     build for now.

- The released `.deb` / AppImage (see `docs/BUILD.md`) are the supported install
  paths today; Flatpak/Flathub is the next packaging target.
