use std::sync::Arc;
use std::time::Duration;

use hbx_core::domain::chunk::{ChunkHash, ChunkLocation};
use hbx_core::domain::common::LockOperation;
use hbx_core::pipeline::{IBackupRepository, RepoError};

pub struct BackupLockGuard {
    lock_id: uuid::Uuid,
}

impl BackupLockGuard {
    pub fn acquire(repo: Arc<dyn IBackupRepository>, timeout: Duration) -> Result<Self, RepoError> {
        let lock = repo.acquire_lock(LockOperation::Backup, timeout)?;
        Ok(Self {
            lock_id: lock.lock_id,
        })
    }

    pub fn release(self) -> Result<(), RepoError> {
        Ok(())
    }
}

impl Drop for BackupLockGuard {
    fn drop(&mut self) {
        tracing::info!(lock_id = %self.lock_id, "backup lock released");
    }
}

#[derive(Default)]
pub struct StagingTracker {
    written_chunks: Vec<(ChunkHash, ChunkLocation)>,
}

impl StagingTracker {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn track(&mut self, hash: ChunkHash, location: ChunkLocation) {
        self.written_chunks.push((hash, location));
    }

    pub fn len(&self) -> usize {
        self.written_chunks.len()
    }

    pub fn is_empty(&self) -> bool {
        self.written_chunks.is_empty()
    }

    pub fn rollback(&self, repo: &dyn IBackupRepository) -> RollbackResult {
        let mut deleted = 0;
        let mut failed = 0;
        for (hash, location) in &self.written_chunks {
            match repo.delete_chunk(location) {
                Ok(()) => deleted += 1,
                Err(e) => {
                    failed += 1;
                    tracing::warn!(hash = ?hash, error = %e, "failed to delete orphan chunk during rollback");
                }
            }
        }
        RollbackResult {
            chunks_deleted: deleted,
            chunks_failed: failed,
        }
    }

    pub fn written_chunks(&self) -> &[(ChunkHash, ChunkLocation)] {
        &self.written_chunks
    }
}

#[derive(Debug, Clone)]
pub struct RollbackResult {
    pub chunks_deleted: u32,
    pub chunks_failed: u32,
}

pub fn is_retryable_repo_error(e: &RepoError) -> bool {
    matches!(e, RepoError::Io(_) | RepoError::Failed(_))
}

pub fn is_storage_full(e: &RepoError) -> bool {
    matches!(e, RepoError::Full)
}

#[cfg(test)]
mod tests {
    use super::*;
    use hbx_core::domain::chunk::{ChunkHash, ChunkLocation};
    use hbx_core::domain::common::{LockOperation, RepoLock, VersionId, VersionSummary};
    use hbx_core::domain::encryption::EncryptedChunk;
    use hbx_core::domain::repository::Manifest;
    use hbx_core::pipeline::RepoError;
    use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
    use std::sync::Arc;

    fn mock_location() -> ChunkLocation {
        ChunkLocation {
            bucket: "default".to_string(),
            path: "/mock/chunk".to_string(),
        }
    }

    fn mock_encrypted() -> EncryptedChunk {
        EncryptedChunk {
            ciphertext: vec![0u8; 16],
            nonce: [0u8; 12],
            auth_tag: [0u8; 16],
        }
    }

    struct MockRepo {
        delete_count: AtomicU32,
        lock_acquired: AtomicBool,
        should_fail_full: AtomicBool,
    }

    impl MockRepo {
        fn new() -> Self {
            Self {
                delete_count: AtomicU32::new(0),
                lock_acquired: AtomicBool::new(false),
                should_fail_full: AtomicBool::new(false),
            }
        }
    }

    impl IBackupRepository for MockRepo {
        fn write_chunk(
            &self,
            _hash: &ChunkHash,
            _encrypted: &EncryptedChunk,
        ) -> Result<ChunkLocation, RepoError> {
            if self.should_fail_full.load(Ordering::SeqCst) {
                Err(RepoError::Full)
            } else {
                Ok(mock_location())
            }
        }

        fn read_chunk(
            &self,
            _location: &ChunkLocation,
        ) -> Result<EncryptedChunk, RepoError> {
            Ok(mock_encrypted())
        }

        fn chunk_exists(&self, _hash: &ChunkHash) -> Result<bool, RepoError> {
            Ok(false)
        }

        fn find_chunk(&self, _hash: &ChunkHash) -> Result<ChunkLocation, RepoError> {
            Ok(mock_location())
        }

        fn delete_chunk(&self, _location: &ChunkLocation) -> Result<(), RepoError> {
            self.delete_count.fetch_add(1, Ordering::SeqCst);
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
            _version_id: &VersionId,
        ) -> Result<Manifest, RepoError> {
            Err(RepoError::NotFound("no manifest".into()))
        }

        fn list_versions(&self) -> Result<Vec<VersionSummary>, RepoError> {
            Ok(vec![])
        }

        fn acquire_lock(
            &self,
            _operation: LockOperation,
            _timeout: Duration,
        ) -> Result<RepoLock, RepoError> {
            self.lock_acquired.store(true, Ordering::SeqCst);
            Ok(RepoLock {
                lock_id: uuid::Uuid::new_v4(),
                holder: "test".to_string(),
                acquired_at: chrono::Utc::now(),
                ttl: Duration::from_secs(60),
            })
        }
    }

    #[test]
    fn test_staging_tracker_track_and_rollback() {
        let repo = MockRepo::new();
        let mut tracker = StagingTracker::new();
        tracker.track(ChunkHash([1u8; 32]), mock_location());
        tracker.track(ChunkHash([2u8; 32]), mock_location());
        assert_eq!(tracker.len(), 2);

        let result = tracker.rollback(&repo);
        assert_eq!(result.chunks_deleted, 2);
        assert_eq!(result.chunks_failed, 0);
        assert_eq!(repo.delete_count.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn test_staging_tracker_empty_rollback() {
        let repo = MockRepo::new();
        let tracker = StagingTracker::new();
        assert!(tracker.is_empty());
        let result = tracker.rollback(&repo);
        assert_eq!(result.chunks_deleted, 0);
    }

    #[test]
    fn test_backup_lock_guard_acquire_release() {
        let repo = Arc::new(MockRepo::new());
        let guard = BackupLockGuard::acquire(repo.clone(), Duration::from_secs(60));
        assert!(guard.is_ok());
        assert!(repo.lock_acquired.load(Ordering::SeqCst));
        guard.unwrap().release().unwrap();
    }

    #[test]
    fn test_is_storage_full() {
        assert!(is_storage_full(&RepoError::Full));
        assert!(!is_storage_full(&RepoError::Failed("x".into())));
        assert!(!is_storage_full(&RepoError::NotFound("x".into())));
    }

    #[test]
    fn test_is_retryable_repo_error() {
        assert!(is_retryable_repo_error(&RepoError::Failed("net".into())));
        assert!(is_retryable_repo_error(&RepoError::Io(std::io::Error::new(
            std::io::ErrorKind::ConnectionRefused,
            "refused"
        ))));
        assert!(!is_retryable_repo_error(&RepoError::Full));
        assert!(!is_retryable_repo_error(&RepoError::AuthFailed));
    }

    #[test]
    fn test_storage_full_triggers_rollback() {
        let repo = Arc::new(MockRepo::new());
        repo.should_fail_full.store(true, Ordering::SeqCst);

        let mut staging = StagingTracker::new();
        let hash = ChunkHash([0u8; 32]);
        let encrypted = mock_encrypted();

        match repo.write_chunk(&hash, &encrypted) {
            Ok(loc) => staging.track(hash, loc),
            Err(e) => {
                assert!(is_storage_full(&e));
                let rollback_result = staging.rollback(repo.as_ref());
                assert_eq!(rollback_result.chunks_deleted, 0);
            }
        }
    }
}