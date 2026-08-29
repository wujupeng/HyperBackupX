//! Raft 集群管理：多节点一致与副本。
//!
//! 模块：
//! - `single_node`: 单节点运行模式（无需 Raft）
//! - `raft`: 简化 Raft 共识（Leader 选举 + 日志复制）
//! - `replica`: Chunk 副本管理（默认 3 副本）
//! - `scale`: 在线扩缩容与数据均衡
//! - `health`: 集群健康检查
//! - `failover`: 故障恢复
//!
//! 映射 spec.md §5.8、design.md §2.4、ADR-BD-008。

pub mod single_node;
pub mod raft;
pub mod replica;
pub mod scale;
pub mod health;
pub mod failover;

pub use single_node::{SingleNodeMode, SingleNodeConfig, SingleNodeError, TlsConfig};
pub use raft::{RaftNode, RaftError, RaftLog, RaftLogEntry, NodeId, NodeInfo, NodeRole};
pub use replica::{ReplicaManager, ReplicaConfig, ReplicaError, ChunkReplicaInfo, ReplicaStatus, ReplicaHealthSummary};
pub use scale::{ScaleManager, ScaleError, ScaleAction, ScaleRecord};
pub use health::{HealthChecker, ClusterHealthReport, NodeHealth, NodeHealthStatus};
pub use failover::{FailoverManager, FailoverError, FailureRecord, FailureType, RecoveryReport};
