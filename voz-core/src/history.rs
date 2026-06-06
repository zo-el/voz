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
        // Migration: a `body` column holds the transcript text for full-text search.
        // Ignored if it already exists (SQLite errors on a duplicate column).
        let _ = conn.execute(
            "ALTER TABLE notes ADD COLUMN body TEXT NOT NULL DEFAULT ''",
            [],
        );
        let history = History { conn };
        // One-time backfill of pre-migration rows (cheap no-op once populated).
        let _ = history.backfill_bodies();
        Ok(history)
    }

    /// Populate `body` for rows that predate the column by reading their raw notes.
    /// Best-effort; returns how many rows were updated.
    ///
    /// # Errors
    /// Returns [`crate::Error::Storage`] on a query failure.
    pub fn backfill_bodies(&self) -> crate::Result<usize> {
        let pending: Vec<(i64, String)> = {
            let mut stmt = self
                .conn
                .prepare("SELECT id, raw_path FROM notes WHERE body = '' OR body IS NULL")
                .map_err(|e| crate::Error::Storage(e.to_string()))?;
            let rows = stmt
                .query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))
                .map_err(|e| crate::Error::Storage(e.to_string()))?;
            rows.flatten().collect()
        };
        let mut n = 0;
        for (id, raw_path) in pending {
            if let Ok(md) = std::fs::read_to_string(&raw_path) {
                let text = crate::store::parse_raw_note(&md).plain_text();
                if !text.is_empty()
                    && self
                        .conn
                        .execute(
                            "UPDATE notes SET body = ?1 WHERE id = ?2",
                            rusqlite::params![text, id],
                        )
                        .is_ok()
                {
                    n += 1;
                }
            }
        }
        Ok(n)
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
        body: &str,
    ) -> crate::Result<i64> {
        self.conn
            .execute(
                "INSERT OR REPLACE INTO notes
                 (created,title,source,voices,words,duration_secs,refine_backend,lossless_ok,refined_path,raw_path,body)
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)",
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
                    body,
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

    /// Full-text-ish search over the title **and transcript body** (case-insensitive
    /// substring), newest first.
    ///
    /// # Errors
    /// Returns [`crate::Error::Storage`] on failure.
    pub fn search(&self, term: &str, limit: u32) -> crate::Result<Vec<HistoryRecord>> {
        let like = format!("%{term}%");
        self.query(
            "SELECT id,created,title,source,voices,words,duration_secs,refine_backend,lossless_ok,refined_path,raw_path
             FROM notes WHERE (title LIKE ?1 OR body LIKE ?1) COLLATE NOCASE
             ORDER BY created DESC LIMIT ?2",
            rusqlite::params![like, limit],
        )
    }

    /// Look up a record by its refined-note path (the History tab's row key).
    ///
    /// # Errors
    /// Returns [`crate::Error::Storage`] on failure.
    pub fn get_by_refined(&self, refined_path: &str) -> crate::Result<Option<HistoryRecord>> {
        let rows = self.query(
            "SELECT id,created,title,source,voices,words,duration_secs,refine_backend,lossless_ok,refined_path,raw_path
             FROM notes WHERE refined_path = ?1 LIMIT 1",
            rusqlite::params![refined_path],
        )?;
        Ok(rows.into_iter().next())
    }

    /// Remove a record by refined-note path. Returns true if a row was deleted.
    ///
    /// # Errors
    /// Returns [`crate::Error::Storage`] on failure.
    pub fn delete_by_refined(&self, refined_path: &str) -> crate::Result<bool> {
        let n = self
            .conn
            .execute(
                "DELETE FROM notes WHERE refined_path = ?1",
                rusqlite::params![refined_path],
            )
            .map_err(|e| crate::Error::Storage(e.to_string()))?;
        Ok(n > 0)
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
            "we agreed Priya ships the parser by Friday",
        )
        .unwrap();
        h.insert(
            "Standup notes",
            &meta("2026-06-05T09:15:00", 120),
            "/n/b.md",
            "/n/raw/b.md",
            "blocked on the staging database",
        )
        .unwrap();

        let recent = h.recent(10).unwrap();
        assert_eq!(recent.len(), 2);
        assert_eq!(recent[0].title, "Planning sync"); // newest first
        assert_eq!(recent[0].voices, "Me, Them");
        assert!(recent[0].lossless_ok);

        // search matches the title...
        let by_title = h.search("standup", 10).unwrap();
        assert_eq!(by_title.len(), 1);
        assert_eq!(by_title[0].words, 120);

        // ...and the transcript body (full-text content search)
        let by_body = h.search("priya", 10).unwrap();
        assert_eq!(by_body.len(), 1);
        assert_eq!(by_body[0].title, "Planning sync");
        assert_eq!(h.search("staging database", 10).unwrap().len(), 1);

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
            "first body",
        )
        .unwrap();
        h.insert(
            "v2",
            &meta("2026-06-05T14:00:00", 20),
            "/same.md",
            "/raw.md",
            "second body",
        )
        .unwrap();
        let recent = h.recent(10).unwrap();
        assert_eq!(recent.len(), 1); // UNIQUE refined_path -> replaced
        assert_eq!(recent[0].title, "v2");
    }
}
