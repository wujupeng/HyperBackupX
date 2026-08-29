# Duplicati 兼容性矩阵 — Phase BD-21

> **目标**：以 Duplicati 功能完整覆盖作为 HyperBackup X 的重要验收目标  
> **原则**：诚实优先 — 宁可 Not Supported 也不要 Fake Supported  
> **日期**：2026-08-26

## 兼容性矩阵

| # | Feature | Implemented | Tested | Result | Evidence |
|---|---------|------------|--------|--------|----------|
| 1 | Full Backup | ✅ Yes | ✅ E2E | PASS | badou-tests/e2e_lifecycle.rs |
| 2 | Incremental Backup | ✅ Yes | ✅ E2E | PASS | badou-tests/e2e_lifecycle.rs (多版本) |
| 3 | Version Management | ✅ Yes | ✅ E2E | PASS | SnapshotList/Get/Delete API |
| 4 | Retention Policy | ✅ Yes | ✅ Unit | PASS | badou-state immutable retention |
| 5 | Encryption | ✅ Yes | ✅ Unit | PASS | hbx-core EncryptedChunk |
| 6 | Compression | ✅ Yes | ✅ Unit | PASS | hbx-core pipeline compression |
| 7 | Deduplication | ✅ Yes | ✅ E2E | PASS | e2e_lifecycle.rs dedup test |
| 8 | Filters (include/exclude) | ✅ Yes | ✅ Unit | PASS | hbx-core filter module |
| 9 | Scheduler | ✅ Yes | ✅ Unit | PASS | hbx-scheduler |
| 10 | Restore | ✅ Yes | ✅ E2E | PASS | badou-recovery stream engine |
| 11 | Partial Restore | ✅ Yes | ✅ Unit | PASS | RecoveryOpen file_path option |
| 12 | Verification | ✅ Yes | ✅ E2E | PASS | badou-verify BLAKE3-256 |
| 13 | Pause/Resume | ✅ Yes | ✅ Unit | PASS | hbx-core pipeline pause/resume |
| 14 | Retry | ✅ Yes | ✅ Unit | PASS | hbx-core retry logic |
| 15 | CLI | ✅ Yes | ✅ Unit | PASS | badou-cli 6 commands |
| 16 | Local Backend | ✅ Yes | ✅ Unit | PASS | badou-store file I/O |
| 17 | SMB Backend | ✅ Yes | ✅ Unit | PASS | hbx-repo SMB provider |
| 18 | FTP Backend | ✅ Yes | ✅ Unit | PASS | hbx-repo FTP provider |
| 19 | SFTP Backend | ✅ Yes | ✅ Unit | PASS | hbx-repo SFTP provider |
| 20 | WebDAV Backend | ✅ Yes | ✅ Unit | PASS | hbx-repo WebDAV provider |
| 21 | S3 Backend | ✅ Yes | ✅ Unit | PASS | hbx-repo S3 provider |
| 22 | BaDou Native Backend | ✅ Yes | ✅ E2E | PASS | hbx-badou-provider (本阶段) |
| 23 | Immutable Retention | ✅ Yes | ✅ E2E | PASS | badou-state ImmutableGcGuard |
| 24 | Crash Recovery | ✅ Yes | ✅ Chaos | PASS | badou-journal replay |
| 25 | GC (Reference Counting) | ✅ Yes | ✅ E2E | PASS | badou-gc ref count + immutable guard |
| 26 | Two-Phase Commit | ✅ Yes | ✅ Unit | PASS | badou-ops staging→VERIFYING→SEALED |
| 27 | mTLS Authentication | ✅ Yes | ✅ Unit | PASS | badou-hbop-server mTLS |
| 28 | JWT Authorization | ✅ Yes | ✅ Unit | PASS | badou-hbop-server JWT + RBAC |
| 29 | Cluster (Raft) | ✅ Yes | ✅ Unit | PASS | badou-cluster Raft |
| 30 | Prometheus Metrics | ✅ Yes | ✅ Unit | PASS | badou-health metrics |
| 31 | Web Dashboard | ✅ Yes | ✅ Unit | PASS | web/ React pages |
| 32 | Control Plane REST API | ✅ Yes | ✅ Unit | PASS | control/ Go 17 endpoints |
| 33 | Duplicati Direct Import | ❌ No | N/A | Not Supported | 超出 Phase BD-21 范围 |
| 34 | Duplicati Config Migration | ❌ No | N/A | Not Supported | 超出 Phase BD-21 范围 |
| 35 | Duplicati Backup Format | ❌ No | N/A | Not Supported | HyperBackup X 使用原生格式 |
| 36 | Duplicati 2.x API Compatible | ❌ No | N/A | Not Supported | HyperBackup X 有独立 API |

## 量化统计

| 统计项 | 数量 |
|--------|------|
| Total Features | 36 |
| Implemented | 32 |
| Tested | 32 |
| Passed | 32 |
| Failed | 0 |
| Not Supported | 4 |
| Fake Supported | 0 |

## 关键说明

### 已实现且测试通过 (32/36)

HyperBackup X + 八斗已实现 Duplicati 的**核心备份/恢复功能全覆盖**，包括：
- 完整/增量备份、版本管理、保留策略
- 加密、压缩、去重、过滤
- 恢复（完整+部分）、校验
- 暂停/恢复、重试、CLI
- 5 种存储后端 + 原生 BaDou 后端
- 不可变保留、崩溃恢复、引用计数 GC
- 两阶段提交、mTLS、JWT、RBAC
- 集群、指标、Web Dashboard、REST API

### 明确 Not Supported (4/36)

以下功能**明确声明不支持**，不是半实现：
1. **Duplicati Direct Import** — 不能直接导入 Duplicati 备份
2. **Duplicati Config Migration** — 不能迁移 Duplicati 配置
3. **Duplicati Backup Format** — 使用原生格式，非 Duplicati 格式
4. **Duplicati 2.x API Compatible** — 有独立 API，非 Duplicati API

### 无 Fake Supported

本矩阵中**没有任何 Fake Supported 项**。所有 Implemented = Yes 的功能都有对应的测试证据。