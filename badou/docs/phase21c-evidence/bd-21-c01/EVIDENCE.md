# BD-21-C01 Evidence: Snapshot ID Consistency Fix

**Date**: 2026-08-27
**Status**: ✅ COMPLETE
**Verdict**: PASS

## Fix Summary

### C01-1: commit_backup snapshot_id consistency (commit.rs:129)

**Before**: `snapshot.snapshot_id = Uuid::new_v4();` (generated new UUID, mismatched version.snapshot_id)
**After**: `snapshot.snapshot_id = version.snapshot_id;` (reuses version's snapshot_id, single source of truth)

**File**: `badou/crates/badou-ops/src/commit.rs` line 129
**Diff**: 1 line core change

### C01-2: repository_stat snapshot_count hardcode (repository_rpc.rs:185)

**Before**: `snapshot_count: 0,` (hardcoded zero)
**After**: `snapshot_count: state.version_ops().version_count(&repo_id) as u64,` (real count from version_ops)

**File**: `badou/crates/badou-hbop-server/src/repository_rpc.rs` line 185
**Additional**: `repo_id.clone()` at line 168 to prevent move
**Diff**: 2 lines (count fix + clone fix)

### C01-3: FileEntry population (snapshot_rpc.rs:88)

**Before**: `entries: vec![],` (empty, RecoveryEngine had no files to recover)
**After**: `entries: manifest.chunk_refs.iter().map(|r| FileEntry { ... }).collect(),` (one entry per chunk)

**File**: `badou/crates/badou-hbop-server/src/snapshot_rpc.rs` lines 86-93
**Import**: Added `FileEntry` to use declaration
**Diff**: 8 lines (import + entries construction)

## New Unit Tests (3)

1. `commit_backup_snapshot_id_matches_version` — asserts `version.snapshot_id == snapshot.snapshot_id`
2. `commit_backup_snapshot_count_increases` — asserts snapshot_count > 0 after commit
3. `commit_backup_multiple_snapshots_consistent` — asserts consistency across multiple commits

**File**: `badou/crates/badou-ops/src/commit.rs` tests module

## Verification Results

| Check | Result | Evidence |
|-------|--------|----------|
| Local clippy zero warnings | PASS | `cargo clippy --workspace --all-targets -- -D warnings` exit 0 |
| Local cargo test --workspace | PASS | 234 tests, 0 failed (excluding E2E) |
| Remote clippy (192.168.2.3) | PASS | `cargo clippy --workspace --all-targets -- -D warnings` exit 0 |
| Remote cargo test --workspace --release | PASS | 235 tests, 0 failed (including E2E) |
| Remote root workspace test | PRE-EXISTING FAIL | hbx-agent crate fails on Linux (std::os::windows), unrelated to our changes |

## Spec Rule Alignment (§5.1.1)

| Rule | Description | Status |
|------|-------------|--------|
| 1 | version.snapshot_id == snapshot.snapshot_id | ✅ Fixed + tested |
| 2 | SnapshotList non-empty via version join | ✅ E2E Step 6 PASS |
| 3 | RecoveryOpen succeeds via version.snapshot_id | ✅ E2E Step 8 PASS |
| 4 | snapshot_count > 0 in repository_stat | ✅ Fixed + E2E Step 9 PASS |
| 5 | Fix inside commit_backup function | ✅ commit.rs:129 |
| 6 | Minimal diff (core lines) | ✅ 11 lines total (3 files) |
| 7 | No API/proto/signature changes | ✅ Verified |
| 8 | clippy zero warnings | ✅ Local + remote |
| 9 | cargo test --workspace passes | ✅ 235 tests |
| 10 | Root workspace behavior unchanged | ✅ Pre-existing hbx-agent issue only |
| 11 | 3 new unit tests | ✅ All pass |
| 12 | No bypass patches | ✅ Direct fix at source |
| 13 | No fake snapshots | ✅ Real version.snapshot_id used |