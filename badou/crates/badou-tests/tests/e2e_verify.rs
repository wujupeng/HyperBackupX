mod common;
use common::*;
use badou_verify::{Verifier, VerifyStatus};
use hbx_core::domain::chunk::ChunkHash;

#[test]
fn e2e_verify_chunk_pass() {
    let env = E2EEnv::new();
    let data = b"verify test data";
    let hash = make_hash(data);
    env.chunk_store.write_chunk(&env.repo_id, &hash, data).unwrap();

    let verifier = Verifier::new(
        &env.repo_id,
        &env.chunk_store,
        &env.manifest_store,
        &env.snapshot_store,
        &env.index,
    );
    let report = verifier.verify_chunk(&hash);
    assert_eq!(report.status, VerifyStatus::Pass);
}

#[test]
fn e2e_verify_chunk_mismatch() {
    let env = E2EEnv::new();
    let data = b"original data";
    let hash = make_hash(data);
    env.chunk_store.write_chunk(&env.repo_id, &hash, data).unwrap();

    let fake_hash = ChunkHash([0xFF; 32]);
    let verifier = Verifier::new(
        &env.repo_id,
        &env.chunk_store,
        &env.manifest_store,
        &env.snapshot_store,
        &env.index,
    );
    let report = verifier.verify_chunk(&fake_hash);
    assert!(matches!(report.status, VerifyStatus::Missing { .. }));
}

#[test]
fn e2e_verify_after_commit() {
    let env = E2EEnv::new();
    let data = b"verify after commit";
    let hash = make_hash(data);
    let manifest = make_manifest();
    let version_id = hbx_core::domain::common::VersionId(uuid::Uuid::new_v4());
    let snapshot = make_snapshot(&version_id);
    let chunks = vec![(hash.clone(), data.to_vec())];
    let _ = env.commit().commit_backup(None, &manifest, snapshot, &chunks).unwrap();

    let verifier = Verifier::new(
        &env.repo_id,
        &env.chunk_store,
        &env.manifest_store,
        &env.snapshot_store,
        &env.index,
    );
    let report = verifier.verify_chunk(&hash);
    assert_eq!(report.status, VerifyStatus::Pass);
}
