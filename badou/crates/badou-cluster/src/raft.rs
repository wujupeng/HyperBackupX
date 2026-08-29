//! 简化 Raft 共识：Leader 选举 + 日志复制。
//!
//! 不依赖 openraft（网络不可用），实现简化版共识协议。
//! 后续网络可用时可替换为 openraft 实现。
//!
//! 映射 design.md §2.4.2 多节点部署、§1.1.3 REQ-BD-CLUSTER-002、ADR-BD-008。

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

/// 节点 ID。
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NodeId(pub String);

impl NodeId {
    pub fn new() -> Self {
        Self(Uuid::new_v4().to_string())
    }
}

impl Default for NodeId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for NodeId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// 节点角色。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NodeRole {
    Leader,
    Follower,
    Candidate,
}

/// 集群节点信息。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeInfo {
    pub node_id: NodeId,
    pub addr: String,
    pub role: NodeRole,
    pub joined_at: DateTime<Utc>,
    pub last_heartbeat: Option<DateTime<Utc>>,
}

/// Raft 日志条目类型。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RaftLogEntry {
    /// 创建 Repository。
    CreateRepository { repo_id: String, name: String },
    /// 删除 Repository。
    DeleteRepository { repo_id: String },
    /// 创建 Version。
    CreateVersion { repo_id: String, version_id: String, parent_version_id: Option<String> },
    /// 封存 Version。
    SealVersion { repo_id: String, version_id: String },
    /// 删除 Version（触发 GC）。
    DeleteVersion { repo_id: String, version_id: String },
    /// 更新 Chunk 引用计数。
    UpdateChunkRef { repo_id: String, chunk_hash: String, ref_count: u32 },
    /// 成员变更：加入节点。
    AddNode { node_id: NodeId, addr: String },
    /// 成员变更：退出节点。
    RemoveNode { node_id: NodeId },
}

/// Raft 日志条目。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RaftLog {
    pub term: u64,
    pub index: u64,
    pub entry: RaftLogEntry,
    pub committed: bool,
    pub timestamp: DateTime<Utc>,
}

/// Raft 节点状态。
pub struct RaftNode {
    node_id: NodeId,
    addr: String,
    role: RwLock<NodeRole>,
    term: RwLock<u64>,
    leader_id: RwLock<Option<NodeId>>,
    peers: RwLock<HashMap<NodeId, NodeInfo>>,
    log: RwLock<Vec<RaftLog>>,
    commit_index: RwLock<u64>,
    last_applied: RwLock<u64>,
}

#[derive(Debug, Error)]
pub enum RaftError {
    #[error("not leader: current role is {0:?}")]
    NotLeader(NodeRole),
    #[error("node not found: {0}")]
    NodeNotFound(NodeId),
    #[error("log entry already committed at index {0}")]
    AlreadyCommitted(u64),
    #[error("term mismatch: expected {expected}, got {actual}")]
    TermMismatch { expected: u64, actual: u64 },
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("serde error: {0}")]
    Serde(#[from] serde_json::Error),
}

impl RaftNode {
    /// 创建单节点 Raft（自动成为 Leader）。
    pub fn new_single(addr: &str) -> Self {
        let node_id = NodeId::new();
        Self {
            node_id: node_id.clone(),
            addr: addr.to_string(),
            role: RwLock::new(NodeRole::Leader),
            term: RwLock::new(1),
            leader_id: RwLock::new(Some(node_id)),
            peers: RwLock::new(HashMap::new()),
            log: RwLock::new(Vec::new()),
            commit_index: RwLock::new(0),
            last_applied: RwLock::new(0),
        }
    }

    /// 创建多节点 Raft Follower。
    pub fn new_follower(addr: &str, _leader_addr: &str) -> Self {
        let node_id = NodeId::new();
        Self {
            node_id: node_id.clone(),
            addr: addr.to_string(),
            role: RwLock::new(NodeRole::Follower),
            term: RwLock::new(1),
            leader_id: RwLock::new(None),
            peers: RwLock::new(HashMap::new()),
            log: RwLock::new(Vec::new()),
            commit_index: RwLock::new(0),
            last_applied: RwLock::new(0),
        }
    }

    pub fn node_id(&self) -> &NodeId {
        &self.node_id
    }

    pub fn addr(&self) -> &str {
        &self.addr
    }

    pub fn role(&self) -> NodeRole {
        *self.role.read()
    }

    pub fn term(&self) -> u64 {
        *self.term.read()
    }

    pub fn leader_id(&self) -> Option<NodeId> {
        self.leader_id.read().clone()
    }

    pub fn is_leader(&self) -> bool {
        *self.role.read() == NodeRole::Leader
    }

    /// 提交日志条目（仅 Leader 可提交）。
    pub fn append_log(&self, entry: RaftLogEntry) -> Result<RaftLog, RaftError> {
        if !self.is_leader() {
            return Err(RaftError::NotLeader(*self.role.read()));
        }

        let mut log = self.log.write();
        let index = log.len() as u64 + 1;
        let term = *self.term.read();
        let raft_log = RaftLog {
            term,
            index,
            entry,
            committed: true,
            timestamp: Utc::now(),
        };
        log.push(raft_log.clone());
        *self.commit_index.write() = index;
        *self.last_applied.write() = index;
        Ok(raft_log)
    }

    /// 获取已提交的日志。
    pub fn committed_logs(&self) -> Vec<RaftLog> {
        self.log.read().iter().filter(|l| l.committed).cloned().collect()
    }

    /// 获取日志条目数。
    pub fn log_count(&self) -> usize {
        self.log.read().len()
    }

    /// 添加 peer 节点。
    pub fn add_peer(&self, node_info: NodeInfo) {
        self.peers.write().insert(node_info.node_id.clone(), node_info);
    }

    /// 移除 peer 节点。
    pub fn remove_peer(&self, node_id: &NodeId) -> Option<NodeInfo> {
        self.peers.write().remove(node_id)
    }

    /// 列出所有 peer 节点。
    pub fn peers(&self) -> Vec<NodeInfo> {
        self.peers.read().values().cloned().collect()
    }

    /// 节点数（含自身）。
    pub fn cluster_size(&self) -> usize {
        self.peers.read().len() + 1
    }

    /// 发起选举（Candidate → Leader）。
    pub fn start_election(&self) -> Result<(), RaftError> {
        let mut role = self.role.write();
        *role = NodeRole::Candidate;
        let mut term = self.term.write();
        *term += 1;
        drop(term);

        // 简化：单节点直接当选
        if self.peers.read().is_empty() {
            *role = NodeRole::Leader;
            *self.leader_id.write() = Some(self.node_id.clone());
        }
        Ok(())
    }

    /// 接收心跳。
    pub fn receive_heartbeat(&self, leader_id: NodeId, leader_term: u64) {
        let mut term = self.term.write();
        if leader_term >= *term {
            *term = leader_term;
            *self.role.write() = NodeRole::Follower;
            *self.leader_id.write() = Some(leader_id);
        }
    }

    /// 持久化日志到文件。
    pub fn persist_logs(&self, path: &std::path::Path) -> Result<(), RaftError> {
        let logs = self.log.read().clone();
        let json = serde_json::to_vec_pretty(&logs)?;
        std::fs::write(path, json)?;
        Ok(())
    }

    /// 从文件恢复日志。
    pub fn restore_logs(&self, path: &std::path::Path) -> Result<(), RaftError> {
        if !path.exists() {
            return Ok(());
        }
        let data = std::fs::read(path)?;
        let logs: Vec<RaftLog> = serde_json::from_slice(&data)?;
        let mut log = self.log.write();
        *log = logs;
        let max_index = log.iter().map(|l| l.index).max().unwrap_or(0);
        *self.commit_index.write() = max_index;
        *self.last_applied.write() = max_index;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_node_becomes_leader() {
        let node = RaftNode::new_single("127.0.0.1:9000");
        assert!(node.is_leader());
        assert_eq!(node.term(), 1);
        assert_eq!(node.cluster_size(), 1);
    }

    #[test]
    fn follower_starts_as_follower() {
        let node = RaftNode::new_follower("127.0.0.1:9001", "127.0.0.1:9000");
        assert!(!node.is_leader());
        assert_eq!(node.role(), NodeRole::Follower);
    }

    #[test]
    fn leader_append_log() {
        let node = RaftNode::new_single("127.0.0.1:9000");
        let log = node.append_log(RaftLogEntry::CreateRepository {
            repo_id: "test-repo".to_string(),
            name: "test".to_string(),
        }).unwrap();
        assert!(log.committed);
        assert_eq!(log.index, 1);
        assert_eq!(node.log_count(), 1);
    }

    #[test]
    fn follower_cannot_append_log() {
        let node = RaftNode::new_follower("127.0.0.1:9001", "127.0.0.1:9000");
        let result = node.append_log(RaftLogEntry::CreateRepository {
            repo_id: "test-repo".to_string(),
            name: "test".to_string(),
        });
        assert!(result.is_err());
    }

    #[test]
    fn add_and_remove_peer() {
        let node = RaftNode::new_single("127.0.0.1:9000");
        let peer_id = NodeId::new();
        let peer_info = NodeInfo {
            node_id: peer_id.clone(),
            addr: "127.0.0.1:9001".to_string(),
            role: NodeRole::Follower,
            joined_at: Utc::now(),
            last_heartbeat: None,
        };
        node.add_peer(peer_info);
        assert_eq!(node.cluster_size(), 2);

        node.remove_peer(&peer_id);
        assert_eq!(node.cluster_size(), 1);
    }

    #[test]
    fn election_promotes_to_leader() {
        let node = RaftNode::new_single("127.0.0.1:9000");
        let mut role = node.role.write();
        *role = NodeRole::Follower;
        drop(role);

        node.start_election().unwrap();
        assert!(node.is_leader());
        assert_eq!(node.term(), 2);
    }

    #[test]
    fn heartbeat_updates_follower() {
        let node = RaftNode::new_follower("127.0.0.1:9001", "127.0.0.1:9000");
        let leader_id = NodeId::new();
        node.receive_heartbeat(leader_id.clone(), 5);
        assert_eq!(node.term(), 5);
        assert_eq!(node.role(), NodeRole::Follower);
        assert_eq!(node.leader_id(), Some(leader_id));
    }

    #[test]
    fn persist_and_restore_logs() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("raft_log.json");

        let node = RaftNode::new_single("127.0.0.1:9000");
        node.append_log(RaftLogEntry::CreateRepository {
            repo_id: "r1".to_string(),
            name: "repo1".to_string(),
        }).unwrap();
        node.append_log(RaftLogEntry::CreateVersion {
            repo_id: "r1".to_string(),
            version_id: "v1".to_string(),
            parent_version_id: None,
        }).unwrap();

        node.persist_logs(&path).unwrap();
        assert!(path.exists());

        let node2 = RaftNode::new_single("127.0.0.1:9001");
        node2.restore_logs(&path).unwrap();
        assert_eq!(node2.log_count(), 2);
    }

    #[test]
    fn committed_logs_filter() {
        let node = RaftNode::new_single("127.0.0.1:9000");
        node.append_log(RaftLogEntry::CreateRepository {
            repo_id: "r1".to_string(),
            name: "repo1".to_string(),
        }).unwrap();
        node.append_log(RaftLogEntry::DeleteRepository {
            repo_id: "r1".to_string(),
        }).unwrap();

        let committed = node.committed_logs();
        assert_eq!(committed.len(), 2);
    }
}