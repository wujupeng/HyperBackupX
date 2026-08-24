use std::time::Duration;

use hbx_core::domain::chunk::{ChunkHash, ChunkLocation};
use hbx_core::domain::common::{LockOperation, RepoLock, VersionId, VersionSummary};
use hbx_core::domain::encryption::EncryptedChunk;
use hbx_core::domain::repository::Manifest;
use hbx_core::pipeline::{IBackupRepository, RepoError};
use parking_lot::Mutex;

use super::config::SmbConfig;

pub struct SmbCredentials {
    pub username: String,
    pub password: String,
    pub domain: Option<String>,
}

pub struct SmbRepository {
    config: SmbConfig,
    credentials: SmbCredentials,
    _lock: Mutex<()>,
}

impl SmbRepository {
    pub fn new(config: SmbConfig, credentials: SmbCredentials) -> Self {
        Self {
            config,
            credentials,
            _lock: Mutex::new(()),
        }
    }

    fn not_implemented(&self, op: &str) -> RepoError {
        let _ = &self.credentials;
        RepoError::Failed(format!(
            "SMB {} not yet implemented (host={}, share={})",
            op, self.config.host, self.config.share
        ))
    }
}

impl IBackupRepository for SmbRepository {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_smb_config_creation() {
        let config = SmbConfig {
            host: "smb://server".to_string(),
            share: "backup".to_string(),
            base_path: "/repo".to_string(),
        };
        let creds = SmbCredentials {
            username: "user".to_string(),
            password: "pass".to_string(),
            domain: Some("WORKGROUP".to_string()),
        };
        let repo = SmbRepository::new(config, creds);
        assert_eq!(repo.config.host, "smb://server");
        assert_eq!(repo.config.share, "backup");
    }
}