mod common;
use common::*;
use badou_ops::commit::CommitResult;
use hbx_core::domain::common::VersionId;
use uuid::Uuid;

#[test]
fn e2e_full_lifecycle() {
    let env = E2EEnv::new();
    let data1 = b"chunk data for backup 1";
    let hash1 = make_hash(data1);
    let data2 = b"chunk data for backup 2";
    let hash2 = make_hash(data2);
    let manifest = make_manifest();
    let version_id = VersionId(Uuid::new_v4());
    let snapshot = make_snapshot(&version_id);
    let chunks = vec![(hash1, data1.to_vec()), (hash2, data2.to_vec())];
    let result = env.commit().commit_backup(None, &manifest, snapshot, &chunks).unwrap();
    assert!(matches!(result, CommitResult::Success { .. }));
}

#[test]
fn e2e_commit_empty_backup() {
    let env = E2EEnv::new();
    let manifest = make_manifest();
    let version_id = VersionId(Uuid::new_v4());
    let snapshot = make_snapshot(&version_id);
    let chunks: Vec<(hbx_core::domain::chunk::ChunkHash, Vec<u8>)> = vec![];
    let result = env.commit().commit_backup(None, &manifest, snapshot, &chunks).unwrap();
    assert!(matches!(result, CommitResult::Success { .. }));
}

#[test]
fn e2e_multiple_commits() {
    let env = E2EEnv::new();
    for i in 0..5 {
        let data = format!("backup data iteration {}", i);
        let hash = make_hash(data.as_bytes());
        let manifest = make_manifest();
        let version_id = VersionId(Uuid::new_v4());
        let snapshot = make_snapshot(&version_id);
        let chunks = vec![(hash, data.into_bytes())];
        let result = env.commit().commit_backup(None, &manifest, snapshot, &chunks).unwrap();
        assert!(matches!(result, CommitResult::Success { .. }), "commit {} failed", i);
    }
}

#[test]
fn e2e_deduplication() {
    let env = E2EEnv::new();
    let shared_data = b"shared chunk data between backups";
    let shared_hash = make_hash(shared_data);

    let manifest1 = make_manifest();
    let vid1 = VersionId(Uuid::new_v4());
    let snap1 = make_snapshot(&vid1);
    let chunks1 = vec![(shared_hash.clone(), shared_data.to_vec())];
    let result1 = env.commit().commit_backup(None, &manifest1, snap1, &chunks1).unwrap();
    assert!(matches!(result1, CommitResult::Success { .. }));

    let manifest2 = make_manifest();
    let vid2 = VersionId(Uuid::new_v4());
    let snap2 = make_snapshot(&vid2);
    let chunks2 = vec![(shared_hash, shared_data.to_vec())];
    let result2 = env.commit().commit_backup(None, &manifest2, snap2, &chunks2).unwrap();
    assert!(matches!(result2, CommitResult::Success { .. }));
}

#[test]
fn e2e_large_chunk() {
    let env = E2EEnv::new();
    let data = vec![0xABu8; 1024 * 1024];
    let hash = make_hash(&data);
    let manifest = make_manifest();
    let version_id = VersionId(Uuid::new_v4());
    let snapshot = make_snapshot(&version_id);
    let chunks = vec![(hash, data)];
    let result = env.commit().commit_backup(None, &manifest, snapshot, &chunks).unwrap();
    assert!(matches!(result, CommitResult::Success { .. }));
}

#[test]
fn e2e_many_small_chunks() {
    let env = E2EEnv::new();
    let mut chunks = Vec::new();
    for i in 0..100 {
        let data = format!("small chunk {:04}", i).into_bytes();
        let hash = make_hash(&data);
        chunks.push((hash, data));
    }
    let manifest = make_manifest();
    let version_id = VersionId(Uuid::new_v4());
    let snapshot = make_snapshot(&version_id);
    let result = env.commit().commit_backup(None, &manifest, snapshot, &chunks).unwrap();
    assert!(matches!(result, CommitResult::Success { .. }));
}
