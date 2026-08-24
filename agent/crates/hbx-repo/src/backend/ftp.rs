use std::time::Duration;

use hbx_core::domain::chunk::{ChunkHash, ChunkLocation};
use hbx_core::domain::common::{LockOperation, RepoLock, VersionId, VersionSummary};
use hbx_core::domain::encryption::EncryptedChunk;
use hbx_core::domain::repository::Manifest;
use hbx_core::pipeline::{IBackupRepository, RepoError};
use parking_lot::Mutex;

use super::config::FtpConfig;

pub struct FtpCredentials {
    pub username: String,
    pub password: String,
}

pub struct FtpRepository {
    config: FtpConfig,
    credentials: FtpCredentials,
    _lock: Mutex<()>,
}

impl FtpRepository {
    pub fn new(config: FtpConfig, credentials: FtpCredentials) -> Self {
        Self {
            config,
            credentials,
            _lock: Mutex::new(()),
        }
    }

    fn not_implemented(&self, op: &str) -> RepoError {
        let _ = &self.credentials;
        RepoError::Failed(format!(
            "FTP {} not yet implemented (host={}:{}, tls={})",
            op, self.config.host, self.config.port, self.config.use_tls
        ))
    }
}

impl IBackupRepository for FtpRepository {
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
    fn test_ftp_config_creation() {
        let config = FtpConfig {
            host: "ftp.example.com".to_string(),
            port: 21,
            base_path: "/backup".to_string(),
            use_tls: false,
        };
        let creds = FtpCredentials {
            username: "anonymous".to_string(),
            password: "pass".to_string(),
        };
        let repo = FtpRepository::new(config, creds);
        assert_eq!(repo.config.host, "ftp.example.com");
    }
}