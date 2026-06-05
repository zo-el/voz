// SPDX-License-Identifier: Apache-2.0
//! Note serialization + safe persistence.
//!
//! Every recording is saved as two linked Obsidian-friendly Markdown notes:
//! a **refined** note (front-matter + structured notes) and a **raw** note
//! (speaker-attributed verbatim transcript, the source of truth). Writes are
//! atomic (temp file + rename) so a crash can't corrupt a vault file.

use crate::model::{NoteMeta, Source, Transcript};
use std::path::{Path, PathBuf};

/// Strip characters that are illegal or awkward in file names (and path
/// separators, to prevent traversal from a model-derived title).
#[must_use]
pub fn sanitize_filename(s: &str) -> String {
    let cleaned: String = s
        .chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' | '\n' | '\r' | '\t' => ' ',
            c if c.is_control() => ' ',
            c => c,
        })
        .collect();
    let trimmed = cleaned.split_whitespace().collect::<Vec<_>>().join(" ");
    let trimmed = trimmed.trim_matches('.').trim();
    if trimmed.is_empty() {
        "Untitled".to_string()
    } else {
        trimmed.chars().take(120).collect()
    }
}

/// Base note name: `YYYY-MM-DD HH-MM <title>` derived from an RFC3339 timestamp
/// (`2026-06-05T14:07:11...`) plus a sanitized title.
#[must_use]
pub fn note_basename(created_rfc3339: &str, title: &str) -> String {
    let date = created_rfc3339.get(0..10).unwrap_or("0000-00-00");
    let hh = created_rfc3339.get(11..13).unwrap_or("00");
    let mm = created_rfc3339.get(14..16).unwrap_or("00");
    format!("{date} {hh}-{mm} {}", sanitize_filename(title))
}

/// Name of the raw note for a given base name.
#[must_use]
pub fn raw_basename(base: &str) -> String {
    format!("{base} (raw)")
}

fn yaml_list(items: &[String]) -> String {
    format!("[{}]", items.join(", "))
}

fn source_str(s: Source) -> &'static str {
    match s {
        Source::Mic => "Mic",
        Source::System => "System",
        Source::Both => "Both",
    }
}

/// Render the refined note (front-matter + body + a `[[wikilink]]` to the raw).
#[must_use]
pub fn refined_note(meta: &NoteMeta, body: &str, raw_link_name: &str) -> String {
    let voices = yaml_list(&meta.voices);
    format!(
        "---\n\
         created: {created}\n\
         duration: {dur}s\n\
         words: {words}\n\
         source: {source}\n\
         voices: {voices}\n\
         model: {model}\n\
         refine: {backend}\n\
         lossless_ok: {lossless}\n\
         raw: \"[[{raw}]]\"\n\
         tags: [voz]\n\
         ---\n\n\
         {body}\n\n\
         > Full transcript: [[{raw}]]\n",
        created = meta.created,
        dur = meta.duration_secs,
        words = meta.words,
        source = source_str(meta.source),
        voices = voices,
        model = meta.model,
        backend = meta.refine_backend,
        lossless = meta.lossless_ok,
        raw = raw_link_name,
        body = body.trim(),
    )
}

/// Render the raw note: speaker-attributed verbatim turns + a back-link.
#[must_use]
pub fn raw_note(created_rfc3339: &str, transcript: &Transcript, refined_link_name: &str) -> String {
    let mut out = format!(
        "---\ncreated: {created}\nrefined: \"[[{refined}]]\"\n---\n\n",
        created = created_rfc3339,
        refined = refined_link_name,
    );
    for turn in &transcript.turns {
        out.push_str(&format!(
            "**{}:** {}\n\n",
            turn.speaker.label(),
            turn.text.trim()
        ));
    }
    out
}

/// Atomically write `contents` to `path` (temp file in the same directory, then
/// rename). Creates parent directories as needed.
///
/// # Errors
/// Propagates any filesystem error.
pub fn write_atomic(path: &Path, contents: &str) -> crate::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp: PathBuf = path.with_extension(format!(
        "{}.tmp",
        path.extension().and_then(|e| e.to_str()).unwrap_or("md")
    ));
    std::fs::write(&tmp, contents)?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Speaker, Turn};

    fn sample_meta() -> NoteMeta {
        NoteMeta {
            created: "2026-06-05T14:07:11".into(),
            duration_secs: 1384,
            source: Source::Both,
            voices: vec!["Me".into(), "Them".into()],
            model: "whisper large-v3-turbo q5_0".into(),
            refine_backend: "Claude Code".into(),
            lossless_ok: true,
            words: 1240,
        }
    }

    #[test]
    fn basename_formats_date_time_and_title() {
        let b = note_basename("2026-06-05T14:07:11Z", "Planning sync");
        assert_eq!(b, "2026-06-05 14-07 Planning sync");
        assert_eq!(raw_basename(&b), "2026-06-05 14-07 Planning sync (raw)");
    }

    #[test]
    fn sanitize_blocks_path_traversal_and_illegal_chars() {
        // The key property: a model-derived title can never become a path —
        // no separators survive, so it stays a single filename component.
        let s = sanitize_filename("../../etc/passwd");
        assert!(!s.contains('/') && !s.contains('\\'));
        let s2 = sanitize_filename("a/b:c*d?e");
        assert!(!s2.contains('/') && !s2.contains(':') && !s2.contains('*') && !s2.contains('?'));
        assert_eq!(sanitize_filename("   "), "Untitled");
    }

    #[test]
    fn refined_note_has_frontmatter_and_link() {
        let md = refined_note(
            &sample_meta(),
            "## Summary\nDid things.",
            "2026-06-05 14-07 Planning sync (raw)",
        );
        assert!(md.starts_with("---\n"));
        assert!(md.contains("source: Both"));
        assert!(md.contains("voices: [Me, Them]"));
        assert!(md.contains("lossless_ok: true"));
        assert!(md.contains("[[2026-06-05 14-07 Planning sync (raw)]]"));
        assert!(md.contains("## Summary"));
    }

    #[test]
    fn raw_note_lists_attributed_turns() {
        let tr = Transcript {
            turns: vec![
                Turn {
                    speaker: Speaker::Me,
                    text: "hello".into(),
                    start_ms: 0,
                    end_ms: 1,
                },
                Turn {
                    speaker: Speaker::Them,
                    text: "hi".into(),
                    start_ms: 1,
                    end_ms: 2,
                },
            ],
            language: None,
        };
        let md = raw_note("2026-06-05T14:07:11", &tr, "2026-06-05 14-07 Planning sync");
        assert!(md.contains("**Me:** hello"));
        assert!(md.contains("**Them:** hi"));
        assert!(md.contains("refined: \"[[2026-06-05 14-07 Planning sync]]\""));
    }

    #[test]
    fn atomic_write_creates_file_and_dirs() {
        let mut dir = std::env::temp_dir();
        dir.push(format!("voz-test-{}", std::process::id()));
        let path = dir.join("sub").join("note.md");
        write_atomic(&path, "hello").unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "hello");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
