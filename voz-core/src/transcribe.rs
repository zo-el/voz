// SPDX-License-Identifier: Apache-2.0
//! The transcription stage. The real backend (whisper.cpp via `whisper-rs`) is
//! added behind the `whisper` feature in a later milestone; this defines the
//! trait the pipeline depends on, so the engine is testable with a mock.

use crate::model::{Speaker, Transcript};

/// Target audio format Whisper expects.
pub const TARGET_SAMPLE_RATE: u32 = 16_000;

/// A speech-to-text backend operating on 16 kHz mono f32 PCM for a single stream.
pub trait Transcriber: Send + Sync {
    /// Transcribe one mono stream, attributing every turn to `speaker`.
    ///
    /// # Errors
    /// Returns [`crate::Error::Transcribe`] on failure.
    fn transcribe(&self, audio: &[f32], speaker: Speaker) -> crate::Result<Transcript>;
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A deterministic transcriber used by pipeline tests.
    struct Echo;
    impl Transcriber for Echo {
        fn transcribe(&self, audio: &[f32], speaker: Speaker) -> crate::Result<Transcript> {
            Ok(Transcript {
                turns: vec![crate::model::Turn {
                    speaker,
                    text: format!("{} samples", audio.len()),
                    start_ms: 0,
                    end_ms: 0,
                }],
                language: Some("en".into()),
            })
        }
    }

    #[test]
    fn trait_is_object_safe_and_usable() {
        let t: Box<dyn Transcriber> = Box::new(Echo);
        let out = t.transcribe(&[0.0; 3], Speaker::Me).unwrap();
        assert_eq!(out.turns[0].text, "3 samples");
        assert_eq!(out.turns[0].speaker, Speaker::Me);
    }
}
