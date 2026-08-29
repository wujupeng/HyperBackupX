//! 三级完整性校验：Repository/Version/Chunk。

use hbx_core::domain::chunk::ChunkHash;
use hbx_core::domain::common::{RepositoryId, VersionId};
use badou_engine::domain::snapshot::SnapshotStatus;
use badou_index::{BadouIndex, ChunkIndexStatus};
use badou_store::{ChunkStore, ManifestStore, SnapshotStore};
use uuid::Uuid;
use chrono::{DateTime, Utc};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum VerifyError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("chunk store error: {0}")]
    ChunkStore(#[from] badou_store::ChunkStoreError),
    #[error("manifest store error: {0}")]
    ManifestStore(#[from] badou_store::ManifestStoreError),
    #[error("snapshot store error: {0}")]
    SnapshotStore(#[from] badou_store::SnapshotStoreError),
    #[error("index error: {0}")]
    Index(#[from] badou_index::IndexError),
    #[error("version not found: {0:?}")]
    VersionNotFound(VersionId),
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum VerifyStatus {
    Pass,
    Fail,
    Mismatch { expected: String, actual: String },
    Missing { detail: String },
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct VerifyReport {
    pub target: VerifyTarget,
    pub status: VerifyStatus,
    pub checked_at: DateTime<Utc>,
    pub detail: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum VerifyTarget {
    Chunk { hash: String },
    Manifest { manifest_id: Uuid },
    Snapshot { snapshot_id: Uuid },
    Version { version_id: VersionId },
    Repository { repo_id: RepositoryId },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VerifyMode {
    Quick,
    Full,
}

pub struct Verifier<'a> {
    repo_id: &'a RepositoryId,
    chunk_store: &'a ChunkStore,
    manifest_store: &'a ManifestStore,
    snapshot_store: &'a SnapshotStore,
    index: &'a BadouIndex,
}

impl<'a> Verifier<'a> {
    pub fn new(
        repo_id: &'a RepositoryId,
        chunk_store: &'a ChunkStore,
        manifest_store: &'a ManifestStore,
        snapshot_store: &'a SnapshotStore,
        index: &'a BadouIndex,
    ) -> Self {
        Self { repo_id, chunk_store, manifest_store, snapshot_store, index }
    }

    pub fn verify_chunk(&self, chunk_hash: &ChunkHash) -> VerifyReport {
        let hash_hex = hex::encode(chunk_hash.0);
        let target = VerifyTarget::Chunk { hash: hash_hex.clone() };

        match self.chunk_store.read_chunk(self.repo_id, chunk_hash) {
            Ok(data) => {
                let actual = blake3::hash(&data);
                if actual.as_bytes() == chunk_hash.0.as_slice() {
                    VerifyReport {
                        target,
                        status: VerifyStatus::Pass,
                        checked_at: Utc::now(),
                        detail: format!("{} bytes verified", data.len()),
                    }
                } else {
                    VerifyReport {
                        target,
                        status: VerifyStatus::Mismatch {
                            expected: hash_hex,
                            actual: hex::encode(actual.as_bytes()),
                        },
                        checked_at: Utc::now(),
                        detail: "hash mismatch detected".to_string(),
                    }
                }
            }
            Err(badou_store::ChunkStoreError::NotFound(_)) => {
                VerifyReport {
                    target,
                    status: VerifyStatus::Missing { detail: "chunk file not found".to_string() },
                    checked_at: Utc::now(),
                    detail: "chunk not in index or file missing".to_string(),
                }
            }
            Err(e) => {
                VerifyReport {
                    target,
                    status: VerifyStatus::Fail,
                    checked_at: Utc::now(),
                    detail: format!("read error: {}", e),
                }
            }
        }
    }

    pub fn verify_manifest(&self, manifest_id: Uuid) -> Result<VerifyReport, VerifyError> {
        let target = VerifyTarget::Manifest { manifest_id };

        match self.manifest_store.read_manifest(self.repo_id, manifest_id) {
            Ok(manifest) => {
                for chunk_ref in &manifest.chunk_refs {
                    let report = self.verify_chunk(&chunk_ref.chunk_hash);
                    if report.status != VerifyStatus::Pass {
                        return Ok(report);
                    }
                }
                Ok(VerifyReport {
                    target,
                    status: VerifyStatus::Pass,
                    checked_at: Utc::now(),
                    detail: format!("{} chunks verified", manifest.chunk_refs.len()),
                })
            }
            Err(badou_store::ManifestStoreError::NotFound(_)) => {
                Ok(VerifyReport {
                    target,
                    status: VerifyStatus::Missing { detail: "manifest not found".to_string() },
                    checked_at: Utc::now(),
                    detail: "manifest file missing".to_string(),
                })
            }
            Err(e) => {
                Ok(VerifyReport {
                    target,
                    status: VerifyStatus::Fail,
                    checked_at: Utc::now(),
                    detail: format!("read error: {}", e),
                })
            }
        }
    }

    pub fn verify_snapshot(&self, snapshot_id: Uuid) -> Result<VerifyReport, VerifyError> {
        let target = VerifyTarget::Snapshot { snapshot_id };

        match self.snapshot_store.read_snapshot(self.repo_id, snapshot_id) {
            Ok(snapshot) => {
                if !snapshot.verify_info.verified && snapshot.status == SnapshotStatus::Sealed {
                    return Ok(VerifyReport {
                        target,
                        status: VerifyStatus::Missing { detail: "verify_info not set for sealed snapshot".to_string() },
                        checked_at: Utc::now(),
                        detail: "sealed snapshot missing verify_info".to_string(),
                    });
                }

                Ok(VerifyReport {
                    target,
                    status: VerifyStatus::Pass,
                    checked_at: Utc::now(),
                    detail: format!("snapshot status={:?}, {} files, {} chunks",
                        snapshot.status, snapshot.file_count, snapshot.chunk_count),
                })
            }
            Err(badou_store::SnapshotStoreError::NotFound(_)) => {
                Ok(VerifyReport {
                    target,
                    status: VerifyStatus::Missing { detail: "snapshot not found".to_string() },
                    checked_at: Utc::now(),
                    detail: "snapshot file missing".to_string(),
                })
            }
            Err(e) => {
                Ok(VerifyReport {
                    target,
                    status: VerifyStatus::Fail,
                    checked_at: Utc::now(),
                    detail: format!("read error: {}", e),
                })
            }
        }
    }

    pub fn verify_repository(&self) -> Result<Vec<VerifyReport>, VerifyError> {
        let mut reports = Vec::new();
        let candidates = self.index.gc_candidates();
        for (hash_hex, entry) in &candidates {
            let report = VerifyReport {
                target: VerifyTarget::Chunk { hash: hash_hex.clone() },
                status: if entry.status == ChunkIndexStatus::GcPending {
                    VerifyStatus::Pass
                } else {
                    VerifyStatus::Fail
                },
                checked_at: Utc::now(),
                detail: format!("ref_count={}, status={:?}", entry.ref_count, entry.status),
            };
            reports.push(report);
        }
        Ok(reports)
    }

    pub fn verify_ref_count_consistency(&self) -> Result<Vec<VerifyReport>, VerifyError> {
        let mut reports = Vec::new();
        let candidates = self.index.gc_candidates();
        for (hash_hex, entry) in &candidates {
            if entry.ref_count == 0 && entry.status != ChunkIndexStatus::GcPending {
                reports.push(VerifyReport {
                    target: VerifyTarget::Chunk { hash: hash_hex.clone() },
                    status: VerifyStatus::Mismatch {
                        expected: "GcPending".to_string(),
                        actual: format!("{:?}", entry.status),
                    },
                    checked_at: Utc::now(),
                    detail: "zero ref_count but not GcPending".to_string(),
                });
            }
        }
        Ok(reports)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use badou_engine::format::BadouDataLayout;

    struct TestEnv {
        index: BadouIndex,
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
            let chunk_store = ChunkStore::new(layout.clone(), index.clone());
            let manifest_store = ManifestStore::new(layout.clone());
            let snapshot_store = SnapshotStore::new(layout);

            std::mem::forget(tmp);
            Self { index, chunk_store, manifest_store, snapshot_store, repo_id }
        }

        fn verifier(&self) -> Verifier<'_> {
            Verifier::new(&self.repo_id, &self.chunk_store, &self.manifest_store, &self.snapshot_store, &self.index)
        }
    }

    fn make_hash(data: &[u8]) -> ChunkHash {
        let h = blake3::hash(data);
        let mut arr = [0u8; 32];
        arr.copy_from_slice(h.as_bytes());
        ChunkHash(arr)
    }

    #[test]
    fn verify_chunk_pass() {
        let env = TestEnv::new();
        let verifier = env.verifier();
        let data = b"verify test data";
        let hash = make_hash(data);
        env.chunk_store.write_chunk(&env.repo_id, &hash, data).unwrap();
        let report = verifier.verify_chunk(&hash);
        assert_eq!(report.status, VerifyStatus::Pass);
    }

    #[test]
    fn verify_chunk_missing() {
        let env = TestEnv::new();
        let verifier = env.verifier();
        let hash = ChunkHash([0xff; 32]);
        let report = verifier.verify_chunk(&hash);
        assert!(matches!(report.status, VerifyStatus::Missing { .. }));
    }

    #[test]
    fn verify_snapshot_missing() {
        let env = TestEnv::new();
        let verifier = env.verifier();
        let report = verifier.verify_snapshot(Uuid::new_v4()).unwrap();
        assert!(matches!(report.status, VerifyStatus::Missing { .. }));
    }

    #[test]
    fn verify_manifest_missing() {
        let env = TestEnv::new();
        let verifier = env.verifier();
        let report = verifier.verify_manifest(Uuid::new_v4()).unwrap();
        assert!(matches!(report.status, VerifyStatus::Missing { .. }));
    }

    #[test]
    fn verify_repository_returns_reports() {
        let env = TestEnv::new();
        let verifier = env.verifier();
        use badou_index::ChunkIndexEntry;
        use chrono::Utc;
        let entry = ChunkIndexEntry {
            bucket: "ab".to_string(),
            path: "/tmp/test.chunk".to_string(),
            ref_count: 0,
            size: 1024,
            stored_size: 512,
            created_at: Utc::now(),
            status: ChunkIndexStatus::GcPending,
        };
        env.index.register("test_hash", entry).unwrap();
        let reports = verifier.verify_repository().unwrap();
        assert!(!reports.is_empty());
    }

    #[test]
    fn verify_ref_count_consistency_clean() {
        let env = TestEnv::new();
        let verifier = env.verifier();
        let reports = verifier.verify_ref_count_consistency().unwrap();
        assert!(reports.is_empty());
    }
}
