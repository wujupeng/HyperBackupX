use std::time::Duration;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::common::{PolicyId, ScheduleId};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Schedule {
    pub schedule_id: ScheduleId,
    pub mode: ScheduleMode,
    pub cron_expression: Option<String>,
    pub interval: Option<u64>,
    pub time_of_day: Option<String>,
    pub day_of_week: Option<u8>,
    pub day_of_month: Option<u8>,
    pub next_run_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ScheduleMode {
    Manual,
    Interval,
    Daily,
    Weekly,
    Monthly,
    Cron,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetentionPolicy {
    pub policy_id: PolicyId,
    pub mode: RetentionMode,
    pub keep_last_n: Option<u32>,
    pub time_based_retention: Option<Duration>,
    pub gfs_config: Option<GfsConfig>,
    pub smart_rules: Option<SmartRules>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RetentionMode {
    KeepAll,
    KeepLastN,
    TimeBased,
    Gfs,
    Smart,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GfsConfig {
    pub daily: u32,
    pub weekly: u32,
    pub monthly: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SmartRules {
    pub min_versions: u32,
    pub max_age_days: u32,
    pub prefer_first_and_last_of_day: bool,
}