#![allow(dead_code)]

use badou_engine::format::BadouDataLayout;
use badou_engine::domain::snapshot::{
    SourceMachine, BackupPolicy, FileTree, ChunkMapping, EncryptionInfo, CompressionInfo, VerifyInfo,
    Snapshot, SnapshotStatus,
};
use badou_engine::domain::manifest::Manifest;
use badou_index::BadouIndex;
use badou_journal::BadouJournal;
use badou_ops::{
    commit::CommitBackup,
    version_ops::VersionOps,
};
use badou_store::{ChunkStore, ManifestStore, SnapshotStore, StagingManager};
use hbx_core::domain::chunk::ChunkHash;
use hbx_core::domain::common::{RepositoryId, VersionId};
use uuid::Uuid;
use chrono::Utc;

#[allow(dead_code)]
pub struct E2EEnv {
    pub chunk_store: ChunkStore,
    pub manifest_store: ManifestStore,
    pub snapshot_store: SnapshotStore,
    pub staging: StagingManager,
    pub journal: BadouJournal,
    pub version_ops: VersionOps,
    pub index: BadouIndex,
    pub repo_id: RepositoryId,
    pub _tmp: tempfile::TempDir,
}

impl E2EEnv {
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        let tmp = tempfile::tempdir().unwrap();
        let layout = BadouDataLayout::new(tmp.path());
        let repo_id = RepositoryId(Uuid::new_v4());
        layout.init_repository(&repo_id).unwrap();

        let index_path = layout.index_dir(&repo_id).join("chunk_index.json");
        let index = BadouIndex::open(&index_path).unwrap();
        let journal_path = layout.journal_dir(&repo_id).join("journal.log");

        let chunk_store = ChunkStore::new(layout.clone(), index.clone());
        let manifest_store = ManifestStore::new(layout.clone());
        let snapshot_store = SnapshotStore::new(layout.clone());
        let staging = StagingManager::new(layout.clone());
        let journal = BadouJournal::open(&journal_path).unwrap();
        let version_ops = VersionOps::new();

        E2EEnv {
            chunk_store,
            manifest_store,
            snapshot_store,
            staging,
            journal,
            version_ops,
            index,
            repo_id,
            _tmp: tmp,
        }
    }

    pub fn commit(&self) -> CommitBackup<'_> {
        CommitBackup::new(
            &self.repo_id,
            &self.chunk_store,
            &self.manifest_store,
            &self.snapshot_store,
            &self.staging,
            &self.journal,
            &self.version_ops,
        )
    }
}

pub fn make_hash(data: &[u8]) -> ChunkHash {
    let h = blake3::hash(data);
    let mut arr = [0u8; 32];
    arr.copy_from_slice(h.as_bytes());
    ChunkHash(arr)
}

pub fn make_snapshot(version_id: &VersionId) -> Snapshot {
    Snapshot {
        snapshot_id: Uuid::new_v4(),
        version_id: version_id.clone(),
        source_machine: SourceMachine {
            hostname: "e2e-test".to_string(),
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

pub fn make_manifest() -> Manifest {
    Manifest::new(Uuid::new_v4(), vec![], vec![])
}
