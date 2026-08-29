mod common;
use common::*;
use badou_journal::{BadouJournal, BadouJournalEntry, JournalOpType};
use badou_recovery::{RecoveryEngine, RecoveryRequest};
use uuid::Uuid;

#[test]
fn chaos_journal_append_and_read_all() {
    let tmp = tempfile::tempdir().unwrap();
    let journal_path = tmp.path().join("test_journal.log");
    let journal = BadouJournal::open(&journal_path).unwrap();

    let entry = BadouJournalEntry::new(
        JournalOpType::CommitStep,
        Uuid::new_v4(),
        b"test payload".to_vec(),
    );
    journal.append(&entry).unwrap();
    drop(journal);

    let journal2 = BadouJournal::open(&journal_path).unwrap();
    let entries = journal2.read_all().unwrap();
    assert!(!entries.is_empty());
}

#[test]
fn chaos_journal_multiple_entries() {
    let tmp = tempfile::tempdir().unwrap();
    let journal_path = tmp.path().join("multi_journal.log");
    let journal = BadouJournal::open(&journal_path).unwrap();

    for i in 0..10 {
        let entry = BadouJournalEntry::new(
            JournalOpType::CommitStep,
            Uuid::new_v4(),
            format!("payload {}", i).into_bytes(),
        );
        journal.append(&entry).unwrap();
    }
    drop(journal);

    let journal2 = BadouJournal::open(&journal_path).unwrap();
    let entries = journal2.read_all().unwrap();
    assert_eq!(entries.len(), 10);
}

#[test]
fn chaos_recovery_empty_snapshot_fails() {
    let env = E2EEnv::new();
    let recovery = RecoveryEngine::new(
        &env.repo_id,
        &env.chunk_store,
        &env.manifest_store,
        &env.snapshot_store,
    );
    let request = RecoveryRequest {
        snapshot_id: Uuid::new_v4(),
        file_filter: None,
    };
    assert!(recovery.recover(&request).is_err());
}

#[test]
fn chaos_recovery_verify_sealed_nonexistent_fails() {
    let env = E2EEnv::new();
    let recovery = RecoveryEngine::new(
        &env.repo_id,
        &env.chunk_store,
        &env.manifest_store,
        &env.snapshot_store,
    );
    assert!(recovery.verify_sealed(Uuid::new_v4()).is_err());
}

#[test]
fn chaos_commit_consistency() {
    let env = E2EEnv::new();
    for i in 0..10 {
        let data = format!("consistency test chunk {}", i);
        let hash = make_hash(data.as_bytes());
        let manifest = make_manifest();
        let version_id = hbx_core::domain::common::VersionId(Uuid::new_v4());
        let snapshot = make_snapshot(&version_id);
        let chunks = vec![(hash, data.into_bytes())];
        let result = env.commit().commit_backup(None, &manifest, snapshot, &chunks).unwrap();
        assert!(matches!(result, badou_ops::commit::CommitResult::Success { .. }));
    }
}
