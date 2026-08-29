use std::time::Duration;

use chrono::{DateTime, Utc};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use hbx_core::domain::common::JobId;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CompatExecutionState {
    Pending,
    Aligning,
    Scanning,
    Chunking,
    Encrypting,
    Uploading,
    CompCommitting,
    Verifying,
    Success,
    Failed,
    Paused,
}

impl CompatExecutionState {
    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Success | Self::Failed)
    }

    pub fn is_active(&self) -> bool {
        !self.is_terminal() && *self != Self::Paused
    }

    pub fn next(&self) -> Option<Self> {
        match self {
            Self::Pending => Some(Self::Aligning),
            Self::Aligning => Some(Self::Scanning),
            Self::Scanning => Some(Self::Chunking),
            Self::Chunking => Some(Self::Encrypting),
            Self::Encrypting => Some(Self::Uploading),
            Self::Uploading => Some(Self::CompCommitting),
            Self::CompCommitting => Some(Self::Verifying),
            Self::Verifying => Some(Self::Success),
            Self::Success | Self::Failed | Self::Paused => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompatCheckpoint {
    pub job_id: JobId,
    pub state: CompatExecutionState,
    pub files_completed: u64,
    pub chunks_completed: u64,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompatibilityExecution {
    pub execution_id: Uuid,
    pub job_id: JobId,
    pub started_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub state: CompatExecutionState,
    pub progress: f64,
    pub checkpoint: Option<CompatCheckpoint>,
}

impl CompatibilityExecution {
    pub fn new(job_id: &JobId) -> Self {
        Self {
            execution_id: Uuid::new_v4(),
            job_id: job_id.clone(),
            started_at: Utc::now(),
            completed_at: None,
            state: CompatExecutionState::Pending,
            progress: 0.0,
            checkpoint: None,
        }
    }

    pub fn advance(&mut self) -> Option<CompatExecutionState> {
        if let Some(next_state) = self.state.next() {
            self.state = next_state;
            if next_state.is_terminal() {
                self.completed_at = Some(Utc::now());
                self.progress = 1.0;
            }
            Some(next_state)
        } else {
            None
        }
    }

    pub fn fail(&mut self) {
        self.state = CompatExecutionState::Failed;
        self.completed_at = Some(Utc::now());
    }

    pub fn pause(&mut self) {
        self.state = CompatExecutionState::Paused;
    }

    pub fn resume(&mut self) {
        if self.state == CompatExecutionState::Paused {
            self.state = CompatExecutionState::Pending;
        }
    }

    pub fn set_progress(&mut self, progress: f64) {
        self.progress = progress.clamp(0.0, 1.0);
    }

    pub fn save_checkpoint(&mut self, files_completed: u64, chunks_completed: u64) {
        self.checkpoint = Some(CompatCheckpoint {
            job_id: self.job_id.clone(),
            state: self.state,
            files_completed,
            chunks_completed,
            timestamp: Utc::now(),
        });
    }

    pub fn restore_from_checkpoint(&mut self, checkpoint: &CompatCheckpoint) {
        self.state = checkpoint.state;
        self.checkpoint = Some(checkpoint.clone());
    }
}

pub struct CompatExecutionTracker {
    inner: Mutex<CompatibilityExecution>,
}

impl CompatExecutionTracker {
    pub fn new(job_id: &JobId) -> Self {
        Self {
            inner: Mutex::new(CompatibilityExecution::new(job_id)),
        }
    }

    pub fn set_state(&self, state: CompatExecutionState) {
        let mut inner = self.inner.lock();
        inner.state = state;
        if state.is_terminal() {
            inner.completed_at = Some(Utc::now());
            if state == CompatExecutionState::Success {
                inner.progress = 1.0;
            }
        }
    }

    pub fn set_progress(&self, progress: f64) {
        self.inner.lock().set_progress(progress);
    }

    pub fn advance(&self) -> Option<CompatExecutionState> {
        self.inner.lock().advance()
    }

    pub fn fail(&self) {
        self.inner.lock().fail();
    }

    pub fn save_checkpoint(&self, files_completed: u64, chunks_completed: u64) {
        self.inner.lock().save_checkpoint(files_completed, chunks_completed);
    }

    pub fn snapshot(&self) -> CompatibilityExecution {
        self.inner.lock().clone()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExceptionAction {
    Retry,
    Skip,
    Abort,
    Continue,
    MarkFailed,
}

pub fn decide_exception_action(state: CompatExecutionState, error_kind: &str) -> ExceptionAction {
    match (state, error_kind) {
        (_, "disk_full") => ExceptionAction::Abort,
        (_, "permission_denied") => ExceptionAction::Skip,
        (_, "source_missing") => ExceptionAction::Skip,
        (_, "file_locked") => ExceptionAction::Retry,
        (_, "network_break") => ExceptionAction::Retry,
        (_, "repo_unavailable") => ExceptionAction::Retry,
        (_, "process_killed") => ExceptionAction::Continue,
        (_, "power_off") => ExceptionAction::Continue,
        (_, _) => ExceptionAction::MarkFailed,
    }
}

pub fn retry_backoff(attempt: u32, base: Duration, max: Duration) -> Duration {
    if attempt == 0 {
        return base;
    }
    let multiplier = 2u64.saturating_pow(attempt.min(10));
    let total = base.as_millis() as u64 * multiplier;
    Duration::from_millis(total.min(max.as_millis() as u64))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_state_sequence() {
        let states = [
            CompatExecutionState::Pending,
            CompatExecutionState::Aligning,
            CompatExecutionState::Scanning,
            CompatExecutionState::Chunking,
            CompatExecutionState::Encrypting,
            CompatExecutionState::Uploading,
            CompatExecutionState::CompCommitting,
            CompatExecutionState::Verifying,
            CompatExecutionState::Success,
        ];
        for i in 0..states.len() - 1 {
            assert_eq!(states[i].next(), Some(states[i + 1]));
        }
        assert_eq!(CompatExecutionState::Success.next(), None);
        assert_eq!(CompatExecutionState::Failed.next(), None);
    }

    #[test]
    fn test_execution_advance() {
        let job_id = JobId(Uuid::new_v4());
        let mut exec = CompatibilityExecution::new(&job_id);
        assert_eq!(exec.state, CompatExecutionState::Pending);

        exec.advance();
        assert_eq!(exec.state, CompatExecutionState::Aligning);

        exec.advance();
        assert_eq!(exec.state, CompatExecutionState::Scanning);

        for _ in 0..6 {
            exec.advance();
        }
        assert_eq!(exec.state, CompatExecutionState::Success);
        assert!(exec.completed_at.is_some());
        assert_eq!(exec.progress, 1.0);
    }

    #[test]
    fn test_execution_fail() {
        let job_id = JobId(Uuid::new_v4());
        let mut exec = CompatibilityExecution::new(&job_id);
        exec.advance();
        exec.fail();
        assert_eq!(exec.state, CompatExecutionState::Failed);
        assert!(exec.completed_at.is_some());
    }

    #[test]
    fn test_checkpoint_save_restore() {
        let job_id = JobId(Uuid::new_v4());
        let mut exec = CompatibilityExecution::new(&job_id);
        exec.advance();
        exec.advance();
        exec.save_checkpoint(5, 10);

        let checkpoint = exec.checkpoint.as_ref().unwrap().clone();
        assert_eq!(checkpoint.files_completed, 5);
        assert_eq!(checkpoint.chunks_completed, 10);

        let mut exec2 = CompatibilityExecution::new(&job_id);
        exec2.restore_from_checkpoint(&checkpoint);
        assert_eq!(exec2.state, CompatExecutionState::Scanning);
    }

    #[test]
    fn test_tracker_thread_safety() {
        let job_id = JobId(Uuid::new_v4());
        let tracker = CompatExecutionTracker::new(&job_id);

        tracker.set_progress(0.5);
        assert_eq!(tracker.snapshot().progress, 0.5);

        tracker.advance();
        assert_eq!(tracker.snapshot().state, CompatExecutionState::Aligning);

        tracker.fail();
        assert_eq!(tracker.snapshot().state, CompatExecutionState::Failed);
    }

    #[test]
    fn test_exception_actions() {
        assert_eq!(
            decide_exception_action(CompatExecutionState::Uploading, "disk_full"),
            ExceptionAction::Abort
        );
        assert_eq!(
            decide_exception_action(CompatExecutionState::Scanning, "permission_denied"),
            ExceptionAction::Skip
        );
        assert_eq!(
            decide_exception_action(CompatExecutionState::Uploading, "network_break"),
            ExceptionAction::Retry
        );
        assert_eq!(
            decide_exception_action(CompatExecutionState::Chunking, "process_killed"),
            ExceptionAction::Continue
        );
        assert_eq!(
            decide_exception_action(CompatExecutionState::Aligning, "unknown_error"),
            ExceptionAction::MarkFailed
        );
    }

    #[test]
    fn test_retry_backoff() {
        let base = Duration::from_millis(100);
        let max = Duration::from_secs(30);

        assert_eq!(retry_backoff(0, base, max), Duration::from_millis(100));
        assert_eq!(retry_backoff(1, base, max), Duration::from_millis(200));
        assert_eq!(retry_backoff(2, base, max), Duration::from_millis(400));
        assert_eq!(retry_backoff(3, base, max), Duration::from_millis(800));

        let large = retry_backoff(20, base, max);
        assert!(large <= max);
    }

    #[test]
    fn test_state_is_terminal() {
        assert!(CompatExecutionState::Success.is_terminal());
        assert!(CompatExecutionState::Failed.is_terminal());
        assert!(!CompatExecutionState::Pending.is_terminal());
        assert!(!CompatExecutionState::Aligning.is_terminal());
        assert!(!CompatExecutionState::CompCommitting.is_terminal());
    }

    #[test]
    fn test_pause_resume() {
        let job_id = JobId(Uuid::new_v4());
        let mut exec = CompatibilityExecution::new(&job_id);
        exec.advance();
        exec.pause();
        assert_eq!(exec.state, CompatExecutionState::Paused);
        exec.resume();
        assert_eq!(exec.state, CompatExecutionState::Pending);
    }
}