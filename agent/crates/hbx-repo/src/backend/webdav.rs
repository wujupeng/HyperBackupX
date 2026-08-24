use std::io::Read;
use std::time::Duration;

use hbx_core::domain::chunk::{ChunkHash, ChunkLocation};
use hbx_core::domain::common::{LockOperation, RepoLock, VersionId, VersionSummary};
use hbx_core::domain::encryption::EncryptedChunk;
use hbx_core::domain::repository::Manifest;
use hbx_core::pipeline::{IBackupRepository, RepoError};
use parking_lot::Mutex;
use uuid::Uuid;

use super::config::WebDavConfig;
use crate::format::{decode_encrypted_chunk, encode_encrypted_chunk};

pub struct WebDavCredentials {
    pub username: String,
    pub password: String,
}

pub struct WebDavRepository {
    config: WebDavConfig,
    credentials: WebDavCredentials,
    _lock: Mutex<()>,
}

impl WebDavRepository {
    pub fn new(config: WebDavConfig, credentials: WebDavCredentials) -> Self {
        Self {
            config,
            credentials,
            _lock: Mutex::new(()),
        }
    }

    fn url(&self, path: &str) -> String {
        let base = self.config.endpoint.trim_end_matches('/');
        let prefix = self.config.base_path.trim_start_matches('/').trim_end_matches('/');
        format!("{base}/{prefix}/{path}")
    }

    fn chunk_path(hash: &ChunkHash) -> String {
        let bucket = format!("{:02x}", hash.0[0]);
        let filename = hex::encode(hash.0) + ".chunk";
        format!("chunks/{bucket}/{filename}")
    }

    fn manifest_path(version_id: &VersionId) -> String {
        format!("manifests/{}.manifest", version_id.0)
    }

    fn lock_path(lock_id: &Uuid) -> String {
        format!("locks/{lock_id}.lock")
    }

    fn auth_header(&self) -> String {
        use base64::Engine;
        let raw = format!("{}:{}", self.credentials.username, self.credentials.password);
        format!("Basic {}", base64::engine::general_purpose::STANDARD.encode(raw))
    }

    fn put(&self, path: &str, data: &[u8]) -> Result<(), RepoError> {
        let url = self.url(path);
        let resp = ureq::put(&url)
            .set("Authorization", &self.auth_header())
            .set("Content-Type", "application/octet-stream")
            .send_bytes(data);
        match resp {
            Ok(_) => Ok(()),
            Err(ureq::Error::Status(code, r)) => {
                let body = r.into_string().unwrap_or_default();
                Err(RepoError::Failed(format!("WebDAV PUT failed: {code}: {body}")))
            }
            Err(e) => Err(RepoError::Failed(format!("WebDAV PUT error: {e}"))),
        }
    }

    fn get(&self, path: &str) -> Result<Vec<u8>, RepoError> {
        let url = self.url(path);
        let resp = ureq::get(&url)
            .set("Authorization", &self.auth_header())
            .call();
        match resp {
            Ok(resp) => {
                let mut buf = Vec::new();
                resp.into_reader()
                    .read_to_end(&mut buf)
                    .map_err(|e| RepoError::Failed(format!("read error: {e}")))?;
                Ok(buf)
            }
            Err(ureq::Error::Status(404, _)) => {
                Err(RepoError::NotFound(format!("WebDAV resource not found: {path}")))
            }
            Err(ureq::Error::Status(code, r)) => {
                let body = r.into_string().unwrap_or_default();
                Err(RepoError::Failed(format!("WebDAV GET failed: {code}: {body}")))
            }
            Err(e) => Err(RepoError::Failed(format!("WebDAV GET error: {e}"))),
        }
    }

    fn head(&self, path: &str) -> Result<bool, RepoError> {
        let url = self.url(path);
        let resp = ureq::head(&url)
            .set("Authorization", &self.auth_header())
            .call();
        match resp {
            Ok(_) => Ok(true),
            Err(ureq::Error::Status(404, _)) => Ok(false),
            Err(ureq::Error::Status(code, r)) => {
                let body = r.into_string().unwrap_or_default();
                Err(RepoError::Failed(format!("WebDAV HEAD failed: {code}: {body}")))
            }
            Err(e) => Err(RepoError::Failed(format!("WebDAV HEAD error: {e}"))),
        }
    }

    fn delete(&self, path: &str) -> Result<(), RepoError> {
        let url = self.url(path);
        let resp = ureq::delete(&url)
            .set("Authorization", &self.auth_header())
            .call();
        match resp {
            Ok(_) => Ok(()),
            Err(ureq::Error::Status(404, _)) => Ok(()),
            Err(ureq::Error::Status(code, r)) => {
                let body = r.into_string().unwrap_or_default();
                Err(RepoError::Failed(format!("WebDAV DELETE failed: {code}: {body}")))
            }
            Err(e) => Err(RepoError::Failed(format!("WebDAV DELETE error: {e}"))),
        }
    }
}

impl IBackupRepository for WebDavRepository {
    fn write_chunk(
        &self,
        hash: &ChunkHash,
        encrypted: &EncryptedChunk,
    ) -> Result<ChunkLocation, RepoError> {
        let path = Self::chunk_path(hash);
        let data = encode_encrypted_chunk(encrypted);
        self.put(&path, &data)?;
        Ok(ChunkLocation {
            bucket: format!("{:02x}", hash.0[0]),
            path: hex::encode(hash.0) + ".chunk",
        })
    }

    fn read_chunk(&self, location: &ChunkLocation) -> Result<EncryptedChunk, RepoError> {
        let path = format!("chunks/{}/{}", location.bucket, location.path);
        let data = self.get(&path)?;
        decode_encrypted_chunk(&data).map_err(RepoError::Io)
    }

    fn chunk_exists(&self, hash: &ChunkHash) -> Result<bool, RepoError> {
        let path = Self::chunk_path(hash);
        self.head(&path)
    }

    fn delete_chunk(&self, location: &ChunkLocation) -> Result<(), RepoError> {
        let path = format!("chunks/{}/{}", location.bucket, location.path);
        self.delete(&path)
    }

    fn write_manifest(&self, version_id: &VersionId, manifest: &Manifest) -> Result<(), RepoError> {
        let path = Self::manifest_path(version_id);
        let data = serde_json::to_vec(manifest)?;
        self.put(&path, &data)
    }

    fn read_manifest(&self, version_id: &VersionId) -> Result<Manifest, RepoError> {
        let path = Self::manifest_path(version_id);
        let data = self.get(&path)?;
        let manifest: Manifest = serde_json::from_slice(&data)?;
        Ok(manifest)
    }

    fn list_versions(&self) -> Result<Vec<VersionSummary>, RepoError> {
        Err(RepoError::Failed(
            "list_versions requires WebDAV PROPFIND, not yet implemented".to_string(),
        ))
    }

    fn acquire_lock(
        &self,
        operation: LockOperation,
        _timeout: Duration,
    ) -> Result<RepoLock, RepoError> {
        let lock_id = Uuid::new_v4();
        let path = Self::lock_path(&lock_id);
        let lock = RepoLock {
            lock_id,
            holder: format!("{:?}", operation),
            acquired_at: chrono::Utc::now(),
            ttl: Duration::from_secs(1800),
        };
        let data = serde_json::to_vec(&serde_json::json!({
            "lock_id": lock_id,
            "holder": lock.holder,
            "acquired_at": lock.acquired_at,
            "ttl_secs": lock.ttl.as_secs(),
        }))?;
        self.put(&path, &data)?;
        Ok(lock)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_webdav_config_creation() {
        let config = WebDavConfig {
            endpoint: "https://dav.example.com".to_string(),
            base_path: "/backup".to_string(),
            use_tls: true,
        };
        let creds = WebDavCredentials {
            username: "user".to_string(),
            password: "pass".to_string(),
        };
        let repo = WebDavRepository::new(config, creds);
        assert_eq!(repo.config.endpoint, "https://dav.example.com");
    }

    #[test]
    fn test_chunk_path_format() {
        let hash = ChunkHash([0xcd; 32]);
        let path = WebDavRepository::chunk_path(&hash);
        assert!(path.starts_with("chunks/cd/"));
        assert!(path.ends_with(".chunk"));
    }

    #[test]
    fn test_auth_header() {
        let config = WebDavConfig {
            endpoint: "https://dav.example.com".to_string(),
            base_path: "/backup".to_string(),
            use_tls: true,
        };
        let creds = WebDavCredentials {
            username: "user".to_string(),
            password: "pass".to_string(),
        };
        let repo = WebDavRepository::new(config, creds);
        let header = repo.auth_header();
        assert!(header.starts_with("Basic "));
    }
}