// SPDX-License-Identifier: Apache-2.0
//! Voz desktop shell (Tauri). Thin bridge over `voz_core::engine::Engine`:
//! maps typed commands to the engine, forwards the engine's Event stream to the
//! webview, and drives the tray icon state. All logic lives in `voz-core`.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::sync::mpsc::{channel, Receiver};
use std::sync::{Arc, Mutex};
use tauri::image::Image;
use tauri::menu::{Menu, MenuItem};
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

mod logging;

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
fn update_settings(
    settings: serde_json::Value,
    app: tauri::AppHandle,
    state: State<AppState>,
) -> Result<(), String> {
    let parsed: Settings = serde_json::from_value(settings).map_err(err)?;
    parsed.save().map_err(err)?; // persist to config.toml
    let reload = {
        let e = state.engine.lock().map_err(err)?;
        e.settings().transcription.model != parsed.transcription.model
            || e.settings().transcription.accel != parsed.transcription.accel
    };
    state
        .engine
        .lock()
        .map_err(err)?
        .update_settings(parsed.clone());
    if reload {
        // model or acceleration changed -> reload the transcriber off-thread
        std::thread::spawn(move || {
            let t = load_transcriber(&parsed);
            if let Some(st) = app.try_state::<AppState>() {
                if let Ok(mut e) = st.engine.lock() {
                    e.set_transcriber(t);
                }
            }
        });
    }
    Ok(())
}

/// Open a saved note in the user's default app (e.g. Obsidian/editor). The path
/// comes from our own history index, and is passed as argv (no shell).
#[tauri::command]
fn open_path(path: String) -> Result<(), String> {
    std::process::Command::new("xdg-open")
        .arg(&path)
        .spawn()
        .map_err(err)?;
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

fn source_from_str(s: &str) -> Source {
    match s {
        "Mic" => Source::Mic,
        "System" => Source::System,
        _ => Source::Both,
    }
}

fn parse_style(s: &str) -> voz_core::model::RefineStyle {
    use voz_core::model::RefineStyle as RS;
    match s {
        "adaptive" => RS::Adaptive,
        "meeting" => RS::Meeting,
        "memo" => RS::Memo,
        other => RS::Custom(other.to_string()),
    }
}

/// Read a note (raw + refined bodies) for the in-app detail view.
#[tauri::command]
fn read_note(refined_path: String) -> Result<serde_json::Value, String> {
    let h = History::open(&History::default_path()).map_err(err)?;
    let rec = h.get_by_refined(&refined_path).map_err(err)?;
    let refined_md = std::fs::read_to_string(&refined_path).unwrap_or_default();
    let raw_path = rec.as_ref().map(|r| r.raw_path.clone()).unwrap_or_default();
    let raw_md = std::fs::read_to_string(&raw_path).unwrap_or_default();
    Ok(serde_json::json!({
        "title": rec.as_ref().map(|r| r.title.clone()).unwrap_or_default(),
        "backend": rec.as_ref().map(|r| r.refine_backend.clone()).unwrap_or_default(),
        "voices": rec.as_ref().map(|r| r.voices.clone()).unwrap_or_default(),
        "lossless_ok": rec.as_ref().is_none_or(|r| r.lossless_ok),
        "refined": voz_core::store::strip_frontmatter(&refined_md),
        "raw": voz_core::store::strip_frontmatter(&raw_md),
        "raw_path": raw_path,
    }))
}

/// Delete a note (refined + raw files + the history row).
#[tauri::command]
fn delete_note(refined_path: String) -> Result<(), String> {
    let h = History::open(&History::default_path()).map_err(err)?;
    if let Some(rec) = h.get_by_refined(&refined_path).map_err(err)? {
        let _ = std::fs::remove_file(&rec.raw_path);
    }
    let _ = std::fs::remove_file(&refined_path);
    h.delete_by_refined(&refined_path).map_err(err)?;
    Ok(())
}

/// Re-run refine on an existing note with a (possibly different) style, in the
/// background; emits `noteUpdated` when the refined note is rewritten.
#[tauri::command]
fn rerefine(
    refined_path: String,
    style: String,
    app: tauri::AppHandle,
    state: State<AppState>,
) -> Result<(), String> {
    let (refine_cfg, model_label) = {
        let e = state.engine.lock().map_err(err)?;
        (
            e.settings().refine.clone(),
            format!("whisper {}", e.settings().transcription.model),
        )
    };
    let h = History::open(&History::default_path()).map_err(err)?;
    let rec = h
        .get_by_refined(&refined_path)
        .map_err(err)?
        .ok_or("note not found")?;
    let raw_md = std::fs::read_to_string(&rec.raw_path).map_err(err)?;
    let transcript = voz_core::store::parse_raw_note(&raw_md);
    let rstyle = parse_style(&style);
    let raw_link = std::path::Path::new(&rec.raw_path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or_default()
        .to_string();

    std::thread::spawn(move || {
        let refiner = voz_core::refine_backends::build_refiner(&refine_cfg, None);
        let (body, backend, lossless) = match refiner.and_then(|r| {
            r.refine(&transcript, &rstyle)
                .ok()
                .map(|b| (b, r.name().to_string()))
        }) {
            Some((b, name)) => {
                let ok = voz_core::refine::lossless_check(&transcript.plain_text(), &b).ok;
                (b, name, ok)
            }
            None => (transcript.plain_text(), "None".to_string(), true),
        };
        let meta = voz_core::NoteMeta {
            created: rec.created.clone(),
            duration_secs: rec.duration_secs as u64,
            source: source_from_str(&rec.source),
            voices: rec.voices.split(", ").map(String::from).collect(),
            model: model_label,
            refine_backend: backend,
            lossless_ok: lossless,
            words: transcript.word_count(),
        };
        let refined_md = voz_core::store::refined_note(&meta, &body, &raw_link);
        let _ = voz_core::store::write_atomic(std::path::Path::new(&refined_path), &refined_md);
        if let Ok(h) = History::open(&History::default_path()) {
            let _ = h.insert(&rec.title, &meta, &refined_path, &rec.raw_path);
        }
        let _ = app.emit(
            "voz://event",
            serde_json::json!({"type":"noteUpdated","refined_path":refined_path}),
        );
    });
    Ok(())
}

/// List the model registry with installed/size info (for the model picker).
#[tauri::command]
fn list_models() -> serde_json::Value {
    let arr: Vec<serde_json::Value> = voz_core::models::MODELS
        .iter()
        .map(|m| {
            serde_json::json!({
                "id": m.id, "display": m.display, "size_mb": m.size_mb,
                "installed": voz_core::models::is_installed(m.id),
                "pinned": !m.sha256.is_empty(),
            })
        })
        .collect();
    serde_json::Value::Array(arr)
}

/// Download a model in the background (progress via `modelProgress` events).
#[tauri::command]
fn download_model(id: String, app: tauri::AppHandle) -> Result<(), String> {
    let (id_prog, id_done, id_err) = (id.clone(), id.clone(), id.clone());
    let (app_prog, app_done) = (app.clone(), app.clone());
    std::thread::spawn(move || {
        let res = voz_core::models::download(&id, false, move |done, total| {
            let pct = if total > 0 {
                done as f32 / total as f32
            } else {
                0.0
            };
            let _ = app_prog.emit(
                "voz://event",
                serde_json::json!({"type":"modelProgress","id":id_prog,"pct":pct}),
            );
        });
        match res {
            Ok(_) => {
                let _ = app_done.emit(
                    "voz://event",
                    serde_json::json!({"type":"modelProgress","id":id_done,"pct":1.0}),
                );
            }
            Err(e) => {
                let _ = app.emit(
                    "voz://event",
                    serde_json::json!({"type":"jobFailed","error":format!("download {id_err}: {e}")}),
                );
            }
        }
    });
    Ok(())
}

/// Redacted diagnostics text for the "Copy diagnostics" button.
#[tauri::command]
fn get_diagnostics(state: State<AppState>) -> Result<String, String> {
    let e = state.engine.lock().map_err(err)?;
    Ok(logging::diagnostics(e.settings()))
}

/// Check the GitHub Releases feed for a newer version. Best-effort and read-only:
/// it fetches release metadata (JSON) and compares the tag — it never downloads or
/// executes anything. Returns `{available, current, latest, url}`.
#[tauri::command]
fn check_update() -> Result<serde_json::Value, String> {
    const REPO: &str = "zo-el/voz";
    let url = format!("https://api.github.com/repos/{REPO}/releases/latest");
    let resp = match ureq::get(&url)
        .set("User-Agent", "voz-app")
        .timeout(std::time::Duration::from_secs(6))
        .call()
    {
        Ok(r) => r,
        // 404 = no release published yet -> treat as "up to date", not an error.
        Err(ureq::Error::Status(404, _)) => {
            return Ok(serde_json::json!({
                "available": false, "current": env!("CARGO_PKG_VERSION"),
                "latest": "", "url": "",
            }));
        }
        Err(e) => return Err(err(e)),
    };
    let json: serde_json::Value = resp.into_json().map_err(err)?;
    let latest = json.get("tag_name").and_then(|v| v.as_str()).unwrap_or("");
    let html = json.get("html_url").and_then(|v| v.as_str()).unwrap_or("");
    let current = env!("CARGO_PKG_VERSION");
    Ok(serde_json::json!({
        "available": voz_core::update::is_newer(current, latest),
        "current": current,
        "latest": latest,
        "url": html,
    }))
}

/// Open the local log file in the user's default app.
#[tauri::command]
fn open_log() -> Result<(), String> {
    std::process::Command::new("xdg-open")
        .arg(logging::log_path())
        .spawn()
        .map_err(err)?;
    Ok(())
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

fn toggle_panel(app: &tauri::AppHandle) {
    if let Some(win) = app.get_webview_window("panel") {
        let _ = if win.is_visible().unwrap_or(false) {
            win.hide()
        } else {
            win.show().and_then(|()| win.set_focus())
        };
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

/// On a brand-new install (no config yet), pick a refine backend that actually
/// works on this machine so a stranger isn't met with failing AI cleanup: if the
/// default (Claude Code) CLI isn't installed, start in raw-only mode. Persists the
/// choice so it's only decided once.
fn first_run_defaults(mut settings: Settings) -> Settings {
    use voz_core::config::RefineBackend;
    if Settings::config_path().exists() {
        // Returning user: don't show onboarding again.
        if !settings.general.onboarded {
            settings.general.onboarded = true;
            let _ = settings.save();
        }
        return settings;
    }
    // Fresh install: pick a backend that works, and leave `onboarded = false` so the
    // UI shows the welcome flow.
    if settings.refine.backend == RefineBackend::ClaudeCode
        && !voz_core::refine_backends::cli_on_path("claude")
    {
        settings.refine.backend = RefineBackend::None;
    }
    let _ = settings.save();
    settings
}

/// GPU on unless the user forced CPU. (No-op on a CPU-only build.)
fn use_gpu(settings: &Settings) -> bool {
    !matches!(settings.transcription.accel, voz_core::config::Accel::Cpu)
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
    match WhisperTranscriber::load(&path, lang, use_gpu(settings)) {
        Ok(t) => Arc::new(t),
        Err(e) => {
            log::warn!("no whisper model loaded ({e}); transcription will error until a model is installed");
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
    let configured = Settings::load().transcription.model;
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
    match res.and_then(|path| WhisperTranscriber::load(&path, Some("en".into()), true)) {
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
    logging::init();
    let settings = first_run_defaults(Settings::load());
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
        .plugin(tauri_plugin_dialog::init())
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
            get_history,
            open_path,
            read_note,
            delete_note,
            rerefine,
            list_models,
            download_model,
            get_diagnostics,
            open_log,
            check_update
        ])
        .setup(move |app| {
            let handle = app.handle().clone();

            // tray icon. On GNOME the AppIndicator extension renders this and
            // left-click activation is unreliable, so we attach a right-click menu
            // (Show/Hide/Quit) and also toggle on left-click where supported.
            let show = MenuItem::with_id(app, "show", "Show / hide Voz", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "Quit Voz", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show, &quit])?;
            let _tray = TrayIconBuilder::with_id("voz-tray")
                .icon(app.default_window_icon().cloned().unwrap())
                .tooltip("Voz")
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "show" => toggle_panel(app),
                    "quit" => app.exit(0),
                    _ => {}
                })
                .on_tray_icon_event(|tray, _event| toggle_panel(tray.app_handle()))
                .build(app)?;

            // forward engine events to the webview + tray
            std::thread::spawn(move || pump_events(handle, rx));

            // recover any recordings spooled by a previous run that crashed mid-job
            if let Some(state) = app.try_state::<AppState>() {
                if let Ok(mut e) = state.engine.lock() {
                    let n = e.recover();
                    if n > 0 {
                        log::info!("recovered {n} unfinished recording(s)");
                    }
                }
            }

            // register the global record hotkey (best-effort; logs on Wayland).
            if let Err(e) = app.global_shortcut().register("Ctrl+Super+Space") {
                log::warn!(
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
