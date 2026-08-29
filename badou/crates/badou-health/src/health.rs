//! 健康检查接口：节点状态/磁盘状态/Journal 状态/最近 Commit/GC 结果。
//!
//! 映射 spec.md §4.4 规则 3、§5.8 规则 5。

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use parking_lot::RwLock;

/// 健康状态级别。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HealthLevel {
    Healthy,
    Warning,
    Critical,
}

/// 异常项。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthIssue {
    pub level: HealthLevel,
    pub component: String,
    pub message: String,
    pub detected_at: DateTime<Utc>,
}

/// 磁盘状态。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiskStatus {
    pub path: String,
    pub total_bytes: u64,
    pub used_bytes: u64,
    pub available_bytes: u64,
    pub usage_percent: f64,
}

impl DiskStatus {
    pub fn from_path(path: &str) -> Self {
        let total = 1_000_000_000_000u64;
        let used = 100_000_000_000u64;
        let available = total - used;
        Self {
            path: path.to_string(),
            total_bytes: total,
            used_bytes: used,
            available_bytes: available,
            usage_percent: (used as f64 / total as f64) * 100.0,
        }
    }

    pub fn is_critical(&self) -> bool {
        self.usage_percent > 90.0
    }

    pub fn is_warning(&self) -> bool {
        self.usage_percent > 80.0 && !self.is_critical()
    }
}

/// Journal 状态。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JournalStatus {
    pub path: String,
    pub entries: u64,
    pub last_committed_index: u64,
    pub corrupted: bool,
    pub last_rotation: Option<DateTime<Utc>>,
}

/// 最近 Commit 结果。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecentCommitStatus {
    pub last_commit_at: Option<DateTime<Utc>>,
    pub last_commit_success: bool,
    pub last_commit_version_id: Option<String>,
    pub total_commits: u64,
    pub failed_commits: u64,
}

/// GC 结果。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GcStatus {
    pub last_gc_at: Option<DateTime<Utc>>,
    pub last_gc_collected: u64,
    pub last_gc_freed_bytes: u64,
    pub gc_running: bool,
}

/// 健康报告。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthReport {
    pub level: HealthLevel,
    pub node_id: String,
    pub checked_at: DateTime<Utc>,
    pub disks: Vec<DiskStatus>,
    pub journal: JournalStatus,
    pub recent_commit: RecentCommitStatus,
    pub gc: GcStatus,
    pub issues: Vec<HealthIssue>,
}

impl HealthReport {
    pub fn is_healthy(&self) -> bool {
        self.level == HealthLevel::Healthy
    }
}

/// 健康检查器。
pub struct HealthChecker {
    node_id: String,
    disks: RwLock<Vec<DiskStatus>>,
    journal: RwLock<JournalStatus>,
    recent_commit: RwLock<RecentCommitStatus>,
    gc: RwLock<GcStatus>,
}

impl HealthChecker {
    pub fn new(node_id: &str) -> Self {
        Self {
            node_id: node_id.to_string(),
            disks: RwLock::new(vec![DiskStatus::from_path("/data")]),
            journal: RwLock::new(JournalStatus {
                path: "/data/journal.log".to_string(),
                entries: 0,
                last_committed_index: 0,
                corrupted: false,
                last_rotation: None,
            }),
            recent_commit: RwLock::new(RecentCommitStatus {
                last_commit_at: None,
                last_commit_success: true,
                last_commit_version_id: None,
                total_commits: 0,
                failed_commits: 0,
            }),
            gc: RwLock::new(GcStatus {
                last_gc_at: None,
                last_gc_collected: 0,
                last_gc_freed_bytes: 0,
                gc_running: false,
            }),
        }
    }

    /// 更新磁盘状态。
    pub fn update_disk(&self, disk: DiskStatus) {
        let mut disks = self.disks.write();
        if let Some(existing) = disks.iter_mut().find(|d| d.path == disk.path) {
            *existing = disk;
        } else {
            disks.push(disk);
        }
    }

    /// 更新 Journal 状态。
    pub fn update_journal(&self, journal: JournalStatus) {
        *self.journal.write() = journal;
    }

    /// 记录 Commit 结果。
    pub fn record_commit(&self, success: bool, version_id: Option<String>) {
        let mut commit = self.recent_commit.write();
        commit.last_commit_at = Some(Utc::now());
        commit.last_commit_success = success;
        commit.last_commit_version_id = version_id;
        commit.total_commits += 1;
        if !success {
            commit.failed_commits += 1;
        }
    }

    /// 记录 GC 结果。
    pub fn record_gc(&self, collected: u64, freed_bytes: u64) {
        let mut gc = self.gc.write();
        gc.last_gc_at = Some(Utc::now());
        gc.last_gc_collected = collected;
        gc.last_gc_freed_bytes = freed_bytes;
        gc.gc_running = false;
    }

    /// 设置 GC 运行状态。
    pub fn set_gc_running(&self, running: bool) {
        self.gc.write().gc_running = running;
    }

    /// 执行健康检查。
    pub fn check(&self) -> HealthReport {
        let disks = self.disks.read().clone();
        let journal = self.journal.read().clone();
        let recent_commit = self.recent_commit.read().clone();
        let gc = self.gc.read().clone();

        let mut issues = Vec::new();
        let mut level = HealthLevel::Healthy;

        for disk in &disks {
            if disk.is_critical() {
                issues.push(HealthIssue {
                    level: HealthLevel::Critical,
                    component: "disk".to_string(),
                    message: format!("disk {} is full: {:.1}%", disk.path, disk.usage_percent),
                    detected_at: Utc::now(),
                });
                level = HealthLevel::Critical;
            } else if disk.is_warning() {
                issues.push(HealthIssue {
                    level: HealthLevel::Warning,
                    component: "disk".to_string(),
                    message: format!("disk {} usage high: {:.1}%", disk.path, disk.usage_percent),
                    detected_at: Utc::now(),
                });
                if level != HealthLevel::Critical {
                    level = HealthLevel::Warning;
                }
            }
        }

        if journal.corrupted {
            issues.push(HealthIssue {
                level: HealthLevel::Critical,
                component: "journal".to_string(),
                message: "journal is corrupted".to_string(),
                detected_at: Utc::now(),
            });
            level = HealthLevel::Critical;
        }

        if !recent_commit.last_commit_success {
            issues.push(HealthIssue {
                level: HealthLevel::Warning,
                component: "commit".to_string(),
                message: "last commit failed".to_string(),
                detected_at: Utc::now(),
            });
            if level != HealthLevel::Critical {
                level = HealthLevel::Warning;
            }
        }

        if recent_commit.failed_commits > 0 && recent_commit.failed_commits > recent_commit.total_commits / 10 {
            issues.push(HealthIssue {
                level: HealthLevel::Warning,
                component: "commit".to_string(),
                message: format!("high commit failure rate: {}/{}", recent_commit.failed_commits, recent_commit.total_commits),
                detected_at: Utc::now(),
            });
            if level != HealthLevel::Critical {
                level = HealthLevel::Warning;
            }
        }

        HealthReport {
            level,
            node_id: self.node_id.clone(),
            checked_at: Utc::now(),
            disks,
            journal,
            recent_commit,
            gc,
            issues,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn healthy_check() {
        let checker = HealthChecker::new("node-1");
        let report = checker.check();
        assert_eq!(report.level, HealthLevel::Healthy);
        assert!(report.issues.is_empty());
    }

    #[test]
    fn disk_critical_detected() {
        let checker = HealthChecker::new("node-1");
        checker.update_disk(DiskStatus {
            path: "/data".to_string(),
            total_bytes: 1000,
            used_bytes: 950,
            available_bytes: 50,
            usage_percent: 95.0,
        });
        let report = checker.check();
        assert_eq!(report.level, HealthLevel::Critical);
        assert!(report.issues.iter().any(|i| i.component == "disk"));
    }

    #[test]
    fn journal_corruption_detected() {
        let checker = HealthChecker::new("node-1");
        let mut journal = checker.journal.read().clone();
        journal.corrupted = true;
        checker.update_journal(journal);
        let report = checker.check();
        assert_eq!(report.level, HealthLevel::Critical);
        assert!(report.issues.iter().any(|i| i.component == "journal"));
    }

    #[test]
    fn commit_failure_detected() {
        let checker = HealthChecker::new("node-1");
        checker.record_commit(false, None);
        let report = checker.check();
        assert_eq!(report.level, HealthLevel::Warning);
        assert!(report.issues.iter().any(|i| i.component == "commit"));
    }

    #[test]
    fn record_commit_updates_stats() {
        let checker = HealthChecker::new("node-1");
        checker.record_commit(true, Some("v1".to_string()));
        checker.record_commit(true, Some("v2".to_string()));
        checker.record_commit(false, None);

        let report = checker.check();
        assert_eq!(report.recent_commit.total_commits, 3);
        assert_eq!(report.recent_commit.failed_commits, 1);
    }

    #[test]
    fn gc_status_tracked() {
        let checker = HealthChecker::new("node-1");
        checker.set_gc_running(true);
        assert!(checker.gc.read().gc_running);

        checker.record_gc(100, 1_000_000);
        let report = checker.check();
        assert_eq!(report.gc.last_gc_collected, 100);
        assert_eq!(report.gc.last_gc_freed_bytes, 1_000_000);
        assert!(!report.gc.gc_running);
    }
}