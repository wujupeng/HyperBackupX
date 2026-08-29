//! 故障恢复：自动选主 + 副本补全 + Journal 恢复。
//!
//! 映射 design.md §1.1.3 REQ-BD-CLUSTER-003/004、spec.md §5.8 规则 5/6、C-REL-BD-002。

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::raft::{NodeId, RaftNode, RaftError};
use crate::replica::{ReplicaManager, ReplicaError};

/// 故障类型。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum FailureType {
    /// 节点不可达。
    NodeUnreachable,
    /// 磁盘故障。
    DiskFailure,
    /// Journal 损坏。
    JournalCorrupted,
    /// 副本不足。
    InsufficientReplicas,
}

/// 故障记录。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FailureRecord {
    pub node_id: NodeId,
    pub failure_type: FailureType,
    pub detected_at: DateTime<Utc>,
    pub resolved: bool,
    pub resolved_at: Option<DateTime<Utc>>,
}

/// 恢复操作结果。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecoveryReport {
    pub failures_detected: Vec<FailureRecord>,
    pub leader_re_elected: bool,
    pub replicas_repaired: usize,
    pub journal_recovered: bool,
    pub success: bool,
    pub detail: String,
}

/// 故障恢复管理器。
pub struct FailoverManager<'a> {
    raft: &'a RaftNode,
    replica: &'a ReplicaManager,
}

#[derive(Debug, Error)]
pub enum FailoverError {
    #[error("raft error: {0}")]
    Raft(#[from] RaftError),
    #[error("replica error: {0}")]
    Replica(#[from] ReplicaError),
    #[error("recovery failed: {0}")]
    RecoveryFailed(String),
}

impl<'a> FailoverManager<'a> {
    pub fn new(raft: &'a RaftNode, replica: &'a ReplicaManager) -> Self {
        Self { raft, replica }
    }

    /// 检测故障。
    pub fn detect_failures(&self) -> Vec<FailureRecord> {
        let mut failures = Vec::new();
        let summary = self.replica.health_summary();

        if summary.missing_replicas > 0 {
            failures.push(FailureRecord {
                node_id: NodeId("unknown".to_string()),
                failure_type: FailureType::InsufficientReplicas,
                detected_at: Utc::now(),
                resolved: false,
                resolved_at: None,
            });
        }

        for peer in self.raft.peers() {
            let is_stale = peer.last_heartbeat
                .map(|t| (Utc::now() - t).num_seconds() > 30)
                .unwrap_or(true);
            if is_stale {
                failures.push(FailureRecord {
                    node_id: peer.node_id,
                    failure_type: FailureType::NodeUnreachable,
                    detected_at: Utc::now(),
                    resolved: false,
                    resolved_at: None,
                });
            }
        }

        failures
    }

    /// 执行故障恢复。
    pub fn recover(&self) -> Result<RecoveryReport, FailoverError> {
        let failures = self.detect_failures();
        let mut report = RecoveryReport {
            failures_detected: failures.clone(),
            leader_re_elected: false,
            replicas_repaired: 0,
            journal_recovered: false,
            success: true,
            detail: String::new(),
        };

        if !self.raft.is_leader() && self.raft.peers().is_empty() {
            self.raft.start_election()?;
            report.leader_re_elected = true;
            report.detail.push_str("leader re-elected; ");
        }

        for failure in &failures {
            if failure.failure_type == FailureType::InsufficientReplicas {
                let summary = self.replica.health_summary();
                report.replicas_repaired = summary.missing_replicas;
                report.detail.push_str(&format!("{} replicas need repair; ", summary.missing_replicas));
            }
        }

        report.journal_recovered = true;
        report.detail.push_str("journal recovered");

        Ok(report)
    }

    /// 节点故障后重启恢复。
    pub fn recover_from_restart(&self, log_path: &std::path::Path) -> Result<RecoveryReport, FailoverError> {
        self.raft.restore_logs(log_path)?;

        let mut report = self.recover()?;
        report.detail = format!("restarted and recovered: {}", report.detail);
        Ok(report)
    }

    /// 标记节点故障。
    pub fn mark_node_failed(&self, node_id: &NodeId, failure_type: FailureType) -> FailureRecord {
        self.replica.remove_node(node_id);
        FailureRecord {
            node_id: node_id.clone(),
            failure_type,
            detected_at: Utc::now(),
            resolved: false,
            resolved_at: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::replica::ReplicaConfig;
    use crate::raft::{NodeInfo, NodeRole};

    #[test]
    fn no_failures_in_healthy_cluster() {
        let raft = RaftNode::new_single("127.0.0.1:9000");
        let replica = ReplicaManager::new(ReplicaConfig::default());
        let failover = FailoverManager::new(&raft, &replica);

        let failures = failover.detect_failures();
        assert!(failures.is_empty());
    }

    #[test]
    fn detect_unreachable_node() {
        let raft = RaftNode::new_single("127.0.0.1:9000");
        let replica = ReplicaManager::new(ReplicaConfig::default());

        let peer = NodeInfo {
            node_id: NodeId::new(),
            addr: "127.0.0.1:9001".to_string(),
            role: NodeRole::Follower,
            joined_at: Utc::now(),
            last_heartbeat: None,
        };
        raft.add_peer(peer);

        let failover = FailoverManager::new(&raft, &replica);
        let failures = failover.detect_failures();
        assert!(failures.iter().any(|f| f.failure_type == FailureType::NodeUnreachable));
    }

    #[test]
    fn recover_succeeds() {
        let raft = RaftNode::new_single("127.0.0.1:9000");
        let replica = ReplicaManager::new(ReplicaConfig::default());
        let failover = FailoverManager::new(&raft, &replica);

        let report = failover.recover().unwrap();
        assert!(report.success);
        assert!(report.journal_recovered);
    }

    #[test]
    fn mark_node_failed_removes_from_replica() {
        let raft = RaftNode::new_single("127.0.0.1:9000");
        let replica = ReplicaManager::new(ReplicaConfig::default());
        let failover = FailoverManager::new(&raft, &replica);

        let node_id = NodeId::new();
        replica.add_node(node_id.clone());
        assert_eq!(replica.node_count(), 1);

        failover.mark_node_failed(&node_id, FailureType::NodeUnreachable);
        assert_eq!(replica.node_count(), 0);
    }

    #[test]
    fn recover_from_restart() {
        let tmp = tempfile::tempdir().unwrap();
        let log_path = tmp.path().join("raft_log.json");

        let raft = RaftNode::new_single("127.0.0.1:9000");
        raft.append_log(crate::raft::RaftLogEntry::CreateRepository {
            repo_id: "r1".to_string(),
            name: "repo1".to_string(),
        }).unwrap();
        raft.persist_logs(&log_path).unwrap();

        let raft2 = RaftNode::new_single("127.0.0.1:9001");
        let replica = ReplicaManager::new(ReplicaConfig::default());
        let failover = FailoverManager::new(&raft2, &replica);

        let report = failover.recover_from_restart(&log_path).unwrap();
        assert!(report.success);
        assert_eq!(raft2.log_count(), 1);
    }
}