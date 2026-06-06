# Voz COSMIC applet (scaffold)

A Wayland-native COSMIC panel applet — the "the tray icon *is* the app" vision done
properly on COSMIC, reusing the same `voz-core` engine as the Tauri shell.

## Status: scaffold, not yet built

**This crate is intentionally unbuilt and excluded from the workspace.** It was
authored on GNOME/X11 with no COSMIC session and no `libcosmic`, so it can't be
compiled or verified here. `src/main.rs` contains the real engine wiring (which
*does* type-check against `voz-core`) plus a sketch of where the libcosmic
`Application` impl goes. Honesty over green checkmarks: see `docs/ROADMAP.md` #27.

## Why it's small

`voz-core` owns everything that matters — capture, transcription, refinement,
storage, history — behind an `Engine` that takes commands and emits `Event`s. A
front-end is just a view over that. `voz-app` (Tauri) and this applet are siblings;
porting to COSMIC is "render the event stream with libcosmic widgets," not a rewrite.

## To make it real

1. Add a COSMIC dev environment (Pop!_OS COSMIC or the nightly).
2. In `Cargo.toml`, uncomment `libcosmic` + `tokio` and pin `libcosmic` to a
   known-good `rev`.
3. Implement `cosmic::Application` for `VozApplet`:
   - **icon**: map `TrayState` → idle / recording / processing panel icons.
   - **on click**: open a popup with the record/stop controls + live state.
   - **subscription**: drain the `Engine` event channel (the `pump()` sketch) and
     stream `Partial`/`Saved`/`JobState` into the popup.
4. Add an `org.voz.Voz.cosmic-applet` desktop file so COSMIC lists it.
5. Re-add this crate to the workspace `members` once it builds.
