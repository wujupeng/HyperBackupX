//! GC 调度器：定时 + 容量阈值 + 手动触发。


use parking_lot::RwLock;
use chrono::{DateTime, Utc};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum SchedulerError {
    #[error("scheduler already running")]
    AlreadyRunning,
    #[error("scheduler not running")]
    NotRunning,
    #[error("GC executor error: {0}")]
    Executor(String),
}

#[derive(Debug, Clone)]
pub struct GcScheduleConfig {
    pub interval_secs: u64,
    pub capacity_threshold_percent: u8,
    pub enabled: bool,
}

impl Default for GcScheduleConfig {
    fn default() -> Self {
        Self {
            interval_secs: 3600,
            capacity_threshold_percent: 80,
            enabled: true,
        }
    }
}

#[derive(Debug, Clone)]
pub struct GcTriggerRecord {
    pub triggered_at: DateTime<Utc>,
    pub trigger_type: GcTriggerType,
    pub purged_count: usize,
    pub freed_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GcTriggerType {
    Manual,
    Scheduled,
    CapacityThreshold,
}

pub struct GcScheduler {
    config: RwLock<GcScheduleConfig>,
    last_run: RwLock<Option<DateTime<Utc>>>,
    history: RwLock<Vec<GcTriggerRecord>>,
    running: RwLock<bool>,
}

impl GcScheduler {
    pub fn new(config: GcScheduleConfig) -> Self {
        Self {
            config: RwLock::new(config),
            last_run: RwLock::new(None),
            history: RwLock::new(Vec::new()),
            running: RwLock::new(false),
        }
    }

    pub fn should_trigger_scheduled(&self) -> bool {
        let config = self.config.read();
        if !config.enabled {
            return false;
        }
        let last = self.last_run.read();
        match *last {
            None => true,
            Some(t) => {
                let elapsed = Utc::now().signed_duration_since(t);
                elapsed.num_seconds() >= config.interval_secs as i64
            }
        }
    }

    pub fn should_trigger_capacity(&self, used_percent: u8) -> bool {
        let config = self.config.read();
        config.enabled && used_percent >= config.capacity_threshold_percent
    }

    pub fn record_trigger(&self, trigger_type: GcTriggerType, purged_count: usize, freed_bytes: u64) {
        let now = Utc::now();
        *self.last_run.write() = Some(now);
        self.history.write().push(GcTriggerRecord {
            triggered_at: now,
            trigger_type,
            purged_count,
            freed_bytes,
        });
    }

    pub fn manual_trigger(&self) -> Result<GcTriggerType, SchedulerError> {
        if *self.running.read() {
            return Err(SchedulerError::AlreadyRunning);
        }
        *self.running.write() = true;
        Ok(GcTriggerType::Manual)
    }

    pub fn finish_trigger(&self) {
        *self.running.write() = false;
    }

    pub fn update_config(&self, config: GcScheduleConfig) {
        *self.config.write() = config;
    }

    pub fn history(&self) -> Vec<GcTriggerRecord> {
        self.history.read().clone()
    }

    pub fn last_run(&self) -> Option<DateTime<Utc>> {
        *self.last_run.read()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scheduler_default_config() {
        let config = GcScheduleConfig::default();
        assert!(config.enabled);
        assert_eq!(config.interval_secs, 3600);
        assert_eq!(config.capacity_threshold_percent, 80);
    }

    #[test]
    fn should_trigger_when_no_previous_run() {
        let scheduler = GcScheduler::new(GcScheduleConfig::default());
        assert!(scheduler.should_trigger_scheduled());
    }

    #[test]
    fn should_not_trigger_when_disabled() {
        let config = GcScheduleConfig { enabled: false, ..Default::default() };
        let scheduler = GcScheduler::new(config);
        assert!(!scheduler.should_trigger_scheduled());
    }

    #[test]
    fn capacity_threshold_trigger() {
        let scheduler = GcScheduler::new(GcScheduleConfig::default());
        assert!(!scheduler.should_trigger_capacity(50));
        assert!(scheduler.should_trigger_capacity(80));
        assert!(scheduler.should_trigger_capacity(95));
    }

    #[test]
    fn manual_trigger_succeeds() {
        let scheduler = GcScheduler::new(GcScheduleConfig::default());
        let trigger = scheduler.manual_trigger().unwrap();
        assert_eq!(trigger, GcTriggerType::Manual);
        scheduler.finish_trigger();
    }

    #[test]
    fn manual_trigger_fails_when_running() {
        let scheduler = GcScheduler::new(GcScheduleConfig::default());
        scheduler.manual_trigger().unwrap();
        let result = scheduler.manual_trigger();
        assert!(result.is_err());
        scheduler.finish_trigger();
    }

    #[test]
    fn record_trigger_updates_history() {
        let scheduler = GcScheduler::new(GcScheduleConfig::default());
        scheduler.record_trigger(GcTriggerType::Manual, 10, 10240);
        scheduler.record_trigger(GcTriggerType::Scheduled, 5, 5120);
        let history = scheduler.history();
        assert_eq!(history.len(), 2);
        assert!(scheduler.last_run().is_some());
    }

    #[test]
    fn update_config_changes_behavior() {
        let scheduler = GcScheduler::new(GcScheduleConfig::default());
        let new_config = GcScheduleConfig { capacity_threshold_percent: 50, ..Default::default() };
        scheduler.update_config(new_config);
        assert!(scheduler.should_trigger_capacity(50));
    }
}