# 八斗存储桶 (BaDou Backup Bucket)

HyperBackup X 原生备份存储层。Backup-Native Storage Bucket，专为 HyperBackup X 的数据模型优化。

## 定位

- **HyperBackup X** = 备份系统（备份、恢复、调度、策略、加密、压缩、去重、版本、客户端）
- **八斗存储桶** = HyperBackup X 原生备份存储层（持久化、索引、快照、完整性、GC、不可变、故障恢复）
- **HBOP** = 两者之间的原生协议（HyperBackup Object Protocol）

## 核心对象

Repository / Chunk / Manifest / Snapshot / Version / Index / Journal

## 架构

```
HyperBackup X
      │
     HBOP
      │
      ▼
  八斗存储桶
      │
      ▼
 Debian 13 Cluster
```

## Crate 结构

| Crate | 职责 |
|-------|------|
| badou-proto | HBOP Protobuf 协议定义与生成代码 |
| badou-hbop-server | HBOP gRPC Server |
| badou-hbop-client | HBOP gRPC Client |
| badou-engine | 核心引擎，七种对象编排 |
| badou-store | 存储引擎层，Chunk/Manifest/Snapshot 持久化 |
| badou-index | LMDB 索引，chunk_hash → location/ref_count |
| badou-journal | append-only Journal，崩溃恢复 |
| badou-state | 状态机，PostgreSQL 元数据 |
| badou-gc | 引用计数 GC |
| badou-verify | 三级完整性校验 |
| badou-recovery | Snapshot 路径流式恢复 |
| badou-cluster | Raft 集群管理 |
| badou-health | 健康检查 + Prometheus 指标 |
| badou-server | 主二进制 |
| badou-cli | 运维 CLI |

## 构建

```bash
cargo build --workspace
cargo clippy --workspace -- -D warnings
cargo test --workspace
```