# BD-22-04 Evidence: 真实恢复 + 双哈希验证

**Date**: 2026-08-28
**Status**: ✅ PASS
**Server**: 192.168.2.87 (Debian 13, Rust 1.98.0)

## 测试结果

### Debian 13
```
BD-22-04: Generated 106172810 bytes (101.25 MB)
BD-22-04: Backup PASS - version_id=...
BD-22-04: Deleted all source files
BD-22-04: Restore PASS - files_restored=130, files_failed=0, verified=true
BD-22-04: All 130 files verified with SHA-256 + BLAKE3 dual hash
test result: ok. 1 passed; finished in 9.98s
```

### Windows 11
```
BD-22-04: All 130 files verified with SHA-256 + BLAKE3 dual hash
test result: ok. 1 passed; finished in 41.95s
```

## 测试流程

1. 生成 100MB 文件集 (130 文件，含子目录)
2. 计算 SHA-256 + BLAKE3 双哈希
3. 全量备份到 badou-server
4. **删除所有源文件**
5. 创建新 BaDouProvider (同 endpoint/repo_id/jwt_token)
6. RestoreEngine 恢复到新目录
7. 逐文件验证 SHA-256 + BLAKE3 双哈希

## 关键修复: chunk_locations 映射

**问题**: `write_chunk` 用 `BLAKE3(serialized_data)` 存储，但 restore 用引擎哈希 `BLAKE3(raw_data)` 查找，两者不同。

**修复**: 给 `Manifest` 添加 `chunk_locations: HashMap<String, ChunkLocation>` 字段:
- 备份引擎收集并存储 chunk location 映射
- restore 引擎优先从 manifest 查找 location
- `#[serde(default)]` 向后兼容

**修改文件**:
- `agent/crates/hbx-core/src/domain/repository.rs` — Manifest 结构体
- `agent/crates/hbx-engine/src/engine.rs` — 4 个备份方法
- `agent/crates/hbx-restore/src/lib.rs` — restore 引擎
- `agent/crates/hbx-compat-engine/src/adapter.rs` — 兼容层
- + 6 个其他文件

## 6 层分层证据

| 层 | 内容 | 状态 |
|----|------|------|
| L1 单元测试 | cargo test 编译通过 | ✅ |
| L2 集成测试 | 真实 backup→delete→restore 闭环 | ✅ |
| L3 性能 | Debian 9.98s, Win11 41.95s | ✅ |
| L4 故障恢复 | N/A | N/A |
| L5 跨平台 | Debian 13 + Win11 双平台 PASS | ✅ |
| L6 数据完整性 | **SHA-256 + BLAKE3 双哈希 130 文件全验证** | ✅ |