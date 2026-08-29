use std::collections::HashSet;

use hbx_core::domain::chunk::ChunkHash;
use hbx_core::domain::common::VersionId;
use hbx_core::pipeline::{IBackupRepository, RepoError};

#[derive(Debug, Clone)]
pub struct ConsistencyReport {
    pub checked_versions: Vec<VersionId>,
    pub healthy_versions: Vec<VersionId>,
    pub incomplete_versions: Vec<VersionId>,
    pub orphan_chunks: Vec<ChunkHash>,
    pub missing_chunks: Vec<(VersionId, ChunkHash)>,
}

impl ConsistencyReport {
    pub fn is_consistent(&self) -> bool {
        self.incomplete_versions.is_empty()
            && self.orphan_chunks.is_empty()
            && self.missing_chunks.is_empty()
    }

    pub fn total_issues(&self) -> usize {
        self.incomplete_versions.len() + self.orphan_chunks.len() + self.missing_chunks.len()
    }
}

#[derive(Debug, Clone)]
pub struct RepairResult {
    pub orphan_chunks_deleted: u32,
    pub orphan_delete_failures: u32,
    pub versions_quarantined: u32,
}

pub struct ConsistencyChecker;

impl ConsistencyChecker {
    pub fn new() -> Self {
        Self
    }

    pub fn check(
        &self,
        repo: &dyn IBackupRepository,
        candidate_orphans: &[ChunkHash],
    ) -> Result<ConsistencyReport, RepoError> {
        let versions = repo.list_versions()?;
        let mut checked_versions = Vec::new();
        let mut healthy_versions = Vec::new();
        let mut incomplete_versions = Vec::new();
        let mut missing_chunks: Vec<(VersionId, ChunkHash)> = Vec::new();
        let mut all_referenced: HashSet<ChunkHash> = HashSet::new();

        for summary in &versions {
            let version_id = VersionId(summary.version_id);
            checked_versions.push(version_id.clone());

            let manifest = match repo.read_manifest(&version_id) {
                Ok(m) => m,
                Err(e) => {
                    tracing::warn!(version_id = ?version_id, error = %e, "failed to read manifest, marking incomplete");
                    incomplete_versions.push(version_id);
                    continue;
                }
            };

            let mut version_has_missing = false;
            for chunk_ref in &manifest.chunk_refs {
                all_referenced.insert(chunk_ref.hash.clone());
                if !repo.chunk_exists(&chunk_ref.hash)? {
                    missing_chunks.push((version_id.clone(), chunk_ref.hash.clone()));
                    version_has_missing = true;
                }
            }

            if version_has_missing {
                incomplete_versions.push(version_id);
            } else {
                healthy_versions.push(version_id);
            }
        }

        let orphan_chunks: Vec<ChunkHash> = candidate_orphans
            .iter()
            .filter(|h| !all_referenced.contains(h) && repo.chunk_exists(h).unwrap_or(false))
            .cloned()
            .collect();

        Ok(ConsistencyReport {
            checked_versions,
            healthy_versions,
            incomplete_versions,
            orphan_chunks,
            missing_chunks,
        })
    }

    pub fn repair(
        &self,
        repo: &dyn IBackupRepository,
        report: &ConsistencyReport,
    ) -> Result<RepairResult, RepoError> {
        let mut orphan_chunks_deleted = 0;
        let mut orphan_delete_failures = 0;

        for hash in &report.orphan_chunks {
            match repo.find_chunk(hash) {
                Ok(location) => match repo.delete_chunk(&location) {
                    Ok(()) => orphan_chunks_deleted += 1,
                    Err(e) => {
                        orphan_delete_failures += 1;
                        tracing::warn!(hash = ?hash, error = %e, "failed to delete orphan chunk");
                    }
                },
                Err(e) => {
                    orphan_delete_failures += 1;
                    tracing::warn!(hash = ?hash, error = %e, "orphan chunk location not found");
                }
            }
        }

        let versions_quarantined = report.incomplete_versions.len() as u32;

        tracing::info!(
            deleted = orphan_chunks_deleted,
            failures = orphan_delete_failures,
            quarantined = versions_quarantined,
            "consistency repair completed"
        );

        Ok(RepairResult {
            orphan_chunks_deleted,
            orphan_delete_failures,
            versions_quarantined,
        })
    }

    pub fn check_and_repair(
        &self,
        repo: &dyn IBackupRepository,
        candidate_orphans: &[ChunkHash],
    ) -> Result<(ConsistencyReport, RepairResult), RepoError> {
        let report = self.check(repo, candidate_orphans)?;
        let repair_result = self.repair(repo, &report)?;
        Ok((report, repair_result))
    }
}

impl Default for ConsistencyChecker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hbx_core::domain::chunk::ChunkLocation;
    use hbx_core::domain::common::{RepoLock, VersionSummary};
    use hbx_core::domain::encryption::EncryptedChunk;
    use hbx_core::domain::repository::Manifest;
    use std::collections::HashMap;
    use std::time::Duration;

    struct MockRepo {
        manifests: HashMap<VersionId, Manifest>,
        chunks: HashSet<ChunkHash>,
        orphan_chunks: HashSet<ChunkHash>,
    }

    impl MockRepo {
        fn new() -> Self {
            Self {
                manifests: HashMap::new(),
                chunks: HashSet::new(),
                orphan_chunks: HashSet::new(),
            }
        }

        fn with_version(manifest: Manifest, chunks: Vec<ChunkHash>) -> Self {
            let mut repo = Self::new();
            let vid = manifest.version_id.clone();
            repo.manifests.insert(vid, manifest);
            for c in chunks {
                repo.chunks.insert(c);
            }
            repo
        }
    }

    impl IBackupRepository for MockRepo {
        fn write_chunk(
            &self,
            _hash: &ChunkHash,
            _encrypted: &EncryptedChunk,
        ) -> Result<ChunkLocation, RepoError> {
            Ok(ChunkLocation {
                bucket: "default".into(),
                path: "/mock".into(),
            })
        }

        fn read_chunk(
            &self,
            _location: &ChunkLocation,
        ) -> Result<EncryptedChunk, RepoError> {
            Ok(EncryptedChunk {
                ciphertext: vec![],
                nonce: [0u8; 12],
                auth_tag: [0u8; 16],
            })
        }

        fn chunk_exists(&self, hash: &ChunkHash) -> Result<bool, RepoError> {
            Ok(self.chunks.contains(hash) || self.orphan_chunks.contains(hash))
        }

        fn find_chunk(&self, _hash: &ChunkHash) -> Result<ChunkLocation, RepoError> {
            Ok(ChunkLocation {
                bucket: "default".into(),
                path: "/mock".into(),
            })
        }

        fn delete_chunk(&self, _location: &ChunkLocation) -> Result<(), RepoError> {
            Ok(())
        }

        fn write_manifest(
            &self,
            _version_id: &VersionId,
            _manifest: &Manifest,
        ) -> Result<(), RepoError> {
            Ok(())
        }

        fn read_manifest(&self, version_id: &VersionId) -> Result<Manifest, RepoError> {
            self.manifests
                .get(version_id)
                .cloned()
                .ok_or_else(|| RepoError::NotFound("version not found".into()))
        }

        fn list_versions(&self) -> Result<Vec<VersionSummary>, RepoError> {
            Ok(self
                .manifests
                .keys()
                .map(|vid| VersionSummary {
                    version_id: vid.0,
                    version_number: 1,
                    timestamp: chrono::Utc::now(),
                    backup_type: hbx_core::domain::backup::BackupType::Full,
                    total_size: 0,
                    stored_size: 0,
                })
                .collect())
        }

        fn acquire_lock(
            &self,
            _operation: hbx_core::domain::common::LockOperation,
            _timeout: Duration,
        ) -> Result<RepoLock, RepoError> {
            Ok(RepoLock {
                lock_id: uuid::Uuid::new_v4(),
                holder: "test".into(),
                acquired_at: chrono::Utc::now(),
                ttl: Duration::from_secs(60),
            })
        }
    }

    fn make_manifest(chunk_hashes: Vec<ChunkHash>) -> Manifest {
        Manifest {
            version_id: VersionId(uuid::Uuid::new_v4()),
            timestamp: chrono::Utc::now(),
            parent_version_id: None,
            version_number: 1,
            backup_type: hbx_core::domain::backup::BackupType::Full,
            files: vec![],
            chunk_refs: chunk_hashes
                .into_iter()
                .map(|h| hbx_core::domain::chunk::ChunkReference {
                    hash: h,
                    version_id: VersionId(uuid::Uuid::nil()),
                    file_path: "/test".into(),
                    offset: 0,
                })
                .collect(),
            hashes: hbx_core::domain::repository::ManifestHashes {
                manifest_hash: [0u8; 32],
                file_index_hash: [0u8; 32],
                chunk_index_hash: [0u8; 32],
                repo_hash: [0u8; 32],
            },
            chunk_locations: Default::default(),
        }
    }

    #[test]
    fn test_consistent_repo() {
        let chunk = ChunkHash([1u8; 32]);
        let manifest = make_manifest(vec![chunk.clone()]);
        let repo = MockRepo::with_version(manifest, vec![chunk]);
        let checker = ConsistencyChecker::new();
        let report = checker.check(&repo, &[]).unwrap();

        assert!(report.is_consistent());
        assert_eq!(report.healthy_versions.len(), 1);
        assert!(report.incomplete_versions.is_empty());
        assert!(report.orphan_chunks.is_empty());
        assert!(report.missing_chunks.is_empty());
    }

    #[test]
    fn test_missing_chunk_detected() {
        let chunk = ChunkHash([1u8; 32]);
        let missing_chunk = ChunkHash([2u8; 32]);
        let manifest = make_manifest(vec![chunk.clone(), missing_chunk.clone()]);
        let repo = MockRepo::with_version(manifest, vec![chunk]);
        let checker = ConsistencyChecker::new();
        let report = checker.check(&repo, &[]).unwrap();

        assert!(!report.is_consistent());
        assert_eq!(report.incomplete_versions.len(), 1);
        assert_eq!(report.missing_chunks.len(), 1);
        assert_eq!(report.missing_chunks[0].1, missing_chunk);
    }

    #[test]
    fn test_orphan_chunk_detected() {
        let chunk = ChunkHash([1u8; 32]);
        let orphan = ChunkHash([2u8; 32]);
        let manifest = make_manifest(vec![chunk.clone()]);
        let mut repo = MockRepo::with_version(manifest, vec![chunk]);
        repo.orphan_chunks.insert(orphan.clone());
        let checker = ConsistencyChecker::new();
        let report = checker.check(&repo, &[orphan]).unwrap();

        assert!(!report.is_consistent());
        assert_eq!(report.orphan_chunks.len(), 1);
    }

    #[test]
    fn test_repair_deletes_orphans() {
        let chunk = ChunkHash([1u8; 32]);
        let orphan = ChunkHash([2u8; 32]);
        let manifest = make_manifest(vec![chunk.clone()]);
        let mut repo = MockRepo::with_version(manifest, vec![chunk]);
        repo.orphan_chunks.insert(orphan.clone());
        let checker = ConsistencyChecker::new();
        let report = checker.check(&repo, &[orphan]).unwrap();
        let repair_result = checker.repair(&repo, &report).unwrap();

        assert_eq!(repair_result.orphan_chunks_deleted, 1);
        assert_eq!(repair_result.orphan_delete_failures, 0);
    }

    #[test]
    fn test_empty_repo_is_consistent() {
        let repo = MockRepo::new();
        let checker = ConsistencyChecker::new();
        let report = checker.check(&repo, &[]).unwrap();

        assert!(report.is_consistent());
        assert_eq!(report.total_issues(), 0);
    }

    #[test]
    fn test_check_and_repair() {
        let chunk = ChunkHash([1u8; 32]);
        let orphan = ChunkHash([2u8; 32]);
        let manifest = make_manifest(vec![chunk.clone()]);
        let mut repo = MockRepo::with_version(manifest, vec![chunk]);
        repo.orphan_chunks.insert(orphan.clone());
        let checker = ConsistencyChecker::new();
        let (report, repair) = checker.check_and_repair(&repo, &[orphan]).unwrap();

        assert!(!report.is_consistent());
        assert_eq!(repair.orphan_chunks_deleted, 1);
    }

    #[test]
    fn test_candidate_not_in_repo_not_orphan() {
        let chunk = ChunkHash([1u8; 32]);
        let candidate = ChunkHash([2u8; 32]);
        let manifest = make_manifest(vec![chunk.clone()]);
        let repo = MockRepo::with_version(manifest, vec![chunk]);
        let checker = ConsistencyChecker::new();
        let report = checker.check(&repo, &[candidate]).unwrap();

        assert!(report.orphan_chunks.is_empty());
    }
}
