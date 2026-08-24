use std::collections::{BTreeMap, HashSet};

use chrono::{Datelike, Duration as ChronoDuration, NaiveDate, Utc};

use hbx_core::domain::common::{VersionId, VersionSummary};
use hbx_core::domain::schedule::{RetentionMode, RetentionPolicy};
use hbx_core::pipeline::traits::{
    IRetentionPolicyExecutor, RetentionDecision, RetentionError,
};

pub struct RetentionPolicyExecutor;

impl IRetentionPolicyExecutor for RetentionPolicyExecutor {
    fn compute(
        &self,
        versions: &[VersionSummary],
        policy: &RetentionPolicy,
    ) -> Result<RetentionDecision, RetentionError> {
        let mut sorted: Vec<&VersionSummary> = versions.iter().collect();
        sorted.sort_by_key(|b| std::cmp::Reverse(b.timestamp));

        let keep_ids: HashSet<VersionId> = match policy.mode {
            RetentionMode::KeepAll => compute_keep_all(&sorted),
            RetentionMode::KeepLastN => compute_keep_last_n(&sorted, policy)?,
            RetentionMode::TimeBased => compute_time_based(&sorted, policy)?,
            RetentionMode::Gfs => compute_gfs(&sorted, policy)?,
            RetentionMode::Smart => compute_smart(&sorted, policy)?,
        };

        let keep: Vec<VersionId> = sorted
            .iter()
            .filter(|v| keep_ids.contains(&VersionId(v.version_id)))
            .map(|v| VersionId(v.version_id))
            .collect();
        let delete: Vec<VersionId> = sorted
            .iter()
            .filter(|v| !keep_ids.contains(&VersionId(v.version_id)))
            .map(|v| VersionId(v.version_id))
            .collect();

        Ok(RetentionDecision { keep, delete })
    }
}

fn compute_keep_all(versions: &[&VersionSummary]) -> HashSet<VersionId> {
    versions.iter().map(|v| VersionId(v.version_id)).collect()
}

fn compute_keep_last_n(
    versions: &[&VersionSummary],
    policy: &RetentionPolicy,
) -> Result<HashSet<VersionId>, RetentionError> {
    let n = policy
        .keep_last_n
        .ok_or_else(|| RetentionError::InvalidPolicy("keep_last_n not set".into()))?
        as usize;

    Ok(versions
        .iter()
        .take(n)
        .map(|v| VersionId(v.version_id))
        .collect())
}

fn compute_time_based(
    versions: &[&VersionSummary],
    policy: &RetentionPolicy,
) -> Result<HashSet<VersionId>, RetentionError> {
    let retention = policy
        .time_based_retention
        .ok_or_else(|| RetentionError::InvalidPolicy("time_based_retention not set".into()))?;

    let now = Utc::now();
    let cutoff = now - retention;

    Ok(versions
        .iter()
        .filter(|v| v.timestamp >= cutoff)
        .map(|v| VersionId(v.version_id))
        .collect())
}

fn compute_gfs(
    versions: &[&VersionSummary],
    policy: &RetentionPolicy,
) -> Result<HashSet<VersionId>, RetentionError> {
    let gfs = policy
        .gfs_config
        .as_ref()
        .ok_or_else(|| RetentionError::InvalidPolicy("gfs_config not set".into()))?;

    let now = Utc::now();
    let mut keep = HashSet::new();

    keep.extend(select_daily(versions, gfs.daily as usize, now));
    keep.extend(select_weekly(versions, gfs.weekly as usize, now));
    keep.extend(select_monthly(versions, gfs.monthly as usize, now));

    Ok(keep)
}

fn select_daily(
    versions: &[&VersionSummary],
    keep_days: usize,
    now: chrono::DateTime<Utc>,
) -> HashSet<VersionId> {
    if keep_days == 0 {
        return HashSet::new();
    }

    let mut by_day: BTreeMap<NaiveDate, VersionId> = BTreeMap::new();
    for v in versions {
        let date = v.timestamp.date_naive();
        by_day
            .entry(date)
            .or_insert_with(|| VersionId(v.version_id));
    }

    let today = now.date_naive();
    let cutoff = today - ChronoDuration::days(keep_days as i64);

    by_day
        .into_iter()
        .filter(|(date, _)| *date > cutoff)
        .map(|(_, id)| id)
        .collect()
}

fn select_weekly(
    versions: &[&VersionSummary],
    keep_weeks: usize,
    now: chrono::DateTime<Utc>,
) -> HashSet<VersionId> {
    if keep_weeks == 0 {
        return HashSet::new();
    }

    let mut by_week: BTreeMap<i64, VersionId> = BTreeMap::new();
    for v in versions {
        let week = week_number(v.timestamp);
        by_week
            .entry(week)
            .or_insert_with(|| VersionId(v.version_id));
    }

    let current_week = week_number(now);
    let cutoff = current_week - keep_weeks as i64;

    by_week
        .into_iter()
        .filter(|(week, _)| *week > cutoff)
        .map(|(_, id)| id)
        .collect()
}

fn select_monthly(
    versions: &[&VersionSummary],
    keep_months: usize,
    now: chrono::DateTime<Utc>,
) -> HashSet<VersionId> {
    if keep_months == 0 {
        return HashSet::new();
    }

    let mut by_month: BTreeMap<(i32, u32), VersionId> = BTreeMap::new();
    for v in versions {
        let key = (v.timestamp.year(), v.timestamp.month());
        by_month
            .entry(key)
            .or_insert_with(|| VersionId(v.version_id));
    }

    let (cur_year, cur_month) = (now.year(), now.month());
    let cutoff_months = (cur_year as i64) * 12 + (cur_month as i64 - 1) - keep_months as i64;

    by_month
        .into_iter()
        .filter(|((year, month), _)| {
            (*year as i64) * 12 + (*month as i64 - 1) > cutoff_months
        })
        .map(|(_, id)| id)
        .collect()
}

fn week_number(dt: chrono::DateTime<Utc>) -> i64 {
    let date = dt.date_naive();
    let epoch = NaiveDate::from_ymd_opt(1970, 1, 1).unwrap();
    let ordinal = (date - epoch).num_days();
    ordinal / 7
}

fn compute_smart(
    versions: &[&VersionSummary],
    policy: &RetentionPolicy,
) -> Result<HashSet<VersionId>, RetentionError> {
    let rules = policy
        .smart_rules
        .as_ref()
        .ok_or_else(|| RetentionError::InvalidPolicy("smart_rules not set".into()))?;

    let now = Utc::now();
    let age_cutoff = now - ChronoDuration::days(rules.max_age_days as i64);

    let mut keep: HashSet<VersionId> = HashSet::new();

    for v in versions.iter().take(rules.min_versions as usize) {
        keep.insert(VersionId(v.version_id));
    }

    for v in versions {
        if v.timestamp >= age_cutoff {
            keep.insert(VersionId(v.version_id));
        }
    }

    if rules.prefer_first_and_last_of_day {
        let mut by_day: BTreeMap<NaiveDate, Vec<&VersionSummary>> = BTreeMap::new();
        for v in versions {
            let date = v.timestamp.date_naive();
            by_day.entry(date).or_default().push(v);
        }

        for (_, day_versions) in by_day {
            if let Some(first) = day_versions.last() {
                keep.insert(VersionId(first.version_id));
            }
            if let Some(last) = day_versions.first() {
                keep.insert(VersionId(last.version_id));
            }
        }
    }

    Ok(keep)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use hbx_core::domain::backup::BackupType;
    use hbx_core::domain::common::PolicyId;
    use hbx_core::domain::schedule::{GfsConfig, SmartRules};
    use uuid::Uuid;

    fn make_version(days_ago: i64, number: u64) -> VersionSummary {
        VersionSummary {
            version_id: Uuid::new_v4(),
            version_number: number,
            timestamp: Utc::now() - ChronoDuration::days(days_ago),
            backup_type: BackupType::Full,
            total_size: 1000,
            stored_size: 500,
        }
    }

    fn make_versions(count: usize) -> Vec<VersionSummary> {
        (0..count)
            .map(|i| make_version(i as i64, (count - i) as u64))
            .collect()
    }

    fn make_policy(mode: RetentionMode) -> RetentionPolicy {
        RetentionPolicy {
            policy_id: PolicyId(Uuid::new_v4()),
            mode,
            keep_last_n: None,
            time_based_retention: None,
            gfs_config: None,
            smart_rules: None,
        }
    }

    fn executor() -> RetentionPolicyExecutor {
        RetentionPolicyExecutor
    }

    fn version_ids(versions: &[VersionSummary]) -> HashSet<VersionId> {
        versions.iter().map(|v| VersionId(v.version_id)).collect()
    }

    #[test]
    fn test_keep_all() {
        let versions = make_versions(5);
        let policy = make_policy(RetentionMode::KeepAll);
        let decision = executor().compute(&versions, &policy).unwrap();

        assert_eq!(decision.keep.len(), 5);
        assert_eq!(decision.delete.len(), 0);
    }

    #[test]
    fn test_keep_last_n() {
        let versions = make_versions(10);
        let mut policy = make_policy(RetentionMode::KeepLastN);
        policy.keep_last_n = Some(3);
        let decision = executor().compute(&versions, &policy).unwrap();

        assert_eq!(decision.keep.len(), 3);
        assert_eq!(decision.delete.len(), 7);
    }

    #[test]
    fn test_keep_last_n_more_than_available() {
        let versions = make_versions(3);
        let mut policy = make_policy(RetentionMode::KeepLastN);
        policy.keep_last_n = Some(10);
        let decision = executor().compute(&versions, &policy).unwrap();

        assert_eq!(decision.keep.len(), 3);
        assert_eq!(decision.delete.len(), 0);
    }

    #[test]
    fn test_keep_last_n_zero() {
        let versions = make_versions(5);
        let mut policy = make_policy(RetentionMode::KeepLastN);
        policy.keep_last_n = Some(0);
        let decision = executor().compute(&versions, &policy).unwrap();

        assert_eq!(decision.keep.len(), 0);
        assert_eq!(decision.delete.len(), 5);
    }

    #[test]
    fn test_time_based() {
        let versions = make_versions(30);
        let mut policy = make_policy(RetentionMode::TimeBased);
        policy.time_based_retention = Some(std::time::Duration::from_secs(7 * 86400));
        let decision = executor().compute(&versions, &policy).unwrap();

        assert!(decision.keep.len() <= 8);
        assert!(decision.delete.len() >= 22);
    }

    #[test]
    fn test_time_based_all_within_window() {
        let versions = make_versions(3);
        let mut policy = make_policy(RetentionMode::TimeBased);
        policy.time_based_retention = Some(std::time::Duration::from_secs(365 * 86400));
        let decision = executor().compute(&versions, &policy).unwrap();

        assert_eq!(decision.keep.len(), 3);
        assert_eq!(decision.delete.len(), 0);
    }

    #[test]
    fn test_gfs_basic() {
        let mut versions = Vec::new();
        for day in 0..30 {
            versions.push(make_version(day, (30 - day) as u64));
        }

        let mut policy = make_policy(RetentionMode::Gfs);
        policy.gfs_config = Some(GfsConfig {
            daily: 7,
            weekly: 4,
            monthly: 12,
        });
        let decision = executor().compute(&versions, &policy).unwrap();

        assert!(decision.keep.len() >= 7);
        assert!(decision.delete.len() > 0);

        let all_ids: HashSet<VersionId> = decision.keep.iter().cloned().collect();
        let del_ids: HashSet<VersionId> = decision.delete.iter().cloned().collect();
        assert!(all_ids.is_disjoint(&del_ids));
        assert_eq!(all_ids.len() + del_ids.len(), 30);
    }

    #[test]
    fn test_gfs_7_4_12() {
        let mut versions = Vec::new();
        for day in 0..60 {
            versions.push(make_version(day, (60 - day) as u64));
        }

        let mut policy = make_policy(RetentionMode::Gfs);
        policy.gfs_config = Some(GfsConfig {
            daily: 7,
            weekly: 4,
            monthly: 12,
        });
        let decision = executor().compute(&versions, &policy).unwrap();

        assert!(decision.keep.len() >= 7);

        let all_ids: HashSet<VersionId> = decision.keep.iter().cloned().collect();
        let del_ids: HashSet<VersionId> = decision.delete.iter().cloned().collect();
        assert_eq!(all_ids.len() + del_ids.len(), 60);
    }

    #[test]
    fn test_gfs_zero_keeps() {
        let versions = make_versions(10);
        let mut policy = make_policy(RetentionMode::Gfs);
        policy.gfs_config = Some(GfsConfig {
            daily: 0,
            weekly: 0,
            monthly: 0,
        });
        let decision = executor().compute(&versions, &policy).unwrap();

        assert_eq!(decision.keep.len(), 0);
        assert_eq!(decision.delete.len(), 10);
    }

    #[test]
    fn test_smart_min_versions() {
        let mut versions = Vec::new();
        for day in 0..20 {
            versions.push(make_version(day, (20 - day) as u64));
        }

        let mut policy = make_policy(RetentionMode::Smart);
        policy.smart_rules = Some(SmartRules {
            min_versions: 5,
            max_age_days: 3,
            prefer_first_and_last_of_day: false,
        });
        let decision = executor().compute(&versions, &policy).unwrap();

        assert!(decision.keep.len() >= 5);
    }

    #[test]
    fn test_smart_max_age() {
        let mut versions = Vec::new();
        for day in 0..20 {
            versions.push(make_version(day, (20 - day) as u64));
        }

        let mut policy = make_policy(RetentionMode::Smart);
        policy.smart_rules = Some(SmartRules {
            min_versions: 1,
            max_age_days: 5,
            prefer_first_and_last_of_day: false,
        });
        let decision = executor().compute(&versions, &policy).unwrap();

        assert!(decision.keep.len() <= 6);
        assert!(decision.delete.len() >= 14);
    }

    #[test]
    fn test_smart_prefer_first_and_last() {
        let now = Utc::now();
        let mut versions = Vec::new();
        for hour in 0..24u32 {
            versions.push(VersionSummary {
                version_id: Uuid::new_v4(),
                version_number: hour as u64 + 1,
                timestamp: Utc.from_utc_datetime(
                    &(now - ChronoDuration::hours(1))
                        .date_naive()
                        .and_hms_opt(hour, 0, 0)
                        .unwrap(),
                ),
                backup_type: BackupType::Full,
                total_size: 1000,
                stored_size: 500,
            });
        }

        let mut policy = make_policy(RetentionMode::Smart);
        policy.smart_rules = Some(SmartRules {
            min_versions: 1,
            max_age_days: 365,
            prefer_first_and_last_of_day: true,
        });
        let decision = executor().compute(&versions, &policy).unwrap();

        assert!(decision.keep.len() >= 2);
    }

    #[test]
    fn test_keep_delete_disjoint_union() {
        let versions = make_versions(15);
        let mut policy = make_policy(RetentionMode::KeepLastN);
        policy.keep_last_n = Some(5);
        let decision = executor().compute(&versions, &policy).unwrap();

        let keep_set: HashSet<VersionId> = decision.keep.iter().cloned().collect();
        let delete_set: HashSet<VersionId> = decision.delete.iter().cloned().collect();
        assert!(keep_set.is_disjoint(&delete_set));
        assert_eq!(keep_set.len() + delete_set.len(), 15);
    }

    #[test]
    fn test_empty_versions() {
        let versions: Vec<VersionSummary> = vec![];
        let policy = make_policy(RetentionMode::KeepAll);
        let decision = executor().compute(&versions, &policy).unwrap();

        assert_eq!(decision.keep.len(), 0);
        assert_eq!(decision.delete.len(), 0);
    }

    #[test]
    fn test_missing_keep_last_n_config() {
        let versions = make_versions(5);
        let policy = make_policy(RetentionMode::KeepLastN);
        let result = executor().compute(&versions, &policy);
        assert!(result.is_err());
    }

    #[test]
    fn test_missing_time_based_config() {
        let versions = make_versions(5);
        let policy = make_policy(RetentionMode::TimeBased);
        let result = executor().compute(&versions, &policy);
        assert!(result.is_err());
    }

    #[test]
    fn test_missing_gfs_config() {
        let versions = make_versions(5);
        let policy = make_policy(RetentionMode::Gfs);
        let result = executor().compute(&versions, &policy);
        assert!(result.is_err());
    }

    #[test]
    fn test_missing_smart_config() {
        let versions = make_versions(5);
        let policy = make_policy(RetentionMode::Smart);
        let result = executor().compute(&versions, &policy);
        assert!(result.is_err());
    }

    #[test]
    fn test_keep_all_preserves_all_ids() {
        let versions = make_versions(7);
        let all_ids = version_ids(&versions);
        let policy = make_policy(RetentionMode::KeepAll);
        let decision = executor().compute(&versions, &policy).unwrap();

        let kept: HashSet<VersionId> = decision.keep.iter().cloned().collect();
        assert_eq!(kept, all_ids);
    }
}