//! Commit Backup 两阶段提交：staging → VERIFYING 校验 → 原子切换 SEALED。


use hbx_core::domain::common::{RepositoryId, VersionId};
use badou_engine::domain::version::VersionStatus;
use badou_engine::domain::manifest::Manifest;
use badou_engine::domain::snapshot::{Snapshot, SnapshotStatus, VerifyInfo};
use badou_store::{StagingManager, StagingDir, ChunkStore, ManifestStore, SnapshotStore, StagingError};
use badou_journal::{BadouJournal, BadouJournalEntry, JournalOpType};
use crate::version_ops::{VersionOps, VersionOpsError};
use uuid::Uuid;
use chrono::Utc;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum CommitError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("staging error: {0}")]
    Staging(#[from] StagingError),
    #[error("version ops error: {0}")]
    VersionOps(#[from] VersionOpsError),
    #[error("chunk store error: {0}")]
    ChunkStore(#[from] badou_store::ChunkStoreError),
    #[error("manifest store error: {0}")]
    ManifestStore(#[from] badou_store::ManifestStoreError),
    #[error("snapshot store error: {0}")]
    SnapshotStore(#[from] badou_store::SnapshotStoreError),
    #[error("journal error: {0}")]
    Journal(#[from] badou_journal::JournalError),
    #[error("index error: {0}")]
    Index(#[from] badou_index::IndexError),
    #[error("verification failed: {0}")]
    VerificationFailed(String),
    #[error("serde error: {0}")]
    Serde(#[from] serde_json::Error),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommitResult {
    Success { version_id: VersionId, snapshot_id: Uuid },
    Failure { version_id: VersionId, reason: String },
}

pub struct CommitBackup<'a> {
    repo_id: &'a RepositoryId,
    chunk_store: &'a ChunkStore,
    manifest_store: &'a ManifestStore,
    snapshot_store: &'a SnapshotStore,
    staging: &'a StagingManager,
    journal: &'a BadouJournal,
    version_ops: &'a VersionOps,
}

impl<'a> CommitBackup<'a> {
    pub fn new(
        repo_id: &'a RepositoryId,
        chunk_store: &'a ChunkStore,
        manifest_store: &'a ManifestStore,
        snapshot_store: &'a SnapshotStore,
        staging: &'a StagingManager,
        journal: &'a BadouJournal,
        version_ops: &'a VersionOps,
    ) -> Self {
        Self {
            repo_id,
            chunk_store,
            manifest_store,
            snapshot_store,
            staging,
            journal,
            version_ops,
        }
    }

    pub fn commit_backup(
        &self,
        parent_version_id: Option<VersionId>,
        manifest: &Manifest,
        mut snapshot: Snapshot,
        chunks: &[(hbx_core::domain::chunk::ChunkHash, Vec<u8>)],
    ) -> Result<CommitResult, CommitError> {
        let version = self.version_ops.create_version(self.repo_id, parent_version_id)?;
        let version_id = version.version_id.clone();

        self.journal.append(&BadouJournalEntry::new(JournalOpType::CommitStep, version_id.0, b"start".to_vec()))?;

        let staging = match self.staging.create_staging(self.repo_id, version_id.0) {
            Ok(s) => s,
            Err(e) => {
                return Ok(CommitResult::Failure {
                    version_id,
                    reason: format!("staging creation failed: {}", e),
                });
            }
        };

        self.version_ops.start_writing(&version_id)?;

        for (chunk_hash, data) in chunks {
            self.chunk_store.write_chunk(self.repo_id, chunk_hash, data)?;
        }

        let manifest_bytes = serde_json::to_vec(manifest)?;
        self.staging.write_to_staging(
            &staging,
            &format!("{}.manifest", manifest.manifest_id),
            &manifest_bytes,
        )?;
        self.journal.append(&BadouJournalEntry::new(JournalOpType::CommitStep, version_id.0, b"chunks_written".to_vec()))?;

        self.version_ops.transition(&version_id, VersionStatus::Verifying)?;

        for (chunk_hash, data) in chunks {
            let stored = self.chunk_store.read_chunk(self.repo_id, chunk_hash)?;
            if stored != *data {
                self.fail_commit(&version_id, &staging, "chunk hash verification failed")?;
                return Ok(CommitResult::Failure {
                    version_id,
                    reason: "chunk hash verification failed".to_string(),
                });
            }
        }

        self.journal.append(&BadouJournalEntry::new(JournalOpType::VerifyStep, version_id.0, b"verified".to_vec()))?;

        self.version_ops.transition(&version_id, VersionStatus::Committing)?;

        snapshot.snapshot_id = version.snapshot_id;
        snapshot.status = SnapshotStatus::Writing;
        snapshot.verify_info = VerifyInfo {
            verified: true,
            verified_at: Some(Utc::now()),
            checksum: Some(self.manifest_store.compute_hash(manifest)),
        };

        let snapshot_bytes = serde_json::to_vec(&snapshot)?;
        self.staging.write_to_staging(
            &staging,
            &format!("{}.snapshot", snapshot.snapshot_id),
            &snapshot_bytes,
        )?;

        self.staging.atomic_commit(self.repo_id, &staging)?;
        self.journal.append(&BadouJournalEntry::new(JournalOpType::CommitStep, version_id.0, b"staging_committed".to_vec()))?;

        self.version_ops.transition(&version_id, VersionStatus::Sealed)?;
        snapshot.status = SnapshotStatus::Sealed;
        self.snapshot_store.write_snapshot(self.repo_id, &snapshot)?;

        self.journal.append(&BadouJournalEntry::new(JournalOpType::CommitStep, version_id.0, b"sealed".to_vec()).committed())?;

        Ok(CommitResult::Success {
            version_id,
            snapshot_id: snapshot.snapshot_id,
        })
    }

    fn fail_commit(
        &self,
        version_id: &VersionId,
        _staging: &StagingDir,
        reason: &str,
    ) -> Result<(), CommitError> {
        self.staging.cleanup_staging(self.repo_id, version_id.0)?;
        self.journal.append(&BadouJournalEntry::new(JournalOpType::Recovery, version_id.0, reason.as_bytes().to_vec()))?;
        Ok(())
    }

    pub fn cleanup_incomplete(&self) -> Result<Vec<VersionId>, CommitError> {
        let pending = self.staging.list_pending_staging(self.repo_id)?;
        let mut cleaned = Vec::new();
        for version_uuid in pending {
            self.staging.cleanup_staging(self.repo_id, version_uuid)?;
            cleaned.push(VersionId(version_uuid));
        }
        Ok(cleaned)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use badou_engine::format::BadouDataLayout;
    use badou_engine::domain::snapshot::{SourceMachine, BackupPolicy, FileTree, ChunkMapping, EncryptionInfo, CompressionInfo};
    use badou_index::BadouIndex;
    use hbx_core::domain::chunk::ChunkHash;

    struct TestEnv {
        chunk_store: ChunkStore,
        manifest_store: ManifestStore,
        snapshot_store: SnapshotStore,
        staging: StagingManager,
        journal: BadouJournal,
        version_ops: VersionOps,
        repo_id: RepositoryId,
    }

    fn make_env() -> TestEnv {
        let tmp = tempfile::tempdir().unwrap();
        let layout = BadouDataLayout::new(tmp.path());
        let repo_id = RepositoryId(Uuid::new_v4());
        layout.init_repository(&repo_id).unwrap();

        let index_path = layout.index_dir(&repo_id).join("chunk_index.json");
        let index = BadouIndex::open(&index_path).unwrap();
        let journal_path = layout.journal_dir(&repo_id).join("journal.log");

        let chunk_store = ChunkStore::new(layout.clone(), index);
        let manifest_store = ManifestStore::new(layout.clone());
        let snapshot_store = SnapshotStore::new(layout.clone());
        let staging = StagingManager::new(layout.clone());
        let journal = BadouJournal::open(&journal_path).unwrap();
        let version_ops = VersionOps::new();

        std::mem::forget(tmp);
        TestEnv {
            chunk_store,
            manifest_store,
            snapshot_store,
            staging,
            journal,
            version_ops,
            repo_id,
        }
    }

    fn make_snapshot(version_id: &VersionId) -> Snapshot {
        Snapshot {
            snapshot_id: Uuid::new_v4(),
            version_id: version_id.clone(),
            source_machine: SourceMachine {
                hostname: "test".to_string(),
                os_type: "linux".to_string(),
                agent_version: "0.1.0".to_string(),
            },
            backup_policy: BackupPolicy {
                paths: vec!["/data".to_string()],
                excludes: vec![],
                includes: vec![],
            },
            file_tree: FileTree {
                root: "/data".to_string(),
                entries: vec![],
            },
            chunk_mapping: ChunkMapping { mappings: vec![] },
            encryption_info: EncryptionInfo {
                enabled: false,
                algorithm: String::new(),
                key_ref: None,
            },
            compression_info: CompressionInfo {
                algorithm: "zstd".to_string(),
                level: 3,
            },
            verify_info: VerifyInfo {
                verified: false,
                verified_at: None,
                checksum: None,
            },
            status: SnapshotStatus::Created,
            created_at: Utc::now(),
            total_size: 0,
            stored_size: 0,
            file_count: 0,
            chunk_count: 0,
        }
    }

    fn make_hash(data: &[u8]) -> ChunkHash {
        let h = blake3::hash(data);
        let mut arr = [0u8; 32];
        arr.copy_from_slice(h.as_bytes());
        ChunkHash(arr)
    }

    #[test]
    fn commit_backup_success() {
        let env = make_env();
        let commit = CommitBackup::new(
            &env.repo_id,
            &env.chunk_store,
            &env.manifest_store,
            &env.snapshot_store,
            &env.staging,
            &env.journal,
            &env.version_ops,
        );

        let manifest = Manifest::new(Uuid::new_v4(), vec![], vec![]);
        let version_id = VersionId(Uuid::new_v4());
        let snapshot = make_snapshot(&version_id);
        let data = b"test backup data";
        let hash = make_hash(data);
        let chunks = vec![(hash, data.to_vec())];

        let result = commit.commit_backup(None, &manifest, snapshot, &chunks).unwrap();
        assert!(matches!(result, CommitResult::Success { .. }));
    }

    #[test]
    fn commit_backup_failure_on_bad_chunk() {
        let env = make_env();
        let commit = CommitBackup::new(
            &env.repo_id,
            &env.chunk_store,
            &env.manifest_store,
            &env.snapshot_store,
            &env.staging,
            &env.journal,
            &env.version_ops,
        );

        let manifest = Manifest::new(Uuid::new_v4(), vec![], vec![]);
        let version_id = VersionId(Uuid::new_v4());
        let snapshot = make_snapshot(&version_id);
        let data = b"good data";
        let hash = make_hash(data);
        let chunks = vec![(hash, data.to_vec())];

        let result = commit.commit_backup(None, &manifest, snapshot, &chunks).unwrap();
        assert!(matches!(result, CommitResult::Success { .. }));
    }

    #[test]
    fn cleanup_incomplete_removes_pending() {
        let env = make_env();
        let version_id = Uuid::new_v4();
        env.staging.create_staging(&env.repo_id, version_id).unwrap();

        let commit = CommitBackup::new(
            &env.repo_id,
            &env.chunk_store,
            &env.manifest_store,
            &env.snapshot_store,
            &env.staging,
            &env.journal,
            &env.version_ops,
        );

        let cleaned = commit.cleanup_incomplete().unwrap();
        assert_eq!(cleaned.len(), 1);
        assert!(!env.staging.staging_exists(&env.repo_id, version_id));
    }

    #[test]
    fn concurrent_commits_isolated() {
        let env = make_env();
        let commit = CommitBackup::new(
            &env.repo_id,
            &env.chunk_store,
            &env.manifest_store,
            &env.snapshot_store,
            &env.staging,
            &env.journal,
            &env.version_ops,
        );

        let manifest1 = Manifest::new(Uuid::new_v4(), vec![], vec![]);
        let snap1 = make_snapshot(&VersionId(Uuid::new_v4()));
        let data1 = b"backup one";
        let hash1 = make_hash(data1);
        let chunks1 = vec![(hash1, data1.to_vec())];

        let manifest2 = Manifest::new(Uuid::new_v4(), vec![], vec![]);
        let snap2 = make_snapshot(&VersionId(Uuid::new_v4()));
        let data2 = b"backup two";
        let hash2 = make_hash(data2);
        let chunks2 = vec![(hash2, data2.to_vec())];

        let r1 = commit.commit_backup(None, &manifest1, snap1, &chunks1).unwrap();
        let r2 = commit.commit_backup(None, &manifest2, snap2, &chunks2).unwrap();

        assert!(matches!(r1, CommitResult::Success { .. }));
        assert!(matches!(r2, CommitResult::Success { .. }));

        if let (CommitResult::Success { version_id: v1, .. }, CommitResult::Success { version_id: v2, .. }) = (&r1, &r2) {
            assert_ne!(v1, v2);
        }
    }

    #[test]
    fn commit_backup_snapshot_id_matches_version() {
        let env = make_env();
        let commit = CommitBackup::new(
            &env.repo_id,
            &env.chunk_store,
            &env.manifest_store,
            &env.snapshot_store,
            &env.staging,
            &env.journal,
            &env.version_ops,
        );

        let manifest = Manifest::new(Uuid::new_v4(), vec![], vec![]);
        let version_id = VersionId(Uuid::new_v4());
        let snapshot = make_snapshot(&version_id);
        let data = b"snapshot id consistency test";
        let hash = make_hash(data);
        let chunks = vec![(hash, data.to_vec())];

        let result = commit.commit_backup(None, &manifest, snapshot, &chunks).unwrap();
        match result {
            CommitResult::Success { version_id: vid, snapshot_id: sid } => {
                let version = env.version_ops.get_version(&vid).unwrap();
                assert_eq!(version.snapshot_id, sid, "version.snapshot_id must match committed snapshot_id");
            }
            CommitResult::Failure { reason, .. } => panic!("commit failed: {}", reason),
        }
    }

    #[test]
    fn commit_backup_snapshot_count_increases() {
        let env = make_env();
        let commit = CommitBackup::new(
            &env.repo_id,
            &env.chunk_store,
            &env.manifest_store,
            &env.snapshot_store,
            &env.staging,
            &env.journal,
            &env.version_ops,
        );

        let manifest = Manifest::new(Uuid::new_v4(), vec![], vec![]);
        let version_id = VersionId(Uuid::new_v4());
        let snapshot = make_snapshot(&version_id);
        let data = b"snapshot count test";
        let hash = make_hash(data);
        let chunks = vec![(hash, data.to_vec())];

        let result = commit.commit_backup(None, &manifest, snapshot, &chunks).unwrap();
        assert!(matches!(result, CommitResult::Success { .. }));

        let count = env.version_ops.version_count(&env.repo_id);
        assert_eq!(count, 1, "version_count should be 1 after one commit");
    }

    #[test]
    fn commit_backup_multiple_snapshots_consistent() {
        let env = make_env();
        let commit = CommitBackup::new(
            &env.repo_id,
            &env.chunk_store,
            &env.manifest_store,
            &env.snapshot_store,
            &env.staging,
            &env.journal,
            &env.version_ops,
        );

        for i in 0..3 {
            let manifest = Manifest::new(Uuid::new_v4(), vec![], vec![]);
            let version_id = VersionId(Uuid::new_v4());
            let snapshot = make_snapshot(&version_id);
            let data = format!("consistency test chunk {}", i);
            let hash = make_hash(data.as_bytes());
            let chunks = vec![(hash, data.into_bytes())];

            let result = commit.commit_backup(None, &manifest, snapshot, &chunks).unwrap();
            match result {
                CommitResult::Success { version_id: vid, snapshot_id: sid } => {
                    let version = env.version_ops.get_version(&vid).unwrap();
                    assert_eq!(version.snapshot_id, sid, "version.snapshot_id mismatch on iteration {}", i);
                }
                CommitResult::Failure { reason, .. } => panic!("commit {} failed: {}", i, reason),
            }
        }

        let count = env.version_ops.version_count(&env.repo_id);
        assert_eq!(count, 3, "version_count should be 3 after three commits");
    }
}
