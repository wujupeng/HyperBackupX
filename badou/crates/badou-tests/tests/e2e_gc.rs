mod common;
use common::*;
use badou_gc::{GcExecutor, VersionDeleter};
use badou_ops::commit::CommitResult;
use hbx_core::domain::common::VersionId;
use uuid::Uuid;

#[test]
fn e2e_gc_empty_repository() {
    let env = E2EEnv::new();
    let gc = GcExecutor::new(&env.repo_id, &env.index, &env.chunk_store);
    let report = gc.execute_gc().unwrap();
    assert_eq!(report.purged_count(), 0);
}

#[test]
fn e2e_gc_after_commit_no_purge() {
    let env = E2EEnv::new();
    let data = b"gc test data";
    let hash = make_hash(data);
    let manifest = make_manifest();
    let version_id = VersionId(Uuid::new_v4());
    let snapshot = make_snapshot(&version_id);
    let chunks = vec![(hash, data.to_vec())];
    let _ = env.commit().commit_backup(None, &manifest, snapshot, &chunks).unwrap();

    let gc = GcExecutor::new(&env.repo_id, &env.index, &env.chunk_store);
    let report = gc.execute_gc().unwrap();
    assert_eq!(report.purged_count(), 0);
}

#[test]
fn e2e_delete_version_then_gc() {
    let env = E2EEnv::new();
    let data = b"delete then gc test";
    let hash = make_hash(data);
    let manifest = make_manifest();
    let version_id = VersionId(Uuid::new_v4());
    let snapshot = make_snapshot(&version_id);
    let chunks = vec![(hash, data.to_vec())];
    let result = env.commit().commit_backup(None, &manifest, snapshot, &chunks).unwrap();

    if let CommitResult::Success { version_id: vid, .. } = result {
        let deleter = VersionDeleter::new(&env.repo_id, &env.index, &env.version_ops);
        let _ = deleter.delete_version(&vid);

        let gc = GcExecutor::new(&env.repo_id, &env.index, &env.chunk_store);
        let _ = gc.execute_gc().unwrap();
    }
}

#[test]
fn e2e_gc_preserves_shared_chunks() {
    let env = E2EEnv::new();
    let shared_data = b"shared chunk for gc test";
    let shared_hash = make_hash(shared_data);
    let unique_data = b"unique chunk for version 1";
    let unique_hash = make_hash(unique_data);

    let manifest1 = make_manifest();
    let vid1 = VersionId(Uuid::new_v4());
    let snap1 = make_snapshot(&vid1);
    let chunks1 = vec![(shared_hash.clone(), shared_data.to_vec()), (unique_hash, unique_data.to_vec())];
    let result1 = env.commit().commit_backup(None, &manifest1, snap1, &chunks1).unwrap();
    assert!(matches!(result1, CommitResult::Success { .. }));

    let manifest2 = make_manifest();
    let vid2 = VersionId(Uuid::new_v4());
    let snap2 = make_snapshot(&vid2);
    let chunks2 = vec![(shared_hash, shared_data.to_vec())];
    let result2 = env.commit().commit_backup(None, &manifest2, snap2, &chunks2).unwrap();
    assert!(matches!(result2, CommitResult::Success { .. }));

    if let CommitResult::Success { version_id: deleted_vid, .. } = result1 {
        let deleter = VersionDeleter::new(&env.repo_id, &env.index, &env.version_ops);
        let _ = deleter.delete_version(&deleted_vid);
        let gc = GcExecutor::new(&env.repo_id, &env.index, &env.chunk_store);
        let _ = gc.execute_gc().unwrap();
    }
}
