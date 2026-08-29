use std::time::Duration;

use hbx_core::domain::chunk::{ChunkHash, ChunkLocation};
use hbx_core::domain::common::{LockOperation, RepoLock, VersionId, VersionSummary};
use hbx_core::domain::encryption::EncryptedChunk;
use hbx_core::domain::repository::{ConnectionTestResult, Manifest, ObjectListPage, PageToken};
use hbx_core::pipeline::{IBackupRepository, IBackupRepositoryExt, RepoError};
use parking_lot::Mutex;

use super::config::AzureBlobConfig;

pub struct AzureBlobCredentials {
    pub account_name: String,
    pub account_key: String,
}

pub struct AzureBlobRepository {
    config: AzureBlobConfig,
    credentials: AzureBlobCredentials,
    _lock: Mutex<()>,
}

impl AzureBlobRepository {
    pub fn new(config: AzureBlobConfig, credentials: AzureBlobCredentials) -> Self {
        Self {
            config,
            credentials,
            _lock: Mutex::new(()),
        }
    }

    fn not_implemented(&self, op: &str) -> RepoError {
        let _ = &self.credentials;
        RepoError::Failed(format!(
            "AzureBlob {} not yet implemented (endpoint={}, container={}, s3_compat={})",
            op, self.config.endpoint, self.config.container, self.config.use_s3_compat
        ))
    }
}

impl IBackupRepository for AzureBlobRepository {
    fn write_chunk(&self, _hash: &ChunkHash, _encrypted: &EncryptedChunk) -> Result<ChunkLocation, RepoError> {
        Err(self.not_implemented("write_chunk"))
    }
    fn read_chunk(&self, _location: &ChunkLocation) -> Result<EncryptedChunk, RepoError> {
        Err(self.not_implemented("read_chunk"))
    }
    fn chunk_exists(&self, _hash: &ChunkHash) -> Result<bool, RepoError> {
        Err(self.not_implemented("chunk_exists"))
    }
    fn delete_chunk(&self, _location: &ChunkLocation) -> Result<(), RepoError> {
        Err(self.not_implemented("delete_chunk"))
    }
    fn write_manifest(&self, _version_id: &VersionId, _manifest: &Manifest) -> Result<(), RepoError> {
        Err(self.not_implemented("write_manifest"))
    }
    fn read_manifest(&self, _version_id: &VersionId) -> Result<Manifest, RepoError> {
        Err(self.not_implemented("read_manifest"))
    }
    fn list_versions(&self) -> Result<Vec<VersionSummary>, RepoError> {
        Err(self.not_implemented("list_versions"))
    }
    fn acquire_lock(&self, _operation: LockOperation, _timeout: Duration) -> Result<RepoLock, RepoError> {
        Err(self.not_implemented("acquire_lock"))
    }
}

impl IBackupRepositoryExt for AzureBlobRepository {
    fn test_connection(&self) -> Result<ConnectionTestResult, RepoError> {
        Ok(ConnectionTestResult::NotSupported)
    }
    fn list_objects(&self, _: &str, _: Option<&PageToken>, _: u32) -> Result<ObjectListPage, RepoError> {
        Err(self.not_implemented("list_objects"))
    }
    fn delete_object(&self, _: &str) -> Result<(), RepoError> {
        Err(self.not_implemented("delete_object"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_azure_blob_config_creation() {
        let config = AzureBlobConfig {
            endpoint: "blob.core.windows.net".to_string(),
            container: "mycontainer".to_string(),
            use_s3_compat: true,
        };
        let creds = AzureBlobCredentials {
            account_name: "account".to_string(),
            account_key: "key".to_string(),
        };
        let repo = AzureBlobRepository::new(config, creds);
        assert_eq!(repo.config.container, "mycontainer");
        assert!(repo.config.use_s3_compat);
    }
}