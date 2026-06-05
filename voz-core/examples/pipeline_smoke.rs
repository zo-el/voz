// SPDX-License-Identifier: Apache-2.0
//! Full vertical slice: audio -> transcribe -> refine -> save 2 linked notes ->
//! index in history. Run:
//!   cargo run -p voz-core --features "whisper audio refine history" \
//!     --example pipeline_smoke
//! Uses /tmp/jfk.wav + base.en.

#[cfg(all(
    feature = "whisper",
    feature = "audio",
    feature = "refine",
    feature = "history"
))]
fn main() {
    use std::path::Path;
    use voz_core::capture::{read_wav_16k_mono, write_wav_16k_mono};
    use voz_core::history::History;
    use voz_core::model::{RefineStyle, Source};
    use voz_core::pipeline::{process, CapturedAudio, ProcessRequest};
    use voz_core::refine_backends::CliRefiner;
    use voz_core::store::{audio_path, save_notes};
    use voz_core::whisper::WhisperTranscriber;

    let samples = read_wav_16k_mono(Path::new("/tmp/jfk.wav")).expect("read wav");
    let transcriber =
        WhisperTranscriber::load(&voz_core::models::model_path("base.en"), Some("en".into()))
            .expect("load model");
    let refiner = CliRefiner::claude_code();

    let req = ProcessRequest {
        created_rfc3339: "2026-06-05T14:07:11".into(),
        title: "JFK inaugural".into(),
        source: Source::Mic,
        model_label: "whisper base.en".into(),
        duration_secs: 11,
        style: RefineStyle::Adaptive,
    };
    let audio = CapturedAudio {
        mic: Some(samples.clone()),
        system: None,
    };

    let out = process(&req, &audio, &transcriber, Some(&refiner)).expect("process");

    let dir = std::env::temp_dir().join("voz_pipeline_demo");
    let dir_s = dir.to_str().unwrap();
    let paths = save_notes(
        dir_s,
        &out.base,
        &out.raw_base,
        &out.refined_md,
        &out.raw_md,
    )
    .expect("save notes");
    write_wav_16k_mono(&audio_path(dir_s, &out.base), &samples).expect("save wav");

    let hist = History::open(&dir.join("history.sqlite")).expect("open history");
    hist.insert(
        &req.title,
        &out.meta,
        paths.refined.to_str().unwrap(),
        paths.raw.to_str().unwrap(),
    )
    .expect("index");

    println!("== saved ==");
    println!("refined: {}", paths.refined.display());
    println!("raw:     {}", paths.raw.display());
    println!("lossless_ok={} words={}", out.lossless_ok, out.meta.words);
    println!("\n== history.recent(5) ==");
    for r in hist.recent(5).expect("recent") {
        println!(
            "- {} | {} | {} | {}w | {}",
            r.created, r.title, r.source, r.words, r.refine_backend
        );
    }
    println!("\n== refined note ==\n{}", out.refined_md);
}

#[cfg(not(all(
    feature = "whisper",
    feature = "audio",
    feature = "refine",
    feature = "history"
)))]
fn main() {
    eprintln!("run with --features \"whisper audio refine history\"");
}
