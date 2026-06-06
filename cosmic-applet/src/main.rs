// SPDX-License-Identifier: Apache-2.0
//! Voz COSMIC panel applet — **scaffold**.
//!
//! This is the Wayland-native counterpart to the Tauri shell: a true panel-anchored
//! popup on COSMIC, reusing the exact same [`voz_core::engine::Engine`]. It is *not*
//! built or verified yet (the dev environment is GNOME/X11 with no libcosmic). The
//! code below sketches the intended structure; the libcosmic calls are commented
//! against a pinned revision because that crate's API moves between releases.
//!
//! Architecture (why this is small): `voz-core` already owns capture → transcribe →
//! refine → store → history and emits [`voz_core::event::Event`]s over an mpsc
//! channel. A front-end only has to (1) send commands to the Engine and (2) render
//! its event stream. So this applet is a thin view, just like `voz-app`.
//!
//! Build (once a COSMIC environment + a pinned libcosmic are available):
//! ```sh
//! # in cosmic-applet/Cargo.toml: uncomment libcosmic + tokio and pin the rev
//! cargo build --release -p voz-cosmic
//! ```

use std::sync::mpsc::{channel, Receiver};
use std::sync::{Arc, Mutex};

use voz_core::engine::Engine;
use voz_core::event::{Event, TrayState};
use voz_core::history::History;
use voz_core::model::Source;

/// Shared state a libcosmic `Application` impl would hold.
struct VozApplet {
    engine: Arc<Mutex<Engine>>,
    events: Receiver<Event>,
    tray: TrayState,
}

impl VozApplet {
    /// Build the engine exactly as the Tauri shell does. The transcriber is injected
    /// by the host binary (a `WhisperTranscriber`); here we leave that to the caller.
    fn new(transcriber: Arc<dyn voz_core::transcribe::Transcriber>) -> Self {
        let (tx, rx) = channel::<Event>();
        let settings = voz_core::Settings::load();
        let engine = Engine::new(settings, transcriber, History::default_path(), tx);
        Self {
            engine: Arc::new(Mutex::new(engine)),
            events: rx,
            tray: TrayState::Idle,
        }
    }

    /// Command plumbing the applet's buttons would call.
    fn toggle_record(&self) {
        if let Ok(mut e) = self.engine.lock() {
            let recording = matches!(e.state(), voz_core::RecState::Recording);
            let _ = if recording {
                e.stop()
            } else {
                e.start(Source::Both)
            };
        }
    }

    /// Drain engine events to update the panel icon + popup (called from the
    /// applet's subscription/update loop).
    fn pump(&mut self) {
        while let Ok(ev) = self.events.try_recv() {
            if let Event::Tray(ts) = ev {
                self.tray = ts; // -> choose the panel icon (idle/recording/processing)
            }
            // Partial / Saved / JobState events feed the popup view here.
        }
    }
}

fn main() {
    eprintln!(
        "voz-cosmic is a scaffold — see cosmic-applet/README.md. \
         The Engine wiring lives in this file; the libcosmic Application impl is TODO."
    );
    // Real entry point once libcosmic is wired:
    //   cosmic::applet::run::<VozApplet>(())  // with the Application trait implemented
    let _ = VozApplet::new; // keep the type referenced
}
