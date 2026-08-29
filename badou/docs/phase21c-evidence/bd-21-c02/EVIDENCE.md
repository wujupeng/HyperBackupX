# BD-21-C02 Evidence: Real Restore E2E Test

**Date**: 2026-08-27
**Status**: ✅ COMPLETE
**Verdict**: PASS

## E2E Test Updates (e2e_cross_process.rs)

### 5 Tolerance Degradations Replaced with Real Assertions

| Step | Before (tolerance) | After (real assert) |
|------|-------------------|---------------------|
| 6 SnapshotList | `[INFO] ... known issue` | `assert!(!list_resp.snapshots.is_empty())` |
| 7 RepositoryVerify | `match { Ok => ..., Err => [WARN] }` | `.expect("verify_repository failed")` |
| 8 RecoveryOpen | `match { Ok => ..., Err => [WARN] }` | `.expect("recovery_open failed")` + BLAKE3 verify |
| 9 RepositoryStat | No assert on snapshot_count | `assert!(snapshot_count > 0)` + `assert_eq!(chunk_count, 3)` |
| Summary | `known issue: version-snapshot ID mismatch` | `full Backup/Restore chain verified (9 steps all PASS)` |

### Additional Changes

- Added `original_chunks: HashMap<String, Vec<u8>>` for RecoveryOpen BLAKE3 verification
- RecoveryOpen: collects 3 chunks, verifies each chunk's BLAKE3 against original data
- `recovered_chunks` changed from `u32` counter to `Vec<(String, Vec<u8>)>` for data verification

## Remote Execution (192.168.2.3)

**Command**: `cargo test --release -p badou-tests --test e2e_cross_process -- --nocapture`
**Environment**: Real badou-server (PID 8560) on 192.168.2.3:9090, release build
**Result**: `test result: ok. 1 passed; 0 failed`

## 9-Step PASS Output

```
[PASS] Repository created: repo_id=86ec1699-5a97-4f69-9901-7bee37bcc3c0
[PASS] chunk_put #0: hash=980d581da4a4a0e3.., stored_size=55
[PASS] chunk_put #1: hash=64e6e601c6f22e61.., stored_size=60
[PASS] chunk_put #2: hash=b01eb9000c096496.., stored_size=65536
[PASS] Snapshot committed: version_id=a5879fc6-c924-43e6-8f0f-7594b1553262
[PASS] chunk_get #0: BLAKE3 match, size=55
[PASS] chunk_get #1: BLAKE3 match, size=60
[PASS] chunk_get #2: BLAKE3 match, size=65536
[PASS] SnapshotList: 1 snapshots (snapshot_id consistency verified)
[PASS] Repository verify complete: 0 reports
[PASS] Recovery complete: 3 chunks, all BLAKE3 verified
[PASS] Repository stat: chunk_count=3, snapshot_count=1 (snapshot_count > 0 verified)
[PASS] -- full Backup/Restore chain verified (9 steps all PASS)
```

## Known Issue Markers Removal

- `grep "known issue" e2e_cross_process.rs` → No output ✅
- `grep "[WARN]" e2e_cross_process.rs` → No output ✅
- `grep "[INFO]" e2e_cross_process.rs` → No output ✅

## Real Cross-Process Environment

- Endpoint: http://127.0.0.1:9090 (real gRPC, not in-process mock)
- Server: badou-server release build on Debian 13 (192.168.2.3)
- JWT authentication: HMAC-SHA256 with secret=phase21-test, role=admin
- Data flow: Client → gRPC → Server → BaDou Storage → ChunkStore → Disk

## Spec Rule Alignment (§5.2.1)

| Rule | Description | Status |
|------|-------------|--------|
| 1 | E2E test uses real cross-process gRPC | ✅ 192.168.2.3:9090 |
| 2 | SnapshotList non-empty assert | ✅ Step 6 |
| 3 | RecoveryOpen success assert | ✅ Step 8 |
| 4 | Recovery data BLAKE3 match | ✅ 3 chunks verified |
| 5 | snapshot_count > 0 assert | ✅ Step 9 |
| 6 | RepositoryVerify success assert | ✅ Step 7 |
| 7 | Remote execution on 192.168.2.3 | ✅ Debian 13 |
| 8 | Known issue markers removed | ✅ grep confirms |
| 9 | Exit code 0 | ✅ test result: ok |
| 10 | No #[ignore] or tolerance bypass | ✅ All real asserts |
| 11 | 9 steps all PASS | ✅ See output above |