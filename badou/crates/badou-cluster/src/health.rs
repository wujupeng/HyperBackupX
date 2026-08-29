//! 集群健康检查。
//!
//! 映射 design.md §1.1.3 REQ-BD-CLUSTER-003、spec.md §5.8 规则 5、§4.4 规则 3。

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::raft::{NodeId, NodeRole, RaftNode};
use crate::replica::{ReplicaManager, ReplicaHealthSummary};

/// 节点健康状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NodeHealthStatus {
    Healthy,
    Degraded,
    Unreachable,
}

/// 单节点健康信息。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeHealth {
    pub node_id: NodeId,
    pub addr: String,
    pub role: NodeRole,
    pub status: NodeHealthStatus,
    pub disk_usage_percent: f64,
    pub journal_ok: bool,
    pub last_heartbeat: Option<DateTime<Utc>>,
}

/// 集群健康报告。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusterHealthReport {
    pub cluster_healthy: bool,
    pub leader_id: Option<NodeId>,
    pub term: u64,
    pub cluster_size: usize,
    pub nodes: Vec<NodeHealth>,
    pub replica_summary: ReplicaHealthSummary,
    pub checked_at: DateTime<Utc>,
}

/// 健康检查器。
pub struct HealthChecker<'a> {
    raft: &'a RaftNode,
    replica: &'a ReplicaManager,
}

impl<'a> HealthChecker<'a> {
    pub fn new(raft: &'a RaftNode, replica: &'a ReplicaManager) -> Self {
        Self { raft, replica }
    }

    /// 执行集群健康检查。
    pub fn check(&self) -> ClusterHealthReport {
        let leader_id = self.raft.leader_id();
        let term = self.raft.term();
        let cluster_size = self.raft.cluster_size();

        let mut nodes = Vec::new();

        // 自身节点
        nodes.push(NodeHealth {
            node_id: self.raft.node_id().clone(),
            addr: self.raft.addr().to_string(),
            role: self.raft.role(),
            status: NodeHealthStatus::Healthy,
            disk_usage_percent: 0.0,
            journal_ok: true,
            last_heartbeat: Some(Utc::now()),
        });

        // peer 节点
        for peer in self.raft.peers() {
            let status = if peer.last_heartbeat.map(|t| (Utc::now() - t).num_seconds() < 30).unwrap_or(false) {
                NodeHealthStatus::Healthy
            } else {
                NodeHealthStatus::Degraded
            };

            nodes.push(NodeHealth {
                node_id: peer.node_id,
                addr: peer.addr,
                role: peer.role,
                status,
                disk_usage_percent: 0.0,
                journal_ok: true,
                last_heartbeat: peer.last_heartbeat,
            });
        }

        let replica_summary = self.replica.health_summary();
        let cluster_healthy = nodes.iter().all(|n| n.status != NodeHealthStatus::Unreachable)
            && replica_summary.unhealthy_chunks == 0;

        ClusterHealthReport {
            cluster_healthy,
            leader_id,
            term,
            cluster_size,
            nodes,
            replica_summary,
            checked_at: Utc::now(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::replica::ReplicaConfig;
    use crate::raft::NodeInfo;

    #[test]
    fn healthy_cluster() {
        let raft = RaftNode::new_single("127.0.0.1:9000");
        let replica = ReplicaManager::new(ReplicaConfig::default());
        let checker = HealthChecker::new(&raft, &replica);

        let report = checker.check();
        assert!(report.cluster_healthy);
        assert_eq!(report.cluster_size, 1);
        assert!(report.leader_id.is_some());
    }

    #[test]
    fn cluster_with_peers() {
        let raft = RaftNode::new_single("127.0.0.1:9000");
        let replica = ReplicaManager::new(ReplicaConfig::default());

        let peer = NodeInfo {
            node_id: NodeId::new(),
            addr: "127.0.0.1:9001".to_string(),
            role: NodeRole::Follower,
            joined_at: Utc::now(),
            last_heartbeat: Some(Utc::now()),
        };
        raft.add_peer(peer);

        let checker = HealthChecker::new(&raft, &replica);
        let report = checker.check();
        assert_eq!(report.cluster_size, 2);
        assert_eq!(report.nodes.len(), 2);
    }

    #[test]
    fn degraded_node_detected() {
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

        let checker = HealthChecker::new(&raft, &replica);
        let report = checker.check();
        assert!(report.nodes.iter().any(|n| n.status == NodeHealthStatus::Degraded));
    }
}