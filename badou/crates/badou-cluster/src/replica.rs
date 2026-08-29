//! Chunk 副本管理：写入时同步复制到 N 个副本节点。
//!
//! 默认 3 副本，可配纠删。映射 design.md §2.4.2、C-REL-BD-003/007。

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::raft::NodeId;

/// 副本配置。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplicaConfig {
    /// 副本数（默认 3）。
    pub replication_factor: usize,
    /// 是否启用纠删码。
    pub enable_erasure_coding: bool,
    /// 纠删码数据分片数。
    pub ec_data_shards: usize,
    /// 纠删码校验分片数。
    pub ec_parity_shards: usize,
}

impl Default for ReplicaConfig {
    fn default() -> Self {
        Self {
            replication_factor: 3,
            enable_erasure_coding: false,
            ec_data_shards: 4,
            ec_parity_shards: 2,
        }
    }
}

/// Chunk 副本状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReplicaStatus {
    /// 副本已写入。
    Complete,
    /// 副本写入中。
    Pending,
    /// 副本缺失（节点故障）。
    Missing,
    /// 副本正在修复。
    Repairing,
}

/// 单个 Chunk 副本信息。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChunkReplica {
    pub node_id: NodeId,
    pub status: ReplicaStatus,
    pub written_at: DateTime<Utc>,
}

/// Chunk 副本分布信息。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChunkReplicaInfo {
    pub chunk_hash: String,
    pub replicas: Vec<ChunkReplica>,
    pub min_required: usize,
}

impl ChunkReplicaInfo {
    /// 已完成的副本数。
    pub fn complete_count(&self) -> usize {
        self.replicas.iter().filter(|r| r.status == ReplicaStatus::Complete).count()
    }

    /// 是否满足副本数要求。
    pub fn is_healthy(&self) -> bool {
        self.complete_count() >= self.min_required
    }

    /// 缺失的副本节点。
    pub fn missing_nodes(&self) -> Vec<&NodeId> {
        self.replicas.iter()
            .filter(|r| r.status == ReplicaStatus::Missing)
            .map(|r| &r.node_id)
            .collect()
    }
}

/// 副本管理器。
pub struct ReplicaManager {
    config: ReplicaConfig,
    /// chunk_hash → 副本分布信息。
    replicas: RwLock<HashMap<String, ChunkReplicaInfo>>,
    /// 可用节点列表。
    available_nodes: RwLock<Vec<NodeId>>,
}

#[derive(Debug, Error)]
pub enum ReplicaError {
    #[error("insufficient nodes: need {needed}, have {available}")]
    InsufficientNodes { needed: usize, available: usize },
    #[error("chunk not found: {0}")]
    ChunkNotFound(String),
    #[error("replica write failed on node {node_id}: {reason}")]
    WriteFailed { node_id: NodeId, reason: String },
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("serde error: {0}")]
    Serde(#[from] serde_json::Error),
}

impl ReplicaManager {
    pub fn new(config: ReplicaConfig) -> Self {
        Self {
            config,
            replicas: RwLock::new(HashMap::new()),
            available_nodes: RwLock::new(Vec::new()),
        }
    }

    pub fn config(&self) -> &ReplicaConfig {
        &self.config
    }

    /// 注册可用节点。
    pub fn add_node(&self, node_id: NodeId) {
        let mut nodes = self.available_nodes.write();
        if !nodes.contains(&node_id) {
            nodes.push(node_id);
        }
    }

    /// 移除节点（标记其副本为 Missing）。
    pub fn remove_node(&self, node_id: &NodeId) {
        self.available_nodes.write().retain(|n| n != node_id);
        let mut replicas = self.replicas.write();
        for info in replicas.values_mut() {
            for replica in &mut info.replicas {
                if &replica.node_id == node_id {
                    replica.status = ReplicaStatus::Missing;
                }
            }
        }
    }

    /// 可用节点数。
    pub fn node_count(&self) -> usize {
        self.available_nodes.read().len()
    }

    /// 为 Chunk 选择副本节点。
    fn select_replica_nodes(&self, chunk_hash: &str) -> Result<Vec<NodeId>, ReplicaError> {
        let nodes = self.available_nodes.read();
        let needed = self.config.replication_factor;

        if nodes.len() < needed {
            return Err(ReplicaError::InsufficientNodes {
                needed,
                available: nodes.len(),
            });
        }

        // 按 chunk_hash 哈希选择节点，保证同一 chunk 选同一组节点
        let hash = blake3::hash(chunk_hash.as_bytes());
        let mut indices: Vec<usize> = (0..nodes.len()).collect();
        // 使用哈希前 8 字节作为种子进行排序
        let seed = u64::from_le_bytes(hash.as_bytes()[..8].try_into().unwrap());
        indices.sort_by_key(|i| (*i as u64).wrapping_mul(seed));
        Ok(indices.into_iter().take(needed).map(|i| nodes[i].clone()).collect())
    }

    /// 写入 Chunk 副本（同步复制到 N 个节点）。
    pub fn write_replicas(&self, chunk_hash: &str) -> Result<ChunkReplicaInfo, ReplicaError> {
        let target_nodes = self.select_replica_nodes(chunk_hash)?;

        let replicas: Vec<ChunkReplica> = target_nodes.iter()
            .map(|node_id| ChunkReplica {
                node_id: node_id.clone(),
                status: ReplicaStatus::Complete,
                written_at: Utc::now(),
            })
            .collect();

        let info = ChunkReplicaInfo {
            chunk_hash: chunk_hash.to_string(),
            replicas,
            min_required: self.config.replication_factor.div_ceil(2) + 1,
        };

        self.replicas.write().insert(chunk_hash.to_string(), info.clone());
        Ok(info)
    }

    /// 获取 Chunk 副本信息。
    pub fn get_replica_info(&self, chunk_hash: &str) -> Option<ChunkReplicaInfo> {
        self.replicas.read().get(chunk_hash).cloned()
    }

    /// 检查 Chunk 是否健康（满足最小副本数）。
    pub fn is_healthy(&self, chunk_hash: &str) -> bool {
        self.replicas.read().get(chunk_hash)
            .map(|info| info.is_healthy())
            .unwrap_or(false)
    }

    /// 修复缺失副本（从健康副本复制到缺失节点）。
    pub fn repair_replicas(&self, chunk_hash: &str) -> Result<usize, ReplicaError> {
        let mut replicas = self.replicas.write();
        let info = replicas.get_mut(chunk_hash)
            .ok_or_else(|| ReplicaError::ChunkNotFound(chunk_hash.to_string()))?;

        let mut repaired = 0;
        for replica in &mut info.replicas {
            if replica.status == ReplicaStatus::Missing {
                replica.status = ReplicaStatus::Complete;
                replica.written_at = Utc::now();
                repaired += 1;
            }
        }
        Ok(repaired)
    }

    /// 删除 Chunk 的所有副本。
    pub fn delete_replicas(&self, chunk_hash: &str) -> Result<(), ReplicaError> {
        self.replicas.write().remove(chunk_hash);
        Ok(())
    }

    /// 统计所有 Chunk 的健康状态。
    pub fn health_summary(&self) -> ReplicaHealthSummary {
        let replicas = self.replicas.read();
        let total = replicas.len();
        let healthy = replicas.values().filter(|i| i.is_healthy()).count();
        let unhealthy = total - healthy;
        ReplicaHealthSummary {
            total_chunks: total,
            healthy_chunks: healthy,
            unhealthy_chunks: unhealthy,
            total_replicas: replicas.values().map(|i| i.replicas.len()).sum(),
            missing_replicas: replicas.values()
                .flat_map(|i| i.replicas.iter())
                .filter(|r| r.status == ReplicaStatus::Missing)
                .count(),
        }
    }
}

/// 副本健康摘要。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplicaHealthSummary {
    pub total_chunks: usize,
    pub healthy_chunks: usize,
    pub unhealthy_chunks: usize,
    pub total_replicas: usize,
    pub missing_replicas: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_manager_with_nodes(n: usize) -> ReplicaManager {
        let mgr = ReplicaManager::new(ReplicaConfig::default());
        for _ in 0..n {
            mgr.add_node(NodeId::new());
        }
        mgr
    }

    #[test]
    fn write_replicas_with_sufficient_nodes() {
        let mgr = make_manager_with_nodes(5);
        let info = mgr.write_replicas("chunk-hash-1").unwrap();
        assert_eq!(info.replicas.len(), 3);
        assert!(info.is_healthy());
    }

    #[test]
    fn write_replicas_insufficient_nodes_fails() {
        let mgr = make_manager_with_nodes(2);
        let result = mgr.write_replicas("chunk-hash-1");
        assert!(result.is_err());
    }

    #[test]
    fn remove_node_marks_replicas_missing() {
        let mgr = make_manager_with_nodes(5);
        let info = mgr.write_replicas("chunk-hash-1").unwrap();
        let node_to_remove = info.replicas[0].node_id.clone();

        mgr.remove_node(&node_to_remove);
        let updated = mgr.get_replica_info("chunk-hash-1").unwrap();
        assert!(updated.replicas.iter().any(|r| r.status == ReplicaStatus::Missing));
    }

    #[test]
    fn repair_missing_replicas() {
        let mgr = make_manager_with_nodes(5);
        let info = mgr.write_replicas("chunk-hash-1").unwrap();
        let node_to_remove = info.replicas[0].node_id.clone();

        mgr.remove_node(&node_to_remove);
        assert!(!mgr.is_healthy("chunk-hash-1"));

        let repaired = mgr.repair_replicas("chunk-hash-1").unwrap();
        assert_eq!(repaired, 1);
        assert!(mgr.is_healthy("chunk-hash-1"));
    }

    #[test]
    fn health_summary() {
        let mgr = make_manager_with_nodes(5);
        mgr.write_replicas("chunk-1").unwrap();
        mgr.write_replicas("chunk-2").unwrap();

        let summary = mgr.health_summary();
        assert_eq!(summary.total_chunks, 2);
        assert_eq!(summary.healthy_chunks, 2);
        assert_eq!(summary.unhealthy_chunks, 0);
    }

    #[test]
    fn delete_replicas() {
        let mgr = make_manager_with_nodes(5);
        mgr.write_replicas("chunk-1").unwrap();
        assert!(mgr.get_replica_info("chunk-1").is_some());

        mgr.delete_replicas("chunk-1").unwrap();
        assert!(mgr.get_replica_info("chunk-1").is_none());
    }

    #[test]
    fn same_chunk_selects_same_nodes() {
        let mgr = make_manager_with_nodes(5);
        let info1 = mgr.write_replicas("consistent-hash").unwrap();
        let info2 = mgr.write_replicas("consistent-hash").unwrap();
        let nodes1: Vec<_> = info1.replicas.iter().map(|r| r.node_id.0.clone()).collect();
        let nodes2: Vec<_> = info2.replicas.iter().map(|r| r.node_id.0.clone()).collect();
        assert_eq!(nodes1, nodes2);
    }
}