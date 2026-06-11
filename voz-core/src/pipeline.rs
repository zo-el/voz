// SPDX-License-Identifier: Apache-2.0
//! Orchestration: capture → transcribe → attribute (Me/Them) → refine → notes.
//!
//! Pure and generic over the [`Transcriber`]/[`Refiner`] traits so it is fully
//! testable with mocks. The raw transcript is always produced (and, in the app,
//! persisted) **before** refine runs, and survives a refine failure — the
//! source-of-truth guarantee from `docs/ARCHITECTURE.md`.

use crate::model::{NoteMeta, RefineStyle, Source, Speaker, Transcript};
use crate::refine::{build_input, interpret_refined, lossless_check, Refiner};
use crate::store::{note_basename, raw_basename, raw_note, refined_note};
use crate::transcribe::Transcriber;

/// Per-source captured audio (16 kHz mono f32), already resampled.
#[derive(Debug, Default, Clone)]
pub struct CapturedAudio {
    pub mic: Option<Vec<f32>>,
    pub system: Option<Vec<f32>>,
}

/// Inputs that describe the recording being processed.
#[derive(Debug, Clone)]
pub struct ProcessRequest {
    pub created_rfc3339: String,
    pub title: String,
    pub source: Source,
    pub model_label: String,
    pub duration_secs: u64,
    pub style: RefineStyle,
}

/// Everything produced for one note.
#[derive(Debug, Clone)]
pub struct ProcessOutput {
    pub transcript: Transcript,
    pub refined_body: String,
    pub lossless_ok: bool,
    pub refine_error: Option<String>,
    pub meta: NoteMeta,
    pub base: String,
    pub raw_base: String,
    pub refined_md: String,
    pub raw_md: String,
}

/// Transcribe each captured stream and merge into one Me/Them-attributed,
/// time-ordered transcript.
///
/// # Errors
/// Propagates transcriber errors.
pub fn transcribe_and_attribute(
    audio: &CapturedAudio,
    transcriber: &dyn Transcriber,
) -> crate::Result<Transcript> {
    let mut turns = Vec::new();
    let mut language = None;
    if let Some(mic) = &audio.mic {
        let t = transcriber.transcribe(mic, Speaker::Me)?;
        language = language.or(t.language);
        turns.extend(t.turns);
    }
    if let Some(sys) = &audio.system {
        let t = transcriber.transcribe(sys, Speaker::Them)?;
        language = language.or(t.language);
        turns.extend(t.turns);
    }
    // Stable sort by start time keeps insertion order (Me before Them) on ties.
    turns.sort_by_key(|t| t.start_ms);
    Ok(Transcript { turns, language })
}

/// Run the full pipeline. A refine failure does **not** fail the call — the raw
/// transcript/note is always returned, with `refine_error` set.
///
/// # Errors
/// Returns an error only if transcription itself fails (no usable raw note).
pub fn process(
    req: &ProcessRequest,
    audio: &CapturedAudio,
    transcriber: &dyn Transcriber,
    refiner: Option<&dyn Refiner>,
) -> crate::Result<ProcessOutput> {
    // 1) Raw transcript — the source of truth — is built first.
    let transcript = transcribe_and_attribute(audio, transcriber)?;

    // 2) Refine (optional, fault-tolerant).
    let mut refined_body = String::new();
    let mut lossless_ok = true;
    let mut refine_error = None;
    let backend_name = match refiner {
        Some(r) => {
            match r.refine(&transcript, &req.style) {
                Ok(body) => {
                    lossless_ok = lossless_check(&transcript.plain_text(), &body).ok;
                    refined_body = body;
                }
                Err(e) => {
                    lossless_ok = false;
                    refine_error = Some(e.to_string());
                }
            }
            r.name().to_string()
        }
        None => "None".to_string(),
    };

    // 3) Lift the refiner's `Title:` line + resolve the kind, then derive the name.
    let refined = interpret_refined(&refined_body, &req.style, &req.title);
    let base = note_basename(&req.created_rfc3339, &refined.kind, &refined.title);
    let raw_base = raw_basename(&base);
    let raw_md = raw_note(&req.created_rfc3339, &transcript, &base);

    // 4) Build the refined note (falls back to the raw text if refine produced nothing).
    let body_for_note = if refined.present {
        refined.body.clone()
    } else {
        transcript.plain_text()
    };
    let meta = NoteMeta {
        created: req.created_rfc3339.clone(),
        duration_secs: req.duration_secs,
        source: req.source,
        voices: transcript.voices().into_iter().map(String::from).collect(),
        model: req.model_label.clone(),
        refine_backend: backend_name,
        lossless_ok,
        words: transcript.word_count(),
        title: refined.title.clone(),
        kind: refined.kind.clone(),
    };
    let refined_md = refined_note(&meta, &body_for_note, &raw_base);

    Ok(ProcessOutput {
        transcript,
        refined_body,
        lossless_ok,
        refine_error,
        meta,
        base,
        raw_base,
        refined_md,
        raw_md,
    })
}

/// Helper to build the exact backend input (used by real backends; re-exported for
/// tests that assert the transcript is delimited as data, never shell-interpolated).
#[must_use]
pub fn refiner_input(t: &Transcript, style: &RefineStyle) -> String {
    build_input(t, style)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Turn;

    struct SpeakerEcho;
    impl Transcriber for SpeakerEcho {
        fn transcribe(&self, _audio: &[f32], speaker: Speaker) -> crate::Result<Transcript> {
            let text = match speaker {
                Speaker::Me => "ship 3 features by Friday",
                _ => "sounds good Alex",
            };
            Ok(Transcript {
                turns: vec![Turn {
                    speaker,
                    text: text.into(),
                    start_ms: 0,
                    end_ms: 10,
                }],
                language: Some("en".into()),
            })
        }
    }

    struct GoodRefiner;
    impl Refiner for GoodRefiner {
        fn name(&self) -> &str {
            "Mock"
        }
        fn refine(&self, raw: &Transcript, _s: &RefineStyle) -> crate::Result<String> {
            // Faithful: echoes the facts (3, Friday, Alex) so the guard passes.
            Ok(format!("## Summary\n{}", raw.plain_text()))
        }
    }

    struct FailingRefiner;
    impl Refiner for FailingRefiner {
        fn name(&self) -> &str {
            "Boom"
        }
        fn refine(&self, _r: &Transcript, _s: &RefineStyle) -> crate::Result<String> {
            Err(crate::Error::Refine("backend offline".into()))
        }
    }

    fn req() -> ProcessRequest {
        ProcessRequest {
            created_rfc3339: "2026-06-05T14:07:11".into(),
            title: "Planning sync".into(),
            source: Source::Both,
            model_label: "whisper turbo".into(),
            duration_secs: 30,
            style: RefineStyle::Adaptive,
        }
    }

    fn both_audio() -> CapturedAudio {
        CapturedAudio {
            mic: Some(vec![0.0; 4]),
            system: Some(vec![0.0; 4]),
        }
    }

    #[test]
    fn attribution_orders_me_then_them() {
        let t = transcribe_and_attribute(&both_audio(), &SpeakerEcho).unwrap();
        assert_eq!(t.turns.len(), 2);
        assert_eq!(t.turns[0].speaker, Speaker::Me);
        assert_eq!(t.turns[1].speaker, Speaker::Them);
        assert_eq!(t.voices(), vec!["Me", "Them"]);
    }

    #[test]
    fn full_pipeline_produces_both_notes_and_passes_guard() {
        let out = process(&req(), &both_audio(), &SpeakerEcho, Some(&GoodRefiner)).unwrap();
        assert!(out.lossless_ok, "guard tripped unexpectedly");
        assert!(out.refine_error.is_none());
        assert_eq!(out.base, "Fri 06-05 Note Planning sync");
        assert!(out.raw_md.contains("**Me:** ship 3 features by Friday"));
        assert!(out.refined_md.contains("## Summary"));
        assert!(out.refined_md.contains("# Fri 06-05: Note: Planning sync"));
        assert!(out.refined_md.contains("refine: Mock"));
        assert!(out
            .refined_md
            .contains("[[Fri 06-05 Note Planning sync (raw)]]"));
    }

    #[test]
    fn refine_failure_preserves_raw_note() {
        // The source-of-truth guarantee: a broken backend never loses the transcript.
        let out = process(&req(), &both_audio(), &SpeakerEcho, Some(&FailingRefiner)).unwrap();
        assert_eq!(
            out.refine_error.as_deref(),
            Some("refine backend error: backend offline")
        );
        assert!(!out.lossless_ok);
        assert!(out.refined_body.is_empty());
        assert!(out.raw_md.contains("ship 3 features by Friday")); // raw intact
                                                                   // The refined note still exists, falling back to the raw text as its body.
        assert!(out.refined_md.contains("ship 3 features by Friday"));
    }

    #[test]
    fn mic_only_skips_them() {
        let audio = CapturedAudio {
            mic: Some(vec![0.0; 2]),
            system: None,
        };
        let out = process(&req(), &audio, &SpeakerEcho, Some(&GoodRefiner)).unwrap();
        assert_eq!(out.transcript.voices(), vec!["Me"]);
    }
}
