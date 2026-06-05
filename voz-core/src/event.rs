// SPDX-License-Identifier: Apache-2.0
//! Events the engine emits to the frontend, the tray, and notifications.
//!
//! Two independent axes: the **recorder** (one at a time) and **jobs** (many,
//! background). The tray icon reflects the derived [`TrayState`].

use crate::audio::Level;
use crate::jobs::{JobId, JobState};
use crate::model::Transcript;
use crate::recorder::RecState;
use std::path::PathBuf;

/// What the panel-closed tray icon shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayState {
    Idle,
    Recording,
    Processing(usize),
    RecordingAndProcessing(usize),
}

impl TrayState {
    /// Derive the tray state from the recorder state and active-job count.
    #[must_use]
    pub fn derive(rec: RecState, processing: usize) -> Self {
        let recording = matches!(rec, RecState::Recording | RecState::Paused);
        match (recording, processing) {
            (false, 0) => TrayState::Idle,
            (true, 0) => TrayState::Recording,
            (false, n) => TrayState::Processing(n),
            (true, n) => TrayState::RecordingAndProcessing(n),
        }
    }

    /// Badge color hint for the UI: `None` (idle), `"rec"`, or `"proc"`.
    #[must_use]
    pub fn badge(self) -> Option<&'static str> {
        match self {
            TrayState::Idle => None,
            TrayState::Recording | TrayState::RecordingAndProcessing(_) => Some("rec"),
            TrayState::Processing(_) => Some("proc"),
        }
    }
}

#[derive(Debug, Clone)]
pub enum Event {
    // recorder
    RecState(RecState),
    Level(Level),
    // per-job lifecycle
    JobState {
        job: JobId,
        state: JobState,
    },
    RawTranscript {
        job: JobId,
        text: Transcript,
    },
    RefineToken {
        job: JobId,
        token: String,
    },
    RefineDone {
        job: JobId,
        refined: String,
        lossless_ok: bool,
    },
    Saved {
        job: JobId,
        refined: PathBuf,
        raw: PathBuf,
    },
    JobFailed {
        job: JobId,
        error: String,
    },
    // global
    Tray(TrayState),
    ModelProgress {
        id: String,
        pct: f32,
    },
    Notify {
        title: String,
        body: String,
        job: JobId,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tray_state_derivation() {
        assert_eq!(TrayState::derive(RecState::Idle, 0), TrayState::Idle);
        assert_eq!(
            TrayState::derive(RecState::Recording, 0),
            TrayState::Recording
        );
        assert_eq!(
            TrayState::derive(RecState::Idle, 2),
            TrayState::Processing(2)
        );
        assert_eq!(
            TrayState::derive(RecState::Paused, 1),
            TrayState::RecordingAndProcessing(1)
        );
    }

    #[test]
    fn badge_colors() {
        assert_eq!(TrayState::Idle.badge(), None);
        assert_eq!(TrayState::Recording.badge(), Some("rec"));
        assert_eq!(TrayState::Processing(3).badge(), Some("proc"));
        assert_eq!(TrayState::RecordingAndProcessing(1).badge(), Some("rec"));
    }
}
