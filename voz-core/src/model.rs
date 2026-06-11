// SPDX-License-Identifier: Apache-2.0
//! Core domain types shared across the engine.

use serde::{Deserialize, Serialize};

/// What a recording captures.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Source {
    /// Microphone only.
    Mic,
    /// System output monitor (loopback) only.
    System,
    /// Mic + system monitor (the default; local meeting capture).
    Both,
}

impl Source {
    /// Whether this source includes the microphone stream ("Me").
    #[must_use]
    pub fn has_mic(self) -> bool {
        matches!(self, Source::Mic | Source::Both)
    }
    /// Whether this source includes the system monitor stream ("Them").
    #[must_use]
    pub fn has_system(self) -> bool {
        matches!(self, Source::System | Source::Both)
    }
}

/// Who spoke a turn. Derived for free from which stream carried the audio
/// (mic = Me, monitor = Them) — no ML diarization required.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Speaker {
    Me,
    Them,
    Unknown,
}

impl Speaker {
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Speaker::Me => "Me",
            Speaker::Them => "Them",
            Speaker::Unknown => "Speaker",
        }
    }
}

/// One attributed utterance.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Turn {
    pub speaker: Speaker,
    pub text: String,
    /// Start offset from the beginning of the recording, milliseconds.
    pub start_ms: u64,
    pub end_ms: u64,
}

/// A finalized, speaker-attributed transcript — the raw source of truth.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Transcript {
    pub turns: Vec<Turn>,
    pub language: Option<String>,
}

impl Transcript {
    /// Flatten to plain text (one line per turn, no speaker tags).
    #[must_use]
    pub fn plain_text(&self) -> String {
        self.turns
            .iter()
            .map(|t| t.text.trim())
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Approximate word count across all turns.
    #[must_use]
    pub fn word_count(&self) -> usize {
        self.turns
            .iter()
            .map(|t| t.text.split_whitespace().count())
            .sum()
    }

    /// Distinct speakers present, in first-seen order (for note front-matter).
    #[must_use]
    pub fn voices(&self) -> Vec<&'static str> {
        let mut seen = Vec::new();
        for t in &self.turns {
            let l = t.speaker.label();
            if !seen.contains(&l) {
                seen.push(l);
            }
        }
        seen
    }
}

/// How the refined note is shaped. `Adaptive` lets the model pick meeting-style
/// vs. memo-style based on the content; the others pin a shape; `Custom` is a
/// user-supplied prompt.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RefineStyle {
    #[default]
    Adaptive,
    Meeting,
    Memo,
    Custom(String),
}

impl RefineStyle {
    /// The instruction sent to the refine backend. Always "reorganize, never lose
    /// information" — the raw transcript remains the source of truth.
    #[must_use]
    pub fn prompt(&self) -> String {
        let shape = match self {
            RefineStyle::Adaptive => {
                "Choose the structure that fits the content: for a meeting or \
                 multi-speaker conversation, produce a short Summary, then \
                 Decisions and Action items (with the responsible person when \
                 stated); for a single-speaker memo, produce clean, detailed \
                 notes or bullets without forcing those headings."
            }
            RefineStyle::Meeting => {
                "Produce a short Summary, then Decisions and Action items (with the \
                 responsible person when stated)."
            }
            RefineStyle::Memo => "Produce clean, detailed notes or bullets.",
            RefineStyle::Custom(s) => return s.clone(),
        };
        format!(
            "You are turning a verbatim transcript into a clean, well-structured \
             note. Begin your reply with a single line in the exact form \
             `Title: <a concise 4-8 word title for this note>` — plain text, no \
             markdown or quotes — then a blank line, then the note. {shape} \
             Preserve every concrete fact, number, name, date, and commitment — \
             when in doubt, keep it. Do not invent anything not in the \
             transcript. Organize and condense wording, but never drop \
             information; the verbatim transcript is kept separately as the \
             source of truth."
        )
    }
}

/// Metadata persisted in a refined note's YAML front-matter and the history index.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NoteMeta {
    /// RFC3339 creation timestamp (supplied by the caller).
    pub created: String,
    pub duration_secs: u64,
    pub source: Source,
    pub voices: Vec<String>,
    pub model: String,
    pub refine_backend: String,
    pub lossless_ok: bool,
    pub words: usize,
    /// Concise human title for the note (refiner-generated, or derived from the
    /// transcript when refine is unavailable). Drives the note's filename + header.
    #[serde(default)]
    pub title: String,
    /// Kind label shown in the note header: `Meeting`, `Memo`, or `Note`.
    #[serde(default)]
    pub kind: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t(sp: Speaker, text: &str) -> Turn {
        Turn {
            speaker: sp,
            text: text.into(),
            start_ms: 0,
            end_ms: 0,
        }
    }

    #[test]
    fn source_membership() {
        assert!(Source::Both.has_mic() && Source::Both.has_system());
        assert!(Source::Mic.has_mic() && !Source::Mic.has_system());
        assert!(!Source::System.has_mic() && Source::System.has_system());
    }

    #[test]
    fn transcript_counts_and_voices() {
        let tr = Transcript {
            turns: vec![t(Speaker::Me, "hello there friend"), t(Speaker::Them, "hi")],
            language: Some("en".into()),
        };
        assert_eq!(tr.word_count(), 4);
        assert_eq!(tr.voices(), vec!["Me", "Them"]);
        assert_eq!(tr.plain_text(), "hello there friend\nhi");
    }

    #[test]
    fn adaptive_prompt_is_lossless() {
        let p = RefineStyle::Adaptive.prompt();
        assert!(p.contains("never drop"));
        assert!(p.contains("source of truth"));
    }

    #[test]
    fn custom_style_passes_through() {
        let p = RefineStyle::Custom("just bullet points".into()).prompt();
        assert_eq!(p, "just bullet points");
    }

    // The Settings UI persists RefineStyle as JSON across the Tauri bridge; pin the
    // shape the front-end relies on (unit variants = bare strings; Custom = object).
    #[cfg(feature = "refine")]
    #[test]
    fn refine_style_json_round_trip() {
        let custom = RefineStyle::Custom("bullets only".into());
        let json = serde_json::to_string(&custom).unwrap();
        assert_eq!(json, r#"{"custom":"bullets only"}"#);
        assert_eq!(serde_json::from_str::<RefineStyle>(&json).unwrap(), custom);
        assert_eq!(
            serde_json::to_string(&RefineStyle::Meeting).unwrap(),
            r#""meeting""#
        );
        assert_eq!(
            serde_json::from_str::<RefineStyle>(r#""adaptive""#).unwrap(),
            RefineStyle::Adaptive
        );
    }
}
