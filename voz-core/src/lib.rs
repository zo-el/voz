// SPDX-License-Identifier: Apache-2.0
//! `voz-core` — the framework-agnostic engine for Voz.
//!
//! Pure-Rust domain logic: capture orchestration, transcription/refinement
//! traits, the Me/Them attribution, note serialization, the background job
//! queue, and settings. Native backends (audio via cpal/PipeWire, transcription
//! via whisper.cpp) live behind the `audio` / `whisper` features and are wired in
//! later milestones; everything here builds and is tested with no system deps.
//!
//! See `docs/ARCHITECTURE.md` for the design this implements.

pub mod audio;
#[cfg(feature = "audio")]
pub mod capture;
pub mod command;
pub mod config;
#[cfg(feature = "engine")]
pub mod engine;
pub mod event;
pub mod gpu;
#[cfg(feature = "history")]
pub mod history;
pub mod jobs;
pub mod model;
#[cfg(feature = "whisper")]
pub mod models;
pub mod pipeline;
pub mod recorder;
pub mod refine;
#[cfg(feature = "refine")]
pub mod refine_backends;
pub mod store;
pub mod transcribe;
pub mod update;
#[cfg(feature = "whisper")]
pub mod whisper;

pub use command::Command;
pub use config::Settings;
pub use event::{Event, TrayState};
pub use jobs::{Job, JobId, JobQueue, JobState};
pub use model::{NoteMeta, RefineStyle, Source, Speaker, Transcript, Turn};
pub use recorder::{RecState, Recorder};

/// Errors surfaced by the engine. Every variant maps to a UI error state.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("invalid state transition: {0}")]
    Transition(&'static str),
    #[error("no audio source available: {0}")]
    NoSource(String),
    #[error("transcription failed: {0}")]
    Transcribe(String),
    #[error("refine backend error: {0}")]
    Refine(String),
    #[error("storage error: {0}")]
    Storage(String),
    #[error("config error: {0}")]
    Config(String),
    #[error("i/o error: {0}")]
    Io(#[from] std::io::Error),
}

/// Convenience result alias used throughout the crate.
pub type Result<T> = std::result::Result<T, Error>;
