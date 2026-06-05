// SPDX-License-Identifier: Apache-2.0
//! Commands the frontend sends to the engine (the only way the thin UI drives
//! `voz-core`). Mapped 1:1 from typed Tauri commands; no business logic in the UI.

use crate::config::Settings;
use crate::jobs::JobId;
use crate::model::{RefineStyle, Source};

#[derive(Debug, Clone)]
pub enum Command {
    // --- recorder (singleton) ---
    Start {
        source: Source,
    },
    Pause,
    Resume,
    Stop,
    Cancel,
    // --- jobs (background, operate on a specific note) ---
    Rerefine {
        job: JobId,
        style: Option<RefineStyle>,
    },
    RetryJob(JobId),
    DismissJob(JobId),
    // --- misc ---
    UpdateSettings(Box<Settings>),
    DownloadModel(String),
}
