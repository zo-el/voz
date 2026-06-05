// SPDX-License-Identifier: Apache-2.0
//! Voz desktop shell (Tauri). Thin: it maps typed Tauri commands to
//! `voz_core::Command`, forwards `voz_core::Event` to the webview, drives the
//! tray icon state, and shows/hides the frameless panel. All logic is in
//! `voz-core` (see `docs/ARCHITECTURE.md §2`).
//!
//! NOTE (milestone status): this is the M0 scaffold. The engine bridge, tray
//! state wiring, global hotkey, and panel positioning are implemented across
//! M1–M5. It is excluded from the workspace build until system deps are present
//! (see `../../BUILD.md`); the type-checked logic lives in `voz-core`.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use voz_core::{Command, Event};

/// Tauri command: the webview asks the engine to start recording.
/// The webview can only invoke these typed commands — no shell/fs/http is exposed.
#[tauri::command]
fn start(source: String) -> Result<(), String> {
    let _cmd = match source.as_str() {
        "mic" => Command::Start { source: voz_core::Source::Mic },
        "system" => Command::Start { source: voz_core::Source::System },
        _ => Command::Start { source: voz_core::Source::Both },
    };
    // M1: forward `_cmd` to the running engine handle.
    Ok(())
}

#[tauri::command]
fn stop() -> Result<(), String> {
    // M1: engine.send(Command::Stop) — enqueues a background job, frees recorder.
    Ok(())
}

/// Forward an engine event to the panel webview as a typed payload.
fn _emit_event(_app: &tauri::AppHandle, _ev: &Event) {
    // M5: app.emit_to("panel", "voz://event", payload) and update the tray icon
    //     from Event::Tray, raise a notification on Event::Notify.
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_notification::init())
        .invoke_handler(tauri::generate_handler![start, stop])
        .setup(|_app| {
            // M5: build the tray icon (idle/recording/processing variants), start
            //     the voz-core engine, and pump its Event stream into _emit_event.
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running Voz");
}
