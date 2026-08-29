use std::time::Duration;

use hbx_core::domain::chunk::{ChunkHash, ChunkLocation};
use hbx_core::domain::common::{LockOperation, RepoLock, VersionId, VersionSummary};
use hbx_core::domain::encryption::EncryptedChunk;
use hbx_core::domain::repository::{ConnectionTestResult, Manifest, ObjectListPage, PageToken};
use hbx_core::pipeline::{IBackupRepository, IBackupRepositoryExt, RepoError};
use parking_lot::Mutex;

use super::config::GcsConfig;

pub struct GcsCredentials {
    pub access_key: String,
    pub secret_key: String,
}

pub struct GcsRepository {
    config: GcsConfig,
    credentials: GcsCredentials,
    _lock: Mutex<()>,
}

impl GcsRepository {
    pub fn new(config: GcsConfig, credentials: GcsCredentials) -> Self {
        Self { config, credentials, _lock: Mutex::new(()) }
    }

    fn not_implemented(&self, op: &str) -> RepoError {
        let _ = &self.credentials;
        RepoError::Failed(format!(
            "GCS {} not yet implemented (endpoint={}, bucket={}, s3_compat={})",
            op, self.config.endpoint, self.config.bucket, self.config.use_s3_compat
        ))
    }
}

impl IBackupRepository for GcsRepository {
    fn write_chunk(&self, _: &ChunkHash, _: &EncryptedChunk) -> Result<ChunkLocation, RepoError> { Err(self.not_implemented("write_chunk")) }
    fn read_chunk(&self, _: &ChunkLocation) -> Result<EncryptedChunk, RepoError> { Err(self.not_implemented("read_chunk")) }
    fn chunk_exists(&self, _: &ChunkHash) -> Result<bool, RepoError> { Err(self.not_implemented("chunk_exists")) }
    fn delete_chunk(&self, _: &ChunkLocation) -> Result<(), RepoError> { Err(self.not_implemented("delete_chunk")) }
    fn write_manifest(&self, _: &VersionId, _: &Manifest) -> Result<(), RepoError> { Err(self.not_implemented("write_manifest")) }
    fn read_manifest(&self, _: &VersionId) -> Result<Manifest, RepoError> { Err(self.not_implemented("read_manifest")) }
    fn list_versions(&self) -> Result<Vec<VersionSummary>, RepoError> { Err(self.not_implemented("list_versions")) }
    fn acquire_lock(&self, _: LockOperation, _: Duration) -> Result<RepoLock, RepoError> { Err(self.not_implemented("acquire_lock")) }
}

impl IBackupRepositoryExt for GcsRepository {
    fn test_connection(&self) -> Result<ConnectionTestResult, RepoError> { Ok(ConnectionTestResult::NotSupported) }
    fn list_objects(&self, _: &str, _: Option<&PageToken>, _: u32) -> Result<ObjectListPage, RepoError> { Err(self.not_implemented("list_objects")) }
    fn delete_object(&self, _: &str) -> Result<(), RepoError> { Err(self.not_implemented("delete_object")) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gcs_config_creation() {
        let config = GcsConfig { endpoint: "storage.googleapis.com".to_string(), bucket: "mybucket".to_string(), use_s3_compat: true };
        let creds = GcsCredentials { access_key: "key".to_string(), secret_key: "secret".to_string() };
        let repo = GcsRepository::new(config, creds);
        assert_eq!(repo.config.bucket, "mybucket");
    }
}