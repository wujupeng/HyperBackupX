//! 校验调度：立即/定时/后台低优先级。


use parking_lot::RwLock;
use chrono::{DateTime, Utc};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum VerifySchedulerError {
    #[error("scheduler already running")]
    AlreadyRunning,
    #[error("verify error: {0}")]
    Verify(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VerifySchedule {
    Immediate,
    Scheduled { interval_secs: u64 },
    Background,
}

#[derive(Debug, Clone)]
pub struct VerifyRecord {
    pub started_at: DateTime<Utc>,
    pub finished_at: DateTime<Utc>,
    pub total_checks: usize,
    pub passed: usize,
    pub failed: usize,
    pub schedule: VerifySchedule,
}

pub struct VerifyScheduler {
    schedule: RwLock<VerifySchedule>,
    history: RwLock<Vec<VerifyRecord>>,
    running: RwLock<bool>,
}

impl VerifyScheduler {
    pub fn new(schedule: VerifySchedule) -> Self {
        Self {
            schedule: RwLock::new(schedule),
            history: RwLock::new(Vec::new()),
            running: RwLock::new(false),
        }
    }

    pub fn start(&self) -> Result<(), VerifySchedulerError> {
        if *self.running.read() {
            return Err(VerifySchedulerError::AlreadyRunning);
        }
        *self.running.write() = true;
        Ok(())
    }

    pub fn finish(&self, total: usize, passed: usize, failed: usize) {
        let now = Utc::now();
        let record = VerifyRecord {
            started_at: now,
            finished_at: now,
            total_checks: total,
            passed,
            failed,
            schedule: *self.schedule.read(),
        };
        self.history.write().push(record);
        *self.running.write() = false;
    }

    pub fn is_running(&self) -> bool {
        *self.running.read()
    }

    pub fn update_schedule(&self, schedule: VerifySchedule) {
        *self.schedule.write() = schedule;
    }

    pub fn history(&self) -> Vec<VerifyRecord> {
        self.history.read().clone()
    }

    pub fn last_result(&self) -> Option<VerifyRecord> {
        self.history.read().last().cloned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scheduler_immediate() {
        let scheduler = VerifyScheduler::new(VerifySchedule::Immediate);
        scheduler.start().unwrap();
        assert!(scheduler.is_running());
        scheduler.finish(10, 8, 2);
        assert!(!scheduler.is_running());
        let result = scheduler.last_result().unwrap();
        assert_eq!(result.total_checks, 10);
        assert_eq!(result.passed, 8);
        assert_eq!(result.failed, 2);
    }

    #[test]
    fn scheduler_already_running_fails() {
        let scheduler = VerifyScheduler::new(VerifySchedule::Immediate);
        scheduler.start().unwrap();
        let result = scheduler.start();
        assert!(result.is_err());
        scheduler.finish(0, 0, 0);
    }

    #[test]
    fn scheduler_update_schedule() {
        let scheduler = VerifyScheduler::new(VerifySchedule::Immediate);
        scheduler.update_schedule(VerifySchedule::Background);
        scheduler.start().unwrap();
        scheduler.finish(5, 5, 0);
        let result = scheduler.last_result().unwrap();
        assert_eq!(result.schedule, VerifySchedule::Background);
    }

    #[test]
    fn scheduler_history_accumulates() {
        let scheduler = VerifyScheduler::new(VerifySchedule::Immediate);
        scheduler.start().unwrap();
        scheduler.finish(10, 10, 0);
        scheduler.start().unwrap();
        scheduler.finish(5, 3, 2);
        assert_eq!(scheduler.history().len(), 2);
    }
}