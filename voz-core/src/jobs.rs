// SPDX-License-Identifier: Apache-2.0
//! The background job queue: each `Stop` enqueues a [`Job`] (transcribe → refine)
//! that runs independently of the recorder, so recording and processing never
//! block each other. A bounded number run at once so a long meeting doesn't starve
//! a new recording.

use crate::model::Source;

/// Stable identifier for a job.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct JobId(pub u64);

/// Lifecycle of a processing job.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JobState {
    Queued,
    Transcribing,
    Refining,
    Done,
    Failed,
}

impl JobState {
    /// Whether the job is still occupying a worker slot.
    #[must_use]
    pub fn is_active(self) -> bool {
        matches!(self, JobState::Transcribing | JobState::Refining)
    }

    /// Whether the job has reached a terminal state.
    #[must_use]
    pub fn is_terminal(self) -> bool {
        matches!(self, JobState::Done | JobState::Failed)
    }
}

/// A unit of post-recording work shown in the History tab.
#[derive(Debug, Clone)]
pub struct Job {
    pub id: JobId,
    pub title: String,
    pub source: Source,
    pub state: JobState,
    /// Populated when the raw transcript is finalized (persisted first).
    pub raw_saved: bool,
}

impl Job {
    fn new(id: JobId, source: Source) -> Self {
        Job {
            id,
            title: String::new(),
            source,
            state: JobState::Queued,
            raw_saved: false,
        }
    }
}

/// In-memory record of jobs and their lifecycle (for the History tab + tray).
/// Concurrency is bounded by the engine's worker slots, not here; the transition
/// rules live here and are unit-tested.
#[derive(Debug)]
pub struct JobQueue {
    jobs: Vec<Job>,
    next_id: u64,
}

impl Default for JobQueue {
    fn default() -> Self {
        Self::new()
    }
}

impl JobQueue {
    /// Create an empty queue. How many jobs run *at once* is bounded by the
    /// engine's worker slots (see `engine::Slots`); this queue is pure bookkeeping
    /// for the History tab and tray badge.
    #[must_use]
    pub fn new() -> Self {
        JobQueue {
            jobs: Vec::new(),
            next_id: 1,
        }
    }

    /// Enqueue a new job for a finished recording; returns its id.
    pub fn enqueue(&mut self, source: Source) -> JobId {
        let id = JobId(self.next_id);
        self.next_id += 1;
        self.jobs.push(Job::new(id, source));
        id
    }

    #[must_use]
    pub fn jobs(&self) -> &[Job] {
        &self.jobs
    }

    #[must_use]
    pub fn active_count(&self) -> usize {
        self.jobs.iter().filter(|j| j.state.is_active()).count()
    }

    /// Update a job's state. Returns false if the id is unknown.
    pub fn set_state(&mut self, id: JobId, state: JobState) -> bool {
        if let Some(j) = self.jobs.iter_mut().find(|j| j.id == id) {
            j.state = state;
            true
        } else {
            false
        }
    }

    /// Mark the raw transcript as persisted (source of truth saved first).
    pub fn mark_raw_saved(&mut self, id: JobId) -> bool {
        if let Some(j) = self.jobs.iter_mut().find(|j| j.id == id) {
            j.raw_saved = true;
            true
        } else {
            false
        }
    }

    /// Remove terminal jobs from the in-memory list (e.g. user dismiss).
    pub fn dismiss(&mut self, id: JobId) {
        self.jobs.retain(|j| j.id != id || !j.state.is_terminal());
    }

    /// Count of jobs the tray badge reports as "processing".
    #[must_use]
    pub fn processing(&self) -> usize {
        self.jobs.iter().filter(|j| !j.state.is_terminal()).count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enqueue_assigns_increasing_ids() {
        let mut q = JobQueue::new();
        let a = q.enqueue(Source::Both);
        let b = q.enqueue(Source::Mic);
        assert_eq!(a, JobId(1));
        assert_eq!(b, JobId(2));
        assert_eq!(q.processing(), 2);
    }

    #[test]
    fn active_count_tracks_in_flight_jobs() {
        let mut q = JobQueue::new();
        let a = q.enqueue(Source::Both);
        let _b = q.enqueue(Source::Both);
        assert_eq!(q.active_count(), 0); // both Queued
        q.set_state(a, JobState::Transcribing);
        assert_eq!(q.active_count(), 1);
        q.set_state(a, JobState::Done);
        assert_eq!(q.active_count(), 0);
    }

    #[test]
    fn lifecycle_and_raw_saved_flag() {
        let mut q = JobQueue::new();
        let id = q.enqueue(Source::Both);
        assert!(q.set_state(id, JobState::Transcribing));
        assert!(q.mark_raw_saved(id));
        assert!(q.set_state(id, JobState::Refining));
        assert!(q.set_state(id, JobState::Done));
        let j = q.jobs().iter().find(|j| j.id == id).unwrap();
        assert!(j.raw_saved && j.state == JobState::Done);
        assert_eq!(q.processing(), 0);
    }

    #[test]
    fn dismiss_removes_only_terminal() {
        let mut q = JobQueue::new();
        let id = q.enqueue(Source::Both);
        q.dismiss(id); // still queued -> not removed
        assert_eq!(q.jobs().len(), 1);
        q.set_state(id, JobState::Failed);
        q.dismiss(id);
        assert_eq!(q.jobs().len(), 0);
    }

    #[test]
    fn unknown_id_is_reported() {
        let mut q = JobQueue::new();
        assert!(!q.set_state(JobId(99), JobState::Done));
    }
}
