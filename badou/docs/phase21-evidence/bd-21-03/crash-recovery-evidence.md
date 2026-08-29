# Gate-BD-21-03: Crash/Recovery Test Evidence

> **日期**: 2026-08-26
> **环境**: Debian 13 (192.168.2.3), badou-server release build

## 测试流程

1. 记录 pre-crash 状态
2. `kill -9` badou-server (SIGKILL)
3. 验证数据文件持久化在磁盘上
4. 重启 badou-server
5. 验证服务器健康
6. 运行完整 E2E gRPC 测试

## 测试结果

```
========== Phase BD-21-03 Crash/Recovery Test ==========
Start: Wed Aug 26 03:24:35 PM UTC 2026

[Step 1] Recording pre-crash state...
  chunks: 9
  snapshots: 3
  manifests: 3
  health: {"data_root":"/tmp/badou-data","status":"healthy"}

[Step 2] Killing badou-server with SIGKILL...
  PID before kill: 250620
  PID after kill: none
  [PASS] Server killed successfully

[Step 3] Verifying data files persist on disk...
  chunks: 9 (was 9)
  snapshots: 3 (was 3)
  manifests: 3 (was 3)
  [PASS] All data files persisted on disk

[Step 4] Restarting badou-server...
  New PID: 257540
  [PASS] Server restarted successfully

[Step 5] Verifying server health after restart...
  health: {"data_root":"/tmp/badou-data","status":"healthy"}
  [PASS] Server healthy after restart

[Step 6] Verifying data accessible after restart...
  repo_id: 0309cb22-a295-4b18-acc7-81cdbad19bc8
  versions: {"versions":[]}
  [PASS] Management API accessible after restart

[Step 7] Running E2E gRPC test after restart...
  [PASS] E2E gRPC test passed after restart

========== Crash/Recovery Test Summary ==========
  Pre-crash chunks:  9
  Post-crash chunks: 9
  Server killed:     SIGKILL
  Server restarted:  PID 257540
  Health:            healthy
  E2E after restart: PASS
  Status:            [PASS]
===================================================
```

## 结论

✅ **PASS** — SIGKILL 后数据全部持久化，服务器重启后 E2E gRPC 测试再次通过。