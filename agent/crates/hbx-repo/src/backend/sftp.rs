use std::time::Duration;

use hbx_core::domain::chunk::{ChunkHash, ChunkLocation};
use hbx_core::domain::common::{LockOperation, RepoLock, VersionId, VersionSummary};
use hbx_core::domain::encryption::EncryptedChunk;
use hbx_core::domain::repository::Manifest;
use hbx_core::pipeline::{IBackupRepository, RepoError};
use parking_lot::Mutex;

use super::config::SftpConfig;

pub struct SftpCredentials {
    pub username: String,
    pub key_path: Option<String>,
    pub password: Option<String>,
}

pub struct SftpRepository {
    config: SftpConfig,
    credentials: SftpCredentials,
    _lock: Mutex<()>,
}

impl SftpRepository {
    pub fn new(config: SftpConfig, credentials: SftpCredentials) -> Self {
        Self {
            config,
            credentials,
            _lock: Mutex::new(()),
        }
    }

    fn not_implemented(&self, op: &str) -> RepoError {
        RepoError::Failed(format!(
            "SFTP {} not yet implemented (host={}:{}, user={})",
            op, self.config.host, self.config.port, self.credentials.username
        ))
    }
}

impl IBackupRepository for SftpRepository {
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
    fn test_sftp_config_creation() {
        let config = SftpConfig {
            host: "sftp.example.com".to_string(),
            port: 22,
            base_path: "/backup".to_string(),
        };
        let creds = SftpCredentials {
            username: "user".to_string(),
            key_path: None,
            password: Some("pass".to_string()),
        };
        let repo = SftpRepository::new(config, creds);
        assert_eq!(repo.config.host, "sftp.example.com");
        assert_eq!(repo.config.port, 22);
    }
}