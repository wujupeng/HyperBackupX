use std::sync::Arc;
use std::time::Duration;

use hbx_core::domain::chunk::{ChunkHash, ChunkLocation};
use hbx_core::domain::common::{LockOperation, RepoLock, VersionId, VersionSummary};
use hbx_core::domain::encryption::EncryptedChunk;
use hbx_core::domain::repository::Manifest;
use hbx_core::pipeline::{IBackupRepository, RepoError};

pub struct RetryRepository {
    inner: Arc<dyn IBackupRepository>,
    max_retries: u32,
    initial_backoff: Duration,
    max_backoff: Duration,
}

impl RetryRepository {
    pub fn new(inner: Arc<dyn IBackupRepository>) -> Self {
        Self {
            inner,
            max_retries: 5,
            initial_backoff: Duration::from_secs(1),
            max_backoff: Duration::from_secs(30),
        }
    }

    pub fn with_config(
        inner: Arc<dyn IBackupRepository>,
        max_retries: u32,
        initial_backoff: Duration,
        max_backoff: Duration,
    ) -> Self {
        Self {
            inner,
            max_retries,
            initial_backoff,
            max_backoff,
        }
    }

    fn backoff(&self, attempt: u32) -> Duration {
        let millis = self.initial_backoff.as_millis() as u64 * 2u64.pow(attempt);
        Duration::from_millis(millis).min(self.max_backoff)
    }

    fn retry<T, F>(&self, op_name: &str, f: F) -> Result<T, RepoError>
    where
        F: Fn() -> Result<T, RepoError>,
    {
        let mut last_err = None;
        for attempt in 0..=self.max_retries {
            match f() {
                Ok(result) => return Ok(result),
                Err(e) => {
                    let is_retryable = matches!(
                        &e,
                        RepoError::Io(_) | RepoError::Failed(_)
                    );
                    if !is_retryable || attempt == self.max_retries {
                        return Err(e);
                    }
                    tracing::warn!(
                        op = op_name,
                        attempt = attempt + 1,
                        max_retries = self.max_retries,
                        error = %e,
                        "retryable error, will retry after backoff"
                    );
                    last_err = Some(e);
                    std::thread::sleep(self.backoff(attempt));
                }
            }
        }
        Err(last_err.unwrap_or_else(|| RepoError::Failed("retry exhausted".to_string())))
    }
}

impl IBackupRepository for RetryRepository {
    fn write_chunk(
        &self,
        hash: &ChunkHash,
        encrypted: &EncryptedChunk,
    ) -> Result<ChunkLocation, RepoError> {
        self.retry("write_chunk", || self.inner.write_chunk(hash, encrypted))
    }

    fn read_chunk(
        &self,
        location: &ChunkLocation,
    ) -> Result<EncryptedChunk, RepoError> {
        self.retry("read_chunk", || self.inner.read_chunk(location))
    }

    fn chunk_exists(&self, hash: &ChunkHash) -> Result<bool, RepoError> {
        self.retry("chunk_exists", || self.inner.chunk_exists(hash))
    }

    fn find_chunk(&self, hash: &ChunkHash) -> Result<ChunkLocation, RepoError> {
        self.retry("find_chunk", || self.inner.find_chunk(hash))
    }

    fn delete_chunk(&self, location: &ChunkLocation) -> Result<(), RepoError> {
        self.retry("delete_chunk", || self.inner.delete_chunk(location))
    }

    fn write_manifest(
        &self,
        version_id: &VersionId,
        manifest: &Manifest,
    ) -> Result<(), RepoError> {
        self.retry("write_manifest", || {
            self.inner.write_manifest(version_id, manifest)
        })
    }

    fn read_manifest(
        &self,
        version_id: &VersionId,
    ) -> Result<Manifest, RepoError> {
        self.retry("read_manifest", || self.inner.read_manifest(version_id))
    }

    fn list_versions(&self) -> Result<Vec<VersionSummary>, RepoError> {
        self.retry("list_versions", || self.inner.list_versions())
    }

    fn acquire_lock(
        &self,
        operation: LockOperation,
        timeout: Duration,
    ) -> Result<RepoLock, RepoError> {
        self.retry("acquire_lock", || {
            self.inner.acquire_lock(operation, timeout)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hbx_core::domain::chunk::{ChunkHash, ChunkLocation};
    use hbx_core::domain::common::{LockOperation, RepoLock, VersionId, VersionSummary};
    use hbx_core::domain::encryption::EncryptedChunk;
    use hbx_core::domain::repository::{Manifest, ManifestHashes};
    use hbx_core::pipeline::RepoError;
    use std::sync::atomic::{AtomicU32, Ordering};
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

    fn mock_manifest() -> Manifest {
        Manifest {
            version_id: VersionId(uuid::Uuid::nil()),
            timestamp: chrono::Utc::now(),
            parent_version_id: None,
            version_number: 1,
            backup_type: hbx_core::domain::backup::BackupType::Full,
            files: vec![],
            chunk_refs: vec![],
            hashes: ManifestHashes {
                manifest_hash: [0u8; 32],
                file_index_hash: [0u8; 32],
                chunk_index_hash: [0u8; 32],
                repo_hash: [0u8; 32],
            },
            chunk_locations: Default::default(),
        }
    }

    struct MockRepo {
        write_fail_count: AtomicU32,
        read_fail_count: AtomicU32,
    }

    impl MockRepo {
        fn new() -> Self {
            Self {
                write_fail_count: AtomicU32::new(0),
                read_fail_count: AtomicU32::new(0),
            }
        }

        fn with_write_fails(n: u32) -> Self {
            Self {
                write_fail_count: AtomicU32::new(n),
                read_fail_count: AtomicU32::new(0),
            }
        }
    }

    impl IBackupRepository for MockRepo {
        fn write_chunk(
            &self,
            _hash: &ChunkHash,
            _encrypted: &EncryptedChunk,
        ) -> Result<ChunkLocation, RepoError> {
            let prev = self.write_fail_count.fetch_sub(1, Ordering::SeqCst);
            if prev > 0 {
                Err(RepoError::Failed("simulated network error".to_string()))
            } else {
                Ok(mock_location())
            }
        }

        fn read_chunk(
            &self,
            _location: &ChunkLocation,
        ) -> Result<EncryptedChunk, RepoError> {
            let prev = self.read_fail_count.fetch_sub(1, Ordering::SeqCst);
            if prev > 0 {
                Err(RepoError::Failed("simulated read error".to_string()))
            } else {
                Ok(mock_encrypted())
            }
        }

        fn chunk_exists(&self, _hash: &ChunkHash) -> Result<bool, RepoError> {
            Ok(false)
        }

        fn find_chunk(&self, _hash: &ChunkHash) -> Result<ChunkLocation, RepoError> {
            Ok(mock_location())
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
            _version_id: &VersionId,
        ) -> Result<Manifest, RepoError> {
            Ok(mock_manifest())
        }

        fn list_versions(&self) -> Result<Vec<VersionSummary>, RepoError> {
            Ok(vec![])
        }

        fn acquire_lock(
            &self,
            _operation: LockOperation,
            _timeout: Duration,
        ) -> Result<RepoLock, RepoError> {
            Ok(RepoLock {
                lock_id: uuid::Uuid::new_v4(),
                holder: "test".to_string(),
                acquired_at: chrono::Utc::now(),
                ttl: Duration::from_secs(60),
            })
        }
    }

    #[test]
    fn test_retry_success_first_try() {
        let mock = Arc::new(MockRepo::new());
        let retry_repo = RetryRepository::with_config(
            mock,
            3,
            Duration::from_millis(1),
            Duration::from_millis(10),
        );
        let hash = ChunkHash([0u8; 32]);
        let encrypted = mock_encrypted();
        let result = retry_repo.write_chunk(&hash, &encrypted);
        assert!(result.is_ok());
    }

    #[test]
    fn test_retry_success_after_failures() {
        let mock = Arc::new(MockRepo::with_write_fails(2));
        let retry_repo = RetryRepository::with_config(
            mock,
            5,
            Duration::from_millis(1),
            Duration::from_millis(10),
        );
        let hash = ChunkHash([0u8; 32]);
        let encrypted = mock_encrypted();
        let result = retry_repo.write_chunk(&hash, &encrypted);
        assert!(result.is_ok());
    }

    #[test]
    fn test_retry_exhausted() {
        let mock = Arc::new(MockRepo::with_write_fails(10));
        let retry_repo = RetryRepository::with_config(
            mock,
            2,
            Duration::from_millis(1),
            Duration::from_millis(5),
        );
        let hash = ChunkHash([0u8; 32]);
        let encrypted = mock_encrypted();
        let result = retry_repo.write_chunk(&hash, &encrypted);
        assert!(result.is_err());
    }

    #[test]
    fn test_backoff_calculation() {
        let mock = Arc::new(MockRepo::new());
        let retry_repo = RetryRepository::with_config(
            mock,
            5,
            Duration::from_secs(1),
            Duration::from_secs(30),
        );
        assert_eq!(retry_repo.backoff(0), Duration::from_secs(1));
        assert_eq!(retry_repo.backoff(1), Duration::from_secs(2));
        assert_eq!(retry_repo.backoff(2), Duration::from_secs(4));
        assert_eq!(retry_repo.backoff(10), Duration::from_secs(30));
    }
}