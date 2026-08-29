//! Snapshot 路径流式恢复引擎。

use std::collections::HashSet;
use hbx_core::domain::common::{RepositoryId, VersionId};
use badou_engine::domain::snapshot::{SnapshotStatus, FileEntry};
use badou_store::{ChunkStore, ManifestStore, SnapshotStore};
use uuid::Uuid;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum RecoveryError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("snapshot not found: {0}")]
    SnapshotNotFound(Uuid),
    #[error("snapshot corrupted: {0}")]
    SnapshotCorrupted(Uuid),
    #[error("snapshot not sealed: current status is {0:?}")]
    NotSealed(SnapshotStatus),
    #[error("chunk not found: {0}")]
    ChunkNotFound(String),
    #[error("chunk hash mismatch: expected {expected}, got {actual}")]
    HashMismatch { expected: String, actual: String },
    #[error("chunk store error: {0}")]
    ChunkStore(#[from] badou_store::ChunkStoreError),
    #[error("manifest store error: {0}")]
    ManifestStore(#[from] badou_store::ManifestStoreError),
    #[error("snapshot store error: {0}")]
    SnapshotStore(#[from] badou_store::SnapshotStoreError),
}

#[derive(Debug, Clone)]
pub struct RecoveryRequest {
    pub snapshot_id: Uuid,
    pub file_filter: Option<Vec<String>>,
}

#[derive(Debug, Clone)]
pub struct RecoveredFile {
    pub path: String,
    pub size: u64,
    pub data: Vec<u8>,
    pub chunk_hashes: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct RecoveryResult {
    pub snapshot_id: Uuid,
    pub version_id: VersionId,
    pub recovered_files: Vec<RecoveredFile>,
    pub failed_files: Vec<RecoveryFailure>,
    pub total_bytes: u64,
}

#[derive(Debug, Clone)]
pub struct RecoveryFailure {
    pub path: String,
    pub reason: String,
}

pub struct RecoveryEngine<'a> {
    repo_id: &'a RepositoryId,
    chunk_store: &'a ChunkStore,
    #[allow(dead_code)]
    manifest_store: &'a ManifestStore,
    snapshot_store: &'a SnapshotStore,
}

impl<'a> RecoveryEngine<'a> {
    pub fn new(
        repo_id: &'a RepositoryId,
        chunk_store: &'a ChunkStore,
        manifest_store: &'a ManifestStore,
        snapshot_store: &'a SnapshotStore,
    ) -> Self {
        Self { repo_id, chunk_store, manifest_store, snapshot_store }
    }

    pub fn recover(&self, request: &RecoveryRequest) -> Result<RecoveryResult, RecoveryError> {
        let snapshot = self.snapshot_store.read_snapshot(self.repo_id, request.snapshot_id)
            .map_err(|_| RecoveryError::SnapshotNotFound(request.snapshot_id))?;

        if snapshot.status == SnapshotStatus::Corrupt {
            return Err(RecoveryError::SnapshotCorrupted(request.snapshot_id));
        }
        if snapshot.status != SnapshotStatus::Sealed {
            return Err(RecoveryError::NotSealed(snapshot.status));
        }

        let file_filter: Option<HashSet<String>> = request.file_filter.as_ref()
            .map(|f| f.iter().cloned().collect());

        let mut recovered_files = Vec::new();
        let mut failed_files = Vec::new();
        let mut total_bytes = 0u64;

        for entry in &snapshot.file_tree.entries {
            if let Some(ref filter) = file_filter {
                if !filter.contains(&entry.path) {
                    continue;
                }
            }

            match self.recover_file(entry) {
                Ok(file) => {
                    total_bytes += file.size;
                    recovered_files.push(file);
                }
                Err(e) => {
                    failed_files.push(RecoveryFailure {
                        path: entry.path.clone(),
                        reason: e.to_string(),
                    });
                }
            }
        }

        Ok(RecoveryResult {
            snapshot_id: request.snapshot_id,
            version_id: snapshot.version_id.clone(),
            recovered_files,
            failed_files,
            total_bytes,
        })
    }

    fn recover_file(&self, entry: &FileEntry) -> Result<RecoveredFile, RecoveryError> {
        let mut file_data = Vec::new();
        let mut chunk_hashes = Vec::new();

        for chunk_hash in &entry.chunk_hashes {
            let hash_hex = hex::encode(chunk_hash.0);
            chunk_hashes.push(hash_hex.clone());

            let chunk_data = self.chunk_store.read_chunk(self.repo_id, chunk_hash)
                .map_err(|_| RecoveryError::ChunkNotFound(hash_hex.clone()))?;

            let actual_hash = blake3::hash(&chunk_data);
            if actual_hash.as_bytes() != chunk_hash.0.as_slice() {
                return Err(RecoveryError::HashMismatch {
                    expected: hash_hex,
                    actual: hex::encode(actual_hash.as_bytes()),
                });
            }

            file_data.extend_from_slice(&chunk_data);
        }

        Ok(RecoveredFile {
            path: entry.path.clone(),
            size: file_data.len() as u64,
            data: file_data,
            chunk_hashes,
        })
    }

    pub fn verify_sealed(&self, snapshot_id: Uuid) -> Result<bool, RecoveryError> {
        let snapshot = self.snapshot_store.read_snapshot(self.repo_id, snapshot_id)
            .map_err(|_| RecoveryError::SnapshotNotFound(snapshot_id))?;
        Ok(snapshot.status == SnapshotStatus::Sealed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hbx_core::domain::chunk::ChunkHash;
    use badou_engine::domain::snapshot::Snapshot;
    use badou_engine::format::BadouDataLayout;
    use badou_engine::domain::snapshot::{SourceMachine, FileTree, VerifyInfo};
    use badou_index::BadouIndex;
    use chrono::Utc;

    struct TestEnv {
        chunk_store: ChunkStore,
        manifest_store: ManifestStore,
        snapshot_store: SnapshotStore,
        repo_id: RepositoryId,
    }

    impl TestEnv {
        fn new() -> Self {
            let tmp = tempfile::tempdir().unwrap();
            let layout = BadouDataLayout::new(tmp.path());
            let repo_id = RepositoryId(Uuid::new_v4());
            std::fs::create_dir_all(layout.chunks_dir(&repo_id)).unwrap();
            std::fs::create_dir_all(layout.manifests_dir(&repo_id)).unwrap();
            std::fs::create_dir_all(layout.snapshots_dir(&repo_id)).unwrap();

            let index = BadouIndex::in_memory();
            let chunk_store = ChunkStore::new(layout.clone(), index);
            let manifest_store = ManifestStore::new(layout.clone());
            let snapshot_store = SnapshotStore::new(layout);

            std::mem::forget(tmp);
            Self { chunk_store, manifest_store, snapshot_store, repo_id }
        }

        fn recovery_engine(&self) -> RecoveryEngine<'_> {
            RecoveryEngine::new(&self.repo_id, &self.chunk_store, &self.manifest_store, &self.snapshot_store)
        }
    }

    fn make_hash(data: &[u8]) -> ChunkHash {
        let h = blake3::hash(data);
        let mut arr = [0u8; 32];
        arr.copy_from_slice(h.as_bytes());
        ChunkHash(arr)
    }

    fn make_sealed_snapshot(file_entries: Vec<FileEntry>) -> Snapshot {
        let mut snap = Snapshot::new(
            Uuid::new_v4(),
            VersionId(Uuid::new_v4()),
            SourceMachine {
                hostname: "test".to_string(),
                os_type: "linux".to_string(),
                agent_version: "0.1.0".to_string(),
            },
        );
        snap.file_tree = FileTree {
            root: "/data".to_string(),
            entries: file_entries,
        };
        snap.status = SnapshotStatus::Sealed;
        snap.verify_info = VerifyInfo {
            verified: true,
            verified_at: Some(Utc::now()),
            checksum: Some("abc".to_string()),
        };
        snap
    }

    #[test]
    fn recover_sealed_snapshot_succeeds() {
        let env = TestEnv::new();
        let data = b"file content";
        let hash = make_hash(data);
        env.chunk_store.write_chunk(&env.repo_id, &hash, data).unwrap();

        let entry = FileEntry {
            path: "/data/test.txt".to_string(),
            size: data.len() as u64,
            is_directory: false,
            chunk_hashes: vec![hash],
        };
        let snapshot = make_sealed_snapshot(vec![entry.clone()]);
        env.snapshot_store.write_snapshot(&env.repo_id, &snapshot).unwrap();

        let engine = env.recovery_engine();
        let request = RecoveryRequest {
            snapshot_id: snapshot.snapshot_id,
            file_filter: None,
        };
        let result = engine.recover(&request).unwrap();
        assert_eq!(result.recovered_files.len(), 1);
        assert_eq!(result.recovered_files[0].data, data);
        assert_eq!(result.total_bytes, data.len() as u64);
    }

    #[test]
    fn recover_non_sealed_fails() {
        let env = TestEnv::new();
        let mut snapshot = make_sealed_snapshot(vec![]);
        snapshot.status = SnapshotStatus::Writing;
        env.snapshot_store.write_snapshot(&env.repo_id, &snapshot).unwrap();

        let engine = env.recovery_engine();
        let request = RecoveryRequest {
            snapshot_id: snapshot.snapshot_id,
            file_filter: None,
        };
        let result = engine.recover(&request);
        assert!(result.is_err());
    }

    #[test]
    fn recover_corrupted_fails() {
        let env = TestEnv::new();
        let mut snapshot = make_sealed_snapshot(vec![]);
        snapshot.status = SnapshotStatus::Corrupt;
        env.snapshot_store.write_snapshot(&env.repo_id, &snapshot).unwrap();

        let engine = env.recovery_engine();
        let request = RecoveryRequest {
            snapshot_id: snapshot.snapshot_id,
            file_filter: None,
        };
        let result = engine.recover(&request);
        assert!(result.is_err());
    }

    #[test]
    fn recover_with_file_filter() {
        let env = TestEnv::new();
        let data1 = b"file one";
        let data2 = b"file two";
        let hash1 = make_hash(data1);
        let hash2 = make_hash(data2);
        env.chunk_store.write_chunk(&env.repo_id, &hash1, data1).unwrap();
        env.chunk_store.write_chunk(&env.repo_id, &hash2, data2).unwrap();

        let entries = vec![
            FileEntry {
                path: "/data/file1.txt".to_string(),
                size: data1.len() as u64,
                is_directory: false,
                chunk_hashes: vec![hash1],
            },
            FileEntry {
                path: "/data/file2.txt".to_string(),
                size: data2.len() as u64,
                is_directory: false,
                chunk_hashes: vec![hash2],
            },
        ];
        let snapshot = make_sealed_snapshot(entries);
        env.snapshot_store.write_snapshot(&env.repo_id, &snapshot).unwrap();

        let engine = env.recovery_engine();
        let request = RecoveryRequest {
            snapshot_id: snapshot.snapshot_id,
            file_filter: Some(vec!["/data/file1.txt".to_string()]),
        };
        let result = engine.recover(&request).unwrap();
        assert_eq!(result.recovered_files.len(), 1);
        assert_eq!(result.recovered_files[0].path, "/data/file1.txt");
    }

    #[test]
    fn recover_missing_chunk_partial_failure() {
        let env = TestEnv::new();
        let data1 = b"present file";
        let hash1 = make_hash(data1);
        let hash2 = ChunkHash([0xff; 32]);
        env.chunk_store.write_chunk(&env.repo_id, &hash1, data1).unwrap();

        let entries = vec![
            FileEntry {
                path: "/data/present.txt".to_string(),
                size: data1.len() as u64,
                is_directory: false,
                chunk_hashes: vec![hash1],
            },
            FileEntry {
                path: "/data/missing.txt".to_string(),
                size: 100,
                is_directory: false,
                chunk_hashes: vec![hash2],
            },
        ];
        let snapshot = make_sealed_snapshot(entries);
        env.snapshot_store.write_snapshot(&env.repo_id, &snapshot).unwrap();

        let engine = env.recovery_engine();
        let request = RecoveryRequest {
            snapshot_id: snapshot.snapshot_id,
            file_filter: None,
        };
        let result = engine.recover(&request).unwrap();
        assert_eq!(result.recovered_files.len(), 1);
        assert_eq!(result.failed_files.len(), 1);
    }

    #[test]
    fn verify_sealed_check() {
        let env = TestEnv::new();
        let snapshot = make_sealed_snapshot(vec![]);
        env.snapshot_store.write_snapshot(&env.repo_id, &snapshot).unwrap();

        let engine = env.recovery_engine();
        assert!(engine.verify_sealed(snapshot.snapshot_id).unwrap());
    }
}
