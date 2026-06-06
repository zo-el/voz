// SPDX-License-Identifier: Apache-2.0
//! whisper.cpp transcription backend (feature `whisper`), implementing
//! [`crate::transcribe::Transcriber`]. Acceleration (CPU / Vulkan / CUDA) is
//! selected at build time via the crate's `vulkan`/`cuda` features.

use crate::model::{Speaker, Transcript, Turn};
use crate::transcribe::Transcriber;
use std::path::Path;
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

/// A loaded whisper.cpp model.
pub struct WhisperTranscriber {
    ctx: WhisperContext,
    language: Option<String>,
}

impl std::fmt::Debug for WhisperTranscriber {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WhisperTranscriber")
            .field("language", &self.language)
            .finish()
    }
}

impl WhisperTranscriber {
    /// Load a GGML model from disk. `language` of `None`/`"auto"` lets Whisper
    /// detect; otherwise it's forced (e.g. `"en"`). `use_gpu` enables the GPU
    /// backend *if this binary was built with one* (Vulkan/CUDA), else it's a no-op
    /// and runs on CPU.
    ///
    /// # Errors
    /// Returns [`crate::Error::Transcribe`] if the model can't be loaded.
    pub fn load(model_path: &Path, language: Option<String>, use_gpu: bool) -> crate::Result<Self> {
        // Route whisper.cpp/ggml's chatty logs into the `log` crate (dropped
        // unless the app installs a subscriber) instead of stderr.
        whisper_rs::install_logging_hooks();
        let path = model_path
            .to_str()
            .ok_or_else(|| crate::Error::Transcribe("non-utf8 model path".into()))?;
        let mut cparams = WhisperContextParameters::default();
        cparams.use_gpu(use_gpu);
        let ctx = WhisperContext::new_with_params(path, cparams)
            .map_err(|e| crate::Error::Transcribe(format!("load model: {e}")))?;
        Ok(Self { ctx, language })
    }
}

impl Transcriber for WhisperTranscriber {
    fn transcribe(&self, audio: &[f32], speaker: Speaker) -> crate::Result<Transcript> {
        let mut state = self
            .ctx
            .create_state()
            .map_err(|e| crate::Error::Transcribe(e.to_string()))?;

        let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
        params.set_print_progress(false);
        params.set_print_special(false);
        params.set_print_realtime(false);
        params.set_print_timestamps(false);
        params.set_debug_mode(false);
        params.set_translate(false);
        if let Some(lang) = self.language.as_deref() {
            if lang != "auto" {
                params.set_language(Some(lang));
            }
        }

        state
            .full(params, audio)
            .map_err(|e| crate::Error::Transcribe(e.to_string()))?;

        let mut turns = Vec::new();
        for segment in state.as_iter() {
            let text = segment
                .to_str_lossy()
                .map_err(|e| crate::Error::Transcribe(e.to_string()))?;
            let trimmed = text.trim();
            if trimmed.is_empty() {
                continue;
            }
            // whisper timestamps are in centiseconds (10 ms units).
            let start_ms = segment.start_timestamp().max(0) as u64 * 10;
            let end_ms = segment.end_timestamp().max(0) as u64 * 10;
            turns.push(Turn {
                speaker,
                text: trimmed.to_string(),
                start_ms,
                end_ms,
            });
        }
        Ok(Transcript {
            turns,
            language: self.language.clone(),
        })
    }
}
