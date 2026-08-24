use std::collections::HashSet;
use std::time::Duration;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use hbx_core::domain::chunk::ChunkHash;
use hbx_core::domain::common::VersionId;
use hbx_core::pipeline::traits::{
    IBackupRepository, RepoError, RetentionDecision,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CleanupPhase {
    NotStarted,
    Phase1DeletingVersions,
    Phase2ReclaimingChunks,
    Completed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CleanupProgress {
    pub cleanup_id: Uuid,
    pub phase: CleanupPhase,
    pub versions_deleted: u32,
    pub chunks_reclaimed: u32,
    pub started_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Default for CleanupProgress {
    fn default() -> Self {
        Self::new()
    }
}

impl CleanupProgress {
    pub fn new() -> Self {
        let now = Utc::now();
        Self {
            cleanup_id: Uuid::new_v4(),
            phase: CleanupPhase::NotStarted,
            versions_deleted: 0,
            chunks_reclaimed: 0,
            started_at: now,
            updated_at: now,
        }
    }

    fn set_phase(&mut self, phase: CleanupPhase) {
        self.phase = phase;
        self.updated_at = Utc::now();
    }
}

#[derive(Debug, Clone)]
pub struct CleanupResult {
    pub deleted_versions: Vec<VersionId>,
    pub reclaimed_chunks: Vec<ChunkHash>,
    pub skipped_in_use: Vec<VersionId>,
}

pub struct CleanupExecutor<'a> {
    repo: &'a dyn IBackupRepository,
    in_use_versions: HashSet<VersionId>,
}

impl<'a> CleanupExecutor<'a> {
    pub fn new(repo: &'a dyn IBackupRepository) -> Self {
        Self {
            repo,
            in_use_versions: HashSet::new(),
        }
    }

    pub fn with_in_use_versions(mut self, versions: Vec<VersionId>) -> Self {
        self.in_use_versions = versions.into_iter().collect();
        self
    }

    pub fn execute(
        &self,
        decision: &RetentionDecision,
    ) -> Result<CleanupResult, RepoError> {
        let mut progress = CleanupProgress::new();
        self.execute_with_progress(decision, &mut progress)
    }

    pub fn execute_with_progress(
        &self,
        decision: &RetentionDecision,
        progress: &mut CleanupProgress,
    ) -> Result<CleanupResult, RepoError> {
        let _lock = self
            .repo
            .acquire_lock(
                hbx_core::domain::common::LockOperation::Compact,
                Duration::from_secs(300),
            )
            .map_err(|e| RepoError::Failed(format!("lock error: {e}")))?;

        let mut to_delete = Vec::new();
        let mut skipped = Vec::new();
        for vid in &decision.delete {
            if self.in_use_versions.contains(vid) {
                skipped.push(vid.clone());
            } else {
                to_delete.push(vid.clone());
            }
        }

        progress.set_phase(CleanupPhase::Phase1DeletingVersions);

        let mut chunks_in_deleted: HashSet<ChunkHash> = HashSet::new();
        for vid in &to_delete {
            if let Ok(manifest) = self.repo.read_manifest(vid) {
                for entry in &manifest.chunk_refs {
                    chunks_in_deleted.insert(entry.hash.clone());
                }
            }
        }

        let mut remaining_chunks: HashSet<ChunkHash> = HashSet::new();
        for vid in &decision.keep {
            if let Ok(manifest) = self.repo.read_manifest(vid) {
                for entry in &manifest.chunk_refs {
                    remaining_chunks.insert(entry.hash.clone());
                }
            }
        }

        let orphaned: Vec<ChunkHash> = chunks_in_deleted
            .iter()
            .filter(|h| !remaining_chunks.contains(h))
            .cloned()
            .collect();

        progress.versions_deleted = to_delete.len() as u32;
        progress.set_phase(CleanupPhase::Phase2ReclaimingChunks);

        let mut reclaimed = Vec::new();
        for chunk_hash in &orphaned {
            if let Ok(loc) = self.repo.find_chunk(chunk_hash) {
                if self.repo.delete_chunk(&loc).is_ok() {
                    reclaimed.push(chunk_hash.clone());
                }
            }
        }

        progress.chunks_reclaimed = reclaimed.len() as u32;
        progress.set_phase(CleanupPhase::Completed);

        Ok(CleanupResult {
            deleted_versions: to_delete,
            reclaimed_chunks: reclaimed,
            skipped_in_use: skipped,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hbx_core::domain::chunk::ChunkLocation;
    use hbx_core::domain::common::{RepoLock, VersionSummary};
    use hbx_core::domain::encryption::EncryptedChunk;
    use hbx_core::domain::repository::Manifest;
    use hbx_core::pipeline::traits::RepoError;
    use std::collections::HashMap;

    struct MockRepo {
        manifests: HashMap<VersionId, Manifest>,
        #[allow(dead_code)]
        chunks: HashMap<ChunkHash, ChunkLocation>,
        lock_ok: bool,
    }

    impl MockRepo {
        fn new() -> Self {
            Self {
                manifests: HashMap::new(),
                chunks: HashMap::new(),
                lock_ok: true,
            }
        }
    }

    impl IBackupRepository for MockRepo {
        fn write_chunk(
            &self,
            _hash: &ChunkHash,
            _encrypted: &EncryptedChunk,
        ) -> Result<ChunkLocation, RepoError> {
            Err(RepoError::Failed("not implemented".into()))
        }

        fn read_chunk(
            &self,
            _location: &ChunkLocation,
        ) -> Result<EncryptedChunk, RepoError> {
            Err(RepoError::Failed("not implemented".into()))
        }

        fn chunk_exists(&self, _hash: &ChunkHash) -> Result<bool, RepoError> {
            Ok(false)
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

        fn read_manifest(
            &self,
            version_id: &VersionId,
        ) -> Result<Manifest, RepoError> {
            self.manifests
                .get(version_id)
                .cloned()
                .ok_or_else(|| RepoError::Failed("not found".into()))
        }

        fn list_versions(&self) -> Result<Vec<VersionSummary>, RepoError> {
            Ok(Vec::new())
        }

        fn acquire_lock(
            &self,
            _operation: hbx_core::domain::common::LockOperation,
            _timeout: Duration,
        ) -> Result<RepoLock, RepoError> {
            if self.lock_ok {
                Ok(RepoLock {
                    lock_id: Uuid::new_v4(),
                    holder: "test".into(),
                    acquired_at: Utc::now(),
                    ttl: Duration::from_secs(300),
                })
            } else {
                Err(RepoError::Failed("locked".into()))
            }
        }
    }

    #[test]
    fn test_cleanup_progress_new() {
        let p = CleanupProgress::new();
        assert_eq!(p.phase, CleanupPhase::NotStarted);
        assert_eq!(p.versions_deleted, 0);
        assert_eq!(p.chunks_reclaimed, 0);
    }

    #[test]
    fn test_cleanup_empty_decision() {
        let repo = MockRepo::new();
        let executor = CleanupExecutor::new(&repo);
        let decision = RetentionDecision {
            keep: vec![],
            delete: vec![],
        };
        let result = executor.execute(&decision).unwrap();
        assert_eq!(result.deleted_versions.len(), 0);
        assert_eq!(result.reclaimed_chunks.len(), 0);
        assert_eq!(result.skipped_in_use.len(), 0);
    }

    #[test]
    fn test_cleanup_skips_in_use_versions() {
        let repo = MockRepo::new();
        let vid_in_use = VersionId(Uuid::new_v4());
        let executor = CleanupExecutor::new(&repo).with_in_use_versions(vec![vid_in_use.clone()]);
        let decision = RetentionDecision {
            keep: vec![],
            delete: vec![vid_in_use],
        };
        let result = executor.execute(&decision).unwrap();
        assert_eq!(result.skipped_in_use.len(), 1);
        assert_eq!(result.deleted_versions.len(), 0);
    }

    #[test]
    fn test_cleanup_phase_progression() {
        let repo = MockRepo::new();
        let executor = CleanupExecutor::new(&repo);
        let decision = RetentionDecision {
            keep: vec![],
            delete: vec![],
        };
        let mut progress = CleanupProgress::new();
        executor
            .execute_with_progress(&decision, &mut progress)
            .unwrap();
        assert_eq!(progress.phase, CleanupPhase::Completed);
    }
}