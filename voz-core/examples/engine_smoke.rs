// SPDX-License-Identifier: Apache-2.0
//! Drives the real Engine (live capture + recorder + job queue + worker threads +
//! events) with a mock transcriber and offline refine, so it's deterministic and
//! needs no model/network. Run:
//!   cargo run -p voz-core --features engine --example engine_smoke

#[cfg(feature = "engine")]
fn main() {
    use std::sync::mpsc::channel;
    use std::sync::Arc;
    use std::time::Duration;
    use voz_core::config::RefineBackend;
    use voz_core::engine::Engine;
    use voz_core::model::{Source, Speaker, Transcript, Turn};
    use voz_core::transcribe::Transcriber;

    struct Mock;
    impl Transcriber for Mock {
        fn transcribe(&self, a: &[f32], s: Speaker) -> voz_core::Result<Transcript> {
            Ok(Transcript {
                turns: vec![Turn {
                    speaker: s,
                    text: format!(
                        "Test note captured {} samples for the engine smoke test",
                        a.len()
                    ),
                    start_ms: 0,
                    end_ms: 0,
                }],
                language: Some("en".into()),
            })
        }
    }

    let dir = std::env::temp_dir().join("voz_engine_demo");
    let _ = std::fs::remove_dir_all(&dir);
    let mut settings = voz_core::Settings::default();
    settings.general.save_dir = dir.to_str().unwrap().to_string();
    settings.general.keep_audio = true;
    settings.refine.backend = RefineBackend::None; // offline, raw-only

    let (tx, rx) = channel();
    let mut engine = Engine::new(settings, Arc::new(Mock), dir.join("history.sqlite"), tx);

    println!("start(Both) ...");
    engine.start(Source::Both).expect("start");
    std::thread::sleep(Duration::from_millis(1000));
    println!("level while recording: {:?}", engine.level());
    println!("stop() -> background job; recorder is now {:?}", {
        engine.stop().expect("stop");
        engine.state()
    });
    std::thread::sleep(Duration::from_millis(800)); // let the worker finish

    println!("--- events ---");
    for ev in rx.try_iter() {
        println!("{ev:?}");
    }
}

#[cfg(not(feature = "engine"))]
fn main() {
    eprintln!("run with --features engine");
}
