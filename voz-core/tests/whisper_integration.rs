// SPDX-License-Identifier: Apache-2.0
//! Real-whisper integration test (slow lane). Ignored by default; run with:
//!   cargo test -p voz-core --features "whisper audio" -- --ignored
//! Requires the `base.en` model in the cache and a speech sample. The CI slow
//! lane downloads both first (see docs/TESTING.md §4).

#![cfg(all(feature = "whisper", feature = "audio"))]

use std::path::Path;
use voz_core::capture::read_wav_16k_mono;
use voz_core::model::Speaker;
use voz_core::transcribe::Transcriber;
use voz_core::whisper::WhisperTranscriber;

#[test]
#[ignore = "needs base.en model + sample; run in the slow lane"]
fn transcribes_known_speech() {
    let model = voz_core::models::model_path("base.en");
    let sample = Path::new("/tmp/jfk.wav");
    if !model.is_file() || !sample.is_file() {
        eprintln!("skipping: model or sample not present");
        return;
    }
    let audio = read_wav_16k_mono(sample).expect("read sample");
    let t = WhisperTranscriber::load(&model, Some("en".into())).expect("load model");
    let transcript = t.transcribe(&audio, Speaker::Me).expect("transcribe");
    let text = transcript.plain_text().to_lowercase();
    assert!(text.contains("country"), "unexpected transcript: {text}");
    assert_eq!(transcript.turns[0].speaker, Speaker::Me);
}
