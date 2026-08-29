use std::time::Duration;

use hbx_core::domain::chunk::{ChunkHash, ChunkLocation};
use hbx_core::domain::common::{LockOperation, RepoLock, VersionId, VersionSummary};
use hbx_core::domain::encryption::EncryptedChunk;
use hbx_core::domain::repository::{ConnectionTestResult, Manifest, ObjectListPage, PageToken};
use hbx_core::pipeline::{IBackupRepository, IBackupRepositoryExt, RepoError};
use parking_lot::Mutex;

use super::config::FtpsConfig;

pub struct FtpsCredentials {
    pub username: String,
    pub password: String,
}

pub struct FtpsRepository {
    config: FtpsConfig,
    credentials: FtpsCredentials,
    _lock: Mutex<()>,
}

impl FtpsRepository {
    pub fn new(config: FtpsConfig, credentials: FtpsCredentials) -> Self {
        Self {
            config,
            credentials,
            _lock: Mutex::new(()),
        }
    }

    fn not_implemented(&self, op: &str) -> RepoError {
        let _ = &self.credentials;
        RepoError::Failed(format!(
            "FTPS {} not yet implemented (host={}:{}, implicit_tls={})",
            op, self.config.host, self.config.port, self.config.implicit_tls
        ))
    }
}

impl IBackupRepository for FtpsRepository {
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

impl IBackupRepositoryExt for FtpsRepository {
    fn test_connection(&self) -> Result<ConnectionTestResult, RepoError> {
        Ok(ConnectionTestResult::NotSupported)
    }

    fn list_objects(
        &self,
        _prefix: &str,
        _page_token: Option<&PageToken>,
        _max_keys: u32,
    ) -> Result<ObjectListPage, RepoError> {
        Err(self.not_implemented("list_objects"))
    }

    fn delete_object(&self, _key: &str) -> Result<(), RepoError> {
        Err(self.not_implemented("delete_object"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ftps_config_creation() {
        let config = FtpsConfig {
            host: "ftps.example.com".to_string(),
            port: 990,
            base_path: "/backup".to_string(),
            implicit_tls: true,
        };
        let creds = FtpsCredentials {
            username: "user".to_string(),
            password: "pass".to_string(),
        };
        let repo = FtpsRepository::new(config, creds);
        assert_eq!(repo.config.host, "ftps.example.com");
        assert_eq!(repo.config.port, 990);
        assert!(repo.config.implicit_tls);
    }

    #[test]
    fn test_ftps_explicit_tls_config() {
        let config = FtpsConfig {
            host: "ftp.example.com".to_string(),
            port: 21,
            base_path: "/backup".to_string(),
            implicit_tls: false,
        };
        let creds = FtpsCredentials {
            username: "user".to_string(),
            password: "pass".to_string(),
        };
        let repo = FtpsRepository::new(config, creds);
        assert!(!repo.config.implicit_tls);
    }

    #[test]
    fn test_ftps_ext_test_connection() {
        let config = FtpsConfig {
            host: "ftps.example.com".to_string(),
            port: 990,
            base_path: "/backup".to_string(),
            implicit_tls: true,
        };
        let creds = FtpsCredentials {
            username: "user".to_string(),
            password: "pass".to_string(),
        };
        let repo = FtpsRepository::new(config, creds);
        let result = repo.test_connection().unwrap();
        assert_eq!(result, ConnectionTestResult::NotSupported);
    }
}