// SPDX-License-Identifier: Apache-2.0
//! Voz desktop shell (Tauri). Thin bridge over `voz_core::engine::Engine`:
//! maps typed commands to the engine, forwards the engine's Event stream to the
//! webview, and drives the tray icon state. All logic lives in `voz-core`.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::sync::mpsc::{channel, Receiver};
use std::sync::{Arc, Mutex};
use tauri::image::Image;
use tauri::tray::TrayIconBuilder;
use tauri::{Emitter, Manager, State};
use tauri_plugin_global_shortcut::GlobalShortcutExt;
use voz_core::engine::Engine;
use voz_core::event::{Event, TrayState};
use voz_core::history::History;
use voz_core::model::Source;
use voz_core::transcribe::Transcriber;
use voz_core::whisper::WhisperTranscriber;
use voz_core::Settings;

struct AppState {
    engine: Mutex<Engine>,
}

fn err<E: std::fmt::Display>(e: E) -> String {
    e.to_string()
}

#[tauri::command]
fn start(source: String, state: State<AppState>) -> Result<(), String> {
    let src = match source.as_str() {
        "mic" => Source::Mic,
        "system" => Source::System,
        _ => Source::Both,
    };
    state.engine.lock().map_err(err)?.start(src).map_err(err)
}

#[tauri::command]
fn stop(state: State<AppState>) -> Result<(), String> {
    state.engine.lock().map_err(err)?.stop().map_err(err)
}

#[tauri::command]
fn pause(state: State<AppState>) -> Result<(), String> {
    state.engine.lock().map_err(err)?.pause().map_err(err)
}

#[tauri::command]
fn resume(state: State<AppState>) -> Result<(), String> {
    state.engine.lock().map_err(err)?.resume().map_err(err)
}

#[tauri::command]
fn cancel(state: State<AppState>) -> Result<(), String> {
    state.engine.lock().map_err(err)?.cancel().map_err(err)
}

#[tauri::command]
fn get_state(state: State<AppState>) -> String {
    format!(
        "{:?}",
        state
            .engine
            .lock()
            .map(|e| e.state())
            .unwrap_or(voz_core::RecState::Idle)
    )
}

#[tauri::command]
fn get_level(state: State<AppState>) -> (f32, f32) {
    let l = state.engine.lock().map(|e| e.level()).unwrap_or_default();
    (l.mic, l.system)
}

#[tauri::command]
fn get_settings(state: State<AppState>) -> Result<serde_json::Value, String> {
    let e = state.engine.lock().map_err(err)?;
    serde_json::to_value(e.settings()).map_err(err)
}

#[tauri::command]
fn update_settings(settings: serde_json::Value, state: State<AppState>) -> Result<(), String> {
    let parsed: Settings = serde_json::from_value(settings).map_err(err)?;
    state.engine.lock().map_err(err)?.update_settings(parsed);
    Ok(())
}

#[tauri::command]
fn get_history() -> Result<serde_json::Value, String> {
    let h = History::open(&History::default_path()).map_err(err)?;
    let rows = h.recent(100).map_err(err)?;
    let arr: Vec<serde_json::Value> = rows
        .iter()
        .map(|r| {
            serde_json::json!({
                "created": r.created, "title": r.title, "source": r.source,
                "voices": r.voices, "words": r.words, "duration": r.duration_secs,
                "backend": r.refine_backend, "lossless_ok": r.lossless_ok,
                "refined_path": r.refined_path,
            })
        })
        .collect();
    Ok(serde_json::Value::Array(arr))
}

/// Map an engine `Event` to a JSON payload for the webview.
fn event_json(ev: &Event) -> serde_json::Value {
    match ev {
        Event::RecState(s) => serde_json::json!({"type":"recState","state":format!("{s:?}")}),
        Event::Level(l) => serde_json::json!({"type":"level","mic":l.mic,"system":l.system}),
        Event::JobState { job, state } => {
            serde_json::json!({"type":"jobState","job":job.0,"state":format!("{state:?}")})
        }
        Event::RawTranscript { job, text } => {
            serde_json::json!({"type":"raw","job":job.0,"text":text.plain_text()})
        }
        Event::RefineToken { job, token } => {
            serde_json::json!({"type":"refineToken","job":job.0,"token":token})
        }
        Event::RefineDone {
            job,
            refined,
            lossless_ok,
        } => {
            serde_json::json!({"type":"refineDone","job":job.0,"refined":refined,"lossless_ok":lossless_ok})
        }
        Event::Saved { job, refined, raw } => serde_json::json!({
            "type":"saved","job":job.0,
            "refined":refined.to_string_lossy(),"raw":raw.to_string_lossy()
        }),
        Event::JobFailed { job, error } => {
            serde_json::json!({"type":"jobFailed","job":job.0,"error":error})
        }
        Event::Tray(ts) => serde_json::json!({"type":"tray","badge":ts.badge()}),
        Event::ModelProgress { id, pct } => {
            serde_json::json!({"type":"modelProgress","id":id,"pct":pct})
        }
        Event::Notify { title, body, job } => {
            serde_json::json!({"type":"notify","title":title,"body":body,"job":job.0})
        }
    }
}

fn tray_icon_for(app: &tauri::AppHandle, ts: TrayState) -> Option<Image<'static>> {
    let name = match ts.badge() {
        Some("rec") => "tray-rec.png",
        Some("proc") => "tray-proc.png",
        _ => "tray-idle.png",
    };
    let path = app.path().resource_dir().ok()?.join("icons").join(name);
    Image::from_path(path).ok()
}

/// Load the transcriber: the configured model if installed, else `base.en`.
fn load_transcriber(settings: &Settings) -> Arc<dyn Transcriber> {
    let id = &settings.transcription.model;
    let path = if voz_core::models::is_installed(id) {
        voz_core::models::model_path(id)
    } else {
        voz_core::models::model_path("base.en")
    };
    let lang = if settings.transcription.language == "auto" {
        None
    } else {
        Some(settings.transcription.language.clone())
    };
    match WhisperTranscriber::load(&path, lang) {
        Ok(t) => Arc::new(t),
        Err(e) => {
            eprintln!("warning: no whisper model loaded ({e}); transcription will error until a model is installed");
            Arc::new(NullTranscriber)
        }
    }
}

/// Fallback used only when no model is installed, so the UI still runs.
struct NullTranscriber;
impl Transcriber for NullTranscriber {
    fn transcribe(
        &self,
        _audio: &[f32],
        _speaker: voz_core::Speaker,
    ) -> voz_core::Result<voz_core::Transcript> {
        Err(voz_core::Error::Transcribe("no model installed".into()))
    }
}

/// Zero-setup model bootstrap: if the configured model and `base.en` are both
/// absent, download `base.en` (pinned + SHA-256 verified), then hot-swap a real
/// transcriber into the engine. Emits `modelProgress` events for the UI.
fn ensure_model(app: tauri::AppHandle) {
    let configured = Settings::default().transcription.model;
    if voz_core::models::is_installed(&configured) || voz_core::models::is_installed("base.en") {
        return; // a model is present; the startup load already used it
    }
    let p = app.clone();
    let emit_pct = move |pct: f32| {
        let _ = p.emit(
            "voz://event",
            serde_json::json!({"type":"modelProgress","id":"base.en","pct":pct}),
        );
    };
    emit_pct(0.0);
    let p2 = app.clone();
    let res = voz_core::models::download("base.en", false, move |done, total| {
        let pct = if total > 0 {
            done as f32 / total as f32
        } else {
            0.0
        };
        let _ = p2.emit(
            "voz://event",
            serde_json::json!({"type":"modelProgress","id":"base.en","pct":pct}),
        );
    });
    match res.and_then(|path| WhisperTranscriber::load(&path, Some("en".into()))) {
        Ok(t) => {
            if let Some(state) = app.try_state::<AppState>() {
                if let Ok(mut e) = state.engine.lock() {
                    e.set_transcriber(Arc::new(t));
                }
            }
            emit_pct(1.0);
        }
        Err(e) => {
            let _ = app.emit(
                "voz://event",
                serde_json::json!({"type":"jobFailed","error":format!("model download: {e}")}),
            );
        }
    }
}

fn pump_events(app: tauri::AppHandle, rx: Receiver<Event>) {
    let tray_id = "voz-tray";
    for ev in rx {
        let _ = app.emit("voz://event", event_json(&ev));
        if let Event::Tray(ts) = &ev {
            if let (Some(tray), Some(img)) = (app.tray_by_id(tray_id), tray_icon_for(&app, *ts)) {
                let _ = tray.set_icon(Some(img));
            }
        }
    }
}

fn main() {
    let settings = Settings::default();
    let transcriber = load_transcriber(&settings);
    let (tx, rx) = channel::<Event>();
    let engine = Engine::new(settings, transcriber, History::default_path(), tx);

    tauri::Builder::default()
        .plugin(tauri_plugin_notification::init())
        .plugin(
            // Global push-to-toggle hotkey. Works on X11 (GNOME); on Wayland/COSMIC
            // the compositor must grant it (GNOME 48 GlobalShortcuts portal), and
            // COSMIC's portal doesn't implement it yet — fall back to a custom
            // Settings shortcut there. See docs/RESEARCH.md §4.
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, _shortcut, event| {
                    if event.state() == tauri_plugin_global_shortcut::ShortcutState::Pressed {
                        if let Some(state) = app.try_state::<AppState>() {
                            if let Ok(mut e) = state.engine.lock() {
                                let _ = if matches!(e.state(), voz_core::RecState::Idle) {
                                    e.start(voz_core::model::Source::Both)
                                } else {
                                    e.stop()
                                };
                            }
                        }
                    }
                })
                .build(),
        )
        .manage(AppState {
            engine: Mutex::new(engine),
        })
        .invoke_handler(tauri::generate_handler![
            start,
            stop,
            pause,
            resume,
            cancel,
            get_state,
            get_level,
            get_settings,
            update_settings,
            get_history
        ])
        .setup(move |app| {
            let handle = app.handle().clone();

            // tray icon; left-click toggles the panel.
            let _tray = TrayIconBuilder::with_id("voz-tray")
                .icon(app.default_window_icon().cloned().unwrap())
                .tooltip("Voz")
                .on_tray_icon_event(|tray, _event| {
                    if let Some(win) = tray.app_handle().get_webview_window("panel") {
                        let _ = if win.is_visible().unwrap_or(false) {
                            win.hide()
                        } else {
                            win.show().and_then(|()| win.set_focus())
                        };
                    }
                })
                .build(app)?;

            // forward engine events to the webview + tray
            std::thread::spawn(move || pump_events(handle, rx));

            // recover any recordings spooled by a previous run that crashed mid-job
            if let Some(state) = app.try_state::<AppState>() {
                if let Ok(mut e) = state.engine.lock() {
                    let n = e.recover();
                    if n > 0 {
                        eprintln!("recovered {n} unfinished recording(s)");
                    }
                }
            }

            // register the global record hotkey (best-effort; logs on Wayland).
            if let Err(e) = app.global_shortcut().register("Ctrl+Super+Space") {
                eprintln!(
                    "global hotkey unavailable ({e}); use the tray icon or a compositor shortcut"
                );
            }

            // zero-setup: if no model is installed, fetch the default in the
            // background and hot-swap it into the engine (with progress events).
            let dl_handle = app.handle().clone();
            std::thread::spawn(move || ensure_model(dl_handle));

            if let Some(win) = app.get_webview_window("panel") {
                let _ = win.show();
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running Voz");
}
