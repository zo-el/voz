// SPDX-License-Identifier: Apache-2.0
//! Note serialization + safe persistence.
//!
//! Every recording is saved as two linked Obsidian-friendly Markdown notes:
//! a **refined** note (front-matter + structured notes) and a **raw** note
//! (speaker-attributed verbatim transcript, the source of truth). Writes are
//! atomic (temp file + rename) so a crash can't corrupt a vault file.

use crate::model::{NoteMeta, Source, Speaker, Transcript, Turn};
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

fn ms_to_srt_ts(ms: u64) -> String {
    let (h, m, s, milli) = (
        ms / 3_600_000,
        (ms % 3_600_000) / 60_000,
        (ms % 60_000) / 1000,
        ms % 1000,
    );
    format!("{h:02}:{m:02}:{s:02},{milli:03}")
}

/// Render a [`Transcript`] as SubRip (`.srt`) subtitles using each turn's
/// timestamps. Only meaningful when the turns carry real offsets (a freshly
/// transcribed clip — saved notes don't persist per-turn timing).
#[must_use]
pub fn transcript_to_srt(t: &Transcript) -> String {
    let mut out = String::new();
    let mut idx = 0;
    for turn in &t.turns {
        let text = turn.text.trim();
        if text.is_empty() {
            continue;
        }
        idx += 1;
        out.push_str(&format!(
            "{idx}\n{} --> {}\n{text}\n\n",
            ms_to_srt_ts(turn.start_ms),
            ms_to_srt_ts(turn.end_ms),
        ));
    }
    out
}

/// Strip a leading YAML front-matter block (`---` … `---`) and return the body.
#[must_use]
pub fn strip_frontmatter(md: &str) -> &str {
    if let Some(rest) = md.strip_prefix("---\n") {
        if let Some(end) = rest.find("\n---\n") {
            return rest[end + 5..].trim_start_matches('\n');
        }
        if let Some(end) = rest.find("\n---") {
            return rest[end + 4..].trim_start_matches('\n');
        }
    }
    md
}

/// Parse a saved raw note back into a [`Transcript`] (for re-refine / display).
/// Lines like `**Me:** text` become attributed turns; anything else is a turn with
/// an unknown speaker.
#[must_use]
pub fn parse_raw_note(md: &str) -> Transcript {
    let body = strip_frontmatter(md);
    let mut turns = Vec::new();
    for line in body.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with("> ") {
            continue;
        }
        if let Some((label, text)) = line.strip_prefix("**").and_then(|r| r.split_once(":**")) {
            let speaker = match label {
                "Me" => Speaker::Me,
                "Them" => Speaker::Them,
                _ => Speaker::Unknown,
            };
            turns.push(Turn {
                speaker,
                text: text.trim().to_string(),
                start_ms: 0,
                end_ms: 0,
            });
        } else {
            turns.push(Turn {
                speaker: Speaker::Unknown,
                text: line.to_string(),
                start_ms: 0,
                end_ms: 0,
            });
        }
    }
    Transcript {
        turns,
        language: None,
    }
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

/// Expand a leading `~`/`~/` to `$HOME` so a configured save dir like
/// `~/Obsidian/Vault/Voz` resolves correctly.
#[must_use]
pub fn expand_tilde(p: &str) -> PathBuf {
    if p == "~" {
        if let Some(home) = std::env::var_os("HOME") {
            return PathBuf::from(home);
        }
    } else if let Some(rest) = p.strip_prefix("~/") {
        if let Some(home) = std::env::var_os("HOME") {
            return PathBuf::from(home).join(rest);
        }
    }
    PathBuf::from(p)
}

/// Where a recording's notes were written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SavedPaths {
    pub refined: PathBuf,
    pub raw: PathBuf,
}

/// Write the two linked notes under `save_dir`: the refined note as a sibling
/// `<base>.md`, the raw note under `raw/<raw_base>.md`. The **raw note is written
/// first** (source of truth) so an interruption can never leave a refined note
/// without its transcript.
///
/// # Errors
/// Propagates filesystem errors.
pub fn save_notes(
    save_dir: &str,
    base: &str,
    raw_base: &str,
    refined_md: &str,
    raw_md: &str,
) -> crate::Result<SavedPaths> {
    let dir = expand_tilde(save_dir);
    let refined = dir.join(format!("{base}.md"));
    let raw = dir.join("raw").join(format!("{raw_base}.md"));
    write_atomic(&raw, raw_md)?;
    write_atomic(&refined, refined_md)?;
    Ok(SavedPaths { refined, raw })
}

/// Path for a recording's kept audio file: `audio/<base>.wav` under the save dir.
#[must_use]
pub fn audio_path(save_dir: &str, base: &str) -> PathBuf {
    expand_tilde(save_dir)
        .join("audio")
        .join(format!("{base}.wav"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Speaker, Turn};

    #[test]
    fn expand_tilde_uses_home() {
        if let Some(home) = std::env::var_os("HOME") {
            let p = expand_tilde("~/x/y");
            assert!(p.starts_with(&home) && p.ends_with("x/y"));
        }
        assert_eq!(expand_tilde("/abs/path"), PathBuf::from("/abs/path"));
    }

    #[test]
    fn save_notes_writes_raw_subfolder_and_refined_sibling() {
        let dir = std::env::temp_dir().join(format!("voz-save-{}", std::process::id()));
        let dir_s = dir.to_str().unwrap();
        let paths = save_notes(
            dir_s,
            "2026-06-05 14-07 Sync",
            "2026-06-05 14-07 Sync (raw)",
            "REFINED",
            "RAW",
        )
        .unwrap();
        assert!(paths.refined.ends_with("2026-06-05 14-07 Sync.md"));
        assert!(paths.raw.ends_with("raw/2026-06-05 14-07 Sync (raw).md"));
        assert_eq!(std::fs::read_to_string(&paths.refined).unwrap(), "REFINED");
        assert_eq!(std::fs::read_to_string(&paths.raw).unwrap(), "RAW");
        assert!(audio_path(dir_s, "x").ends_with("audio/x.wav"));
        let _ = std::fs::remove_dir_all(&dir);
    }

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

    #[test]
    fn frontmatter_stripped_and_raw_note_parsed_back() {
        assert_eq!(
            strip_frontmatter("---\na: 1\n---\n\nbody here"),
            "body here"
        );
        assert_eq!(strip_frontmatter("no frontmatter"), "no frontmatter");

        let md = "---\ncreated: x\n---\n\n**Me:** hello there\n\n**Them:** hi back\n\n> link";
        let t = parse_raw_note(md);
        assert_eq!(t.turns.len(), 2);
        assert_eq!(t.turns[0].speaker, Speaker::Me);
        assert_eq!(t.turns[0].text, "hello there");
        assert_eq!(t.turns[1].speaker, Speaker::Them);
        assert_eq!(t.voices(), vec!["Me", "Them"]);
    }

    #[test]
    fn srt_formats_indexed_timestamped_cues() {
        let tr = Transcript {
            turns: vec![
                Turn {
                    speaker: Speaker::Me,
                    text: "hello world".into(),
                    start_ms: 0,
                    end_ms: 2500,
                },
                Turn {
                    speaker: Speaker::Them,
                    text: " hi ".into(),
                    start_ms: 63_000,
                    end_ms: 64_010,
                },
            ],
            language: None,
        };
        let srt = transcript_to_srt(&tr);
        assert!(srt.contains("1\n00:00:00,000 --> 00:00:02,500\nhello world"));
        assert!(srt.contains("2\n00:01:03,000 --> 00:01:04,010\nhi"));
    }
}
