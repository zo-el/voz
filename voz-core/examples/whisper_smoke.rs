// SPDX-License-Identifier: Apache-2.0
//! Manual smoke test for local transcription. Run with:
//!   cargo run -p voz-core --features "whisper audio" --example whisper_smoke -- [wav] [model_id]
//! Defaults: /tmp/jfk.wav with the `base.en` model from the cache.

#[cfg(all(feature = "whisper", feature = "audio"))]
fn main() {
    use std::path::Path;
    use voz_core::capture::read_wav_16k_mono;
    use voz_core::model::Speaker;
    use voz_core::transcribe::Transcriber;
    use voz_core::whisper::WhisperTranscriber;

    let wav = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "/tmp/jfk.wav".into());
    let model_id = std::env::args().nth(2).unwrap_or_else(|| "base.en".into());
    let model_path = voz_core::models::model_path(&model_id);
    println!("model: {}", model_path.display());

    let samples = read_wav_16k_mono(Path::new(&wav)).expect("read wav");
    println!(
        "loaded {} samples ({:.1}s)",
        samples.len(),
        samples.len() as f32 / 16000.0
    );

    let t = WhisperTranscriber::load(&model_path, Some("en".into()), true).expect("load model");
    let transcript = t.transcribe(&samples, Speaker::Me).expect("transcribe");
    for turn in &transcript.turns {
        println!(
            "[{:>6}ms] {}: {}",
            turn.start_ms,
            turn.speaker.label(),
            turn.text
        );
    }
    println!("words: {}", transcript.word_count());
}

#[cfg(not(all(feature = "whisper", feature = "audio")))]
fn main() {
    eprintln!("run with --features \"whisper audio\"");
}
