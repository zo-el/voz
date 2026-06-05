// SPDX-License-Identifier: Apache-2.0
//! The recorder state machine — a singleton, independent of processing jobs.
//!
//! `Stop` hands the captured audio to the background job queue (see [`crate::jobs`])
//! and returns the recorder to `Idle`, so a new recording can start while previous
//! ones are still transcribing/refining.

use crate::Error;

/// Recorder state. Processing state lives on individual [`crate::jobs::Job`]s, not here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecState {
    Idle,
    Recording,
    Paused,
}

/// Drives valid recorder transitions; rejects invalid ones.
#[derive(Debug, Clone, Copy)]
pub struct Recorder {
    state: RecState,
}

impl Default for Recorder {
    fn default() -> Self {
        Recorder {
            state: RecState::Idle,
        }
    }
}

impl Recorder {
    #[must_use]
    pub fn state(self) -> RecState {
        self.state
    }

    /// Begin recording.
    /// # Errors
    /// Fails unless currently `Idle`.
    pub fn start(&mut self) -> crate::Result<()> {
        match self.state {
            RecState::Idle => {
                self.state = RecState::Recording;
                Ok(())
            }
            _ => Err(Error::Transition("start requires Idle")),
        }
    }

    /// # Errors
    /// Fails unless currently `Recording`.
    pub fn pause(&mut self) -> crate::Result<()> {
        match self.state {
            RecState::Recording => {
                self.state = RecState::Paused;
                Ok(())
            }
            _ => Err(Error::Transition("pause requires Recording")),
        }
    }

    /// # Errors
    /// Fails unless currently `Paused`.
    pub fn resume(&mut self) -> crate::Result<()> {
        match self.state {
            RecState::Paused => {
                self.state = RecState::Recording;
                Ok(())
            }
            _ => Err(Error::Transition("resume requires Paused")),
        }
    }

    /// Stop and return to `Idle` (caller enqueues a job with the captured audio).
    /// # Errors
    /// Fails if currently `Idle`.
    pub fn stop(&mut self) -> crate::Result<()> {
        match self.state {
            RecState::Recording | RecState::Paused => {
                self.state = RecState::Idle;
                Ok(())
            }
            RecState::Idle => Err(Error::Transition("stop requires an active recording")),
        }
    }

    /// Discard the current recording without producing a job.
    /// # Errors
    /// Fails if currently `Idle`.
    pub fn cancel(&mut self) -> crate::Result<()> {
        self.stop()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn happy_path() {
        let mut r = Recorder::default();
        assert_eq!(r.state(), RecState::Idle);
        r.start().unwrap();
        assert_eq!(r.state(), RecState::Recording);
        r.pause().unwrap();
        assert_eq!(r.state(), RecState::Paused);
        r.resume().unwrap();
        r.stop().unwrap();
        assert_eq!(r.state(), RecState::Idle);
    }

    #[test]
    fn invalid_transitions_rejected() {
        let mut r = Recorder::default();
        assert!(r.pause().is_err());
        assert!(r.stop().is_err());
        r.start().unwrap();
        assert!(r.start().is_err()); // already recording
        assert!(r.resume().is_err()); // not paused
    }

    #[test]
    fn can_restart_after_stop() {
        // The whole point: recorder frees immediately so you can record again.
        let mut r = Recorder::default();
        r.start().unwrap();
        r.stop().unwrap();
        assert!(r.start().is_ok());
    }
}
