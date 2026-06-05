// SPDX-License-Identifier: Apache-2.0
//! The History index (feature `history`): a small SQLite database mirroring each
//! saved note's metadata for instant search/filter in the History tab. The
//! Markdown notes remain the portable source of truth; this is a derived cache.

use crate::model::{NoteMeta, Source};
use rusqlite::Connection;
use std::path::{Path, PathBuf};

/// One row in the history index.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoryRecord {
    pub id: i64,
    pub created: String,
    pub title: String,
    pub source: String,
    pub voices: String,
    pub words: i64,
    pub duration_secs: i64,
    pub refine_backend: String,
    pub lossless_ok: bool,
    pub refined_path: String,
    pub raw_path: String,
}

/// The history database.
#[derive(Debug)]
pub struct History {
    conn: Connection,
}

fn source_str(s: Source) -> &'static str {
    match s {
        Source::Mic => "Mic",
        Source::System => "System",
        Source::Both => "Both",
    }
}

impl History {
    /// `$XDG_DATA_HOME/voz/history.sqlite` (fallback `~/.local/share/voz/...`).
    #[must_use]
    pub fn default_path() -> PathBuf {
        let base = std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")))
            .unwrap_or_else(|| PathBuf::from("."));
        base.join("voz").join("history.sqlite")
    }

    /// Open (creating if needed) the index at `path` and run migrations.
    ///
    /// # Errors
    /// Returns [`crate::Error::Storage`] if the database can't be opened/migrated.
    pub fn open(path: &Path) -> crate::Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(path).map_err(|e| crate::Error::Storage(e.to_string()))?;
        Self::from_conn(conn)
    }

    /// In-memory index (tests).
    ///
    /// # Errors
    /// Returns [`crate::Error::Storage`] on failure.
    pub fn open_in_memory() -> crate::Result<Self> {
        let conn =
            Connection::open_in_memory().map_err(|e| crate::Error::Storage(e.to_string()))?;
        Self::from_conn(conn)
    }

    fn from_conn(conn: Connection) -> crate::Result<Self> {
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS notes (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                created TEXT NOT NULL,
                title TEXT NOT NULL,
                source TEXT NOT NULL,
                voices TEXT NOT NULL,
                words INTEGER NOT NULL,
                duration_secs INTEGER NOT NULL,
                refine_backend TEXT NOT NULL,
                lossless_ok INTEGER NOT NULL,
                refined_path TEXT NOT NULL UNIQUE,
                raw_path TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_notes_created ON notes(created DESC);",
        )
        .map_err(|e| crate::Error::Storage(e.to_string()))?;
        Ok(History { conn })
    }

    /// Insert (or replace) a note record. Returns the row id.
    ///
    /// # Errors
    /// Returns [`crate::Error::Storage`] on failure.
    pub fn insert(
        &self,
        title: &str,
        meta: &NoteMeta,
        refined_path: &str,
        raw_path: &str,
    ) -> crate::Result<i64> {
        self.conn
            .execute(
                "INSERT OR REPLACE INTO notes
                 (created,title,source,voices,words,duration_secs,refine_backend,lossless_ok,refined_path,raw_path)
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)",
                rusqlite::params![
                    meta.created,
                    title,
                    source_str(meta.source),
                    meta.voices.join(", "),
                    meta.words as i64,
                    meta.duration_secs as i64,
                    meta.refine_backend,
                    i64::from(meta.lossless_ok),
                    refined_path,
                    raw_path,
                ],
            )
            .map_err(|e| crate::Error::Storage(e.to_string()))?;
        Ok(self.conn.last_insert_rowid())
    }

    /// Most recent `limit` notes, newest first.
    ///
    /// # Errors
    /// Returns [`crate::Error::Storage`] on failure.
    pub fn recent(&self, limit: u32) -> crate::Result<Vec<HistoryRecord>> {
        self.query(
            "SELECT id,created,title,source,voices,words,duration_secs,refine_backend,lossless_ok,refined_path,raw_path
             FROM notes ORDER BY created DESC LIMIT ?1",
            rusqlite::params![limit],
        )
    }

    /// Full-text-ish search over title (case-insensitive substring), newest first.
    ///
    /// # Errors
    /// Returns [`crate::Error::Storage`] on failure.
    pub fn search(&self, term: &str, limit: u32) -> crate::Result<Vec<HistoryRecord>> {
        let like = format!("%{term}%");
        self.query(
            "SELECT id,created,title,source,voices,words,duration_secs,refine_backend,lossless_ok,refined_path,raw_path
             FROM notes WHERE title LIKE ?1 COLLATE NOCASE ORDER BY created DESC LIMIT ?2",
            rusqlite::params![like, limit],
        )
    }

    fn query(&self, sql: &str, params: impl rusqlite::Params) -> crate::Result<Vec<HistoryRecord>> {
        let mut stmt = self
            .conn
            .prepare(sql)
            .map_err(|e| crate::Error::Storage(e.to_string()))?;
        let rows = stmt
            .query_map(params, |r| {
                Ok(HistoryRecord {
                    id: r.get(0)?,
                    created: r.get(1)?,
                    title: r.get(2)?,
                    source: r.get(3)?,
                    voices: r.get(4)?,
                    words: r.get(5)?,
                    duration_secs: r.get(6)?,
                    refine_backend: r.get(7)?,
                    lossless_ok: r.get::<_, i64>(8)? != 0,
                    refined_path: r.get(9)?,
                    raw_path: r.get(10)?,
                })
            })
            .map_err(|e| crate::Error::Storage(e.to_string()))?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row.map_err(|e| crate::Error::Storage(e.to_string()))?);
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn meta(created: &str, words: usize) -> NoteMeta {
        NoteMeta {
            created: created.into(),
            duration_secs: 60,
            source: Source::Both,
            voices: vec!["Me".into(), "Them".into()],
            model: "turbo".into(),
            refine_backend: "Claude Code".into(),
            lossless_ok: true,
            words,
        }
    }

    #[test]
    fn insert_recent_and_search() {
        let h = History::open_in_memory().unwrap();
        h.insert(
            "Planning sync",
            &meta("2026-06-05T14:07:00", 64),
            "/n/a.md",
            "/n/raw/a.md",
        )
        .unwrap();
        h.insert(
            "Standup notes",
            &meta("2026-06-05T09:15:00", 120),
            "/n/b.md",
            "/n/raw/b.md",
        )
        .unwrap();

        let recent = h.recent(10).unwrap();
        assert_eq!(recent.len(), 2);
        assert_eq!(recent[0].title, "Planning sync"); // newest first
        assert_eq!(recent[0].voices, "Me, Them");
        assert!(recent[0].lossless_ok);

        let found = h.search("standup", 10).unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].words, 120);

        assert_eq!(h.search("nothing", 10).unwrap().len(), 0);
    }

    #[test]
    fn replace_on_same_refined_path() {
        let h = History::open_in_memory().unwrap();
        h.insert(
            "v1",
            &meta("2026-06-05T14:00:00", 10),
            "/same.md",
            "/raw.md",
        )
        .unwrap();
        h.insert(
            "v2",
            &meta("2026-06-05T14:00:00", 20),
            "/same.md",
            "/raw.md",
        )
        .unwrap();
        let recent = h.recent(10).unwrap();
        assert_eq!(recent.len(), 1); // UNIQUE refined_path -> replaced
        assert_eq!(recent[0].title, "v2");
    }
}
