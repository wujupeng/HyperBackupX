use std::fs;
use std::path::PathBuf;
use std::time::Duration;

use hbx_core::domain::chunk::{ChunkHash, ChunkLocation};
use hbx_core::domain::common::{LockOperation, RepoLock, RepositoryId, VersionId, VersionSummary};
use hbx_core::domain::encryption::EncryptedChunk;
use hbx_core::domain::repository::{Manifest, BackendType};
use hbx_core::pipeline::{IBackupRepository, RepoError};
use parking_lot::Mutex;
use uuid::Uuid;

use crate::format::{
    bucket_name, chunk_filename, decode_encrypted_chunk, encode_encrypted_chunk, manifest_filename,
    RepositoryInitializer,
};

pub struct LocalRepository {
    root: PathBuf,
    _lock: Mutex<()>,
}

impl LocalRepository {
    pub fn open(root: impl Into<PathBuf>) -> Result<Self, RepoError> {
        let root = root.into();
        if !root.join("repository.json").exists() {
            return Err(RepoError::NotFound("repository.json not found".to_string()));
        }
        Ok(Self {
            root,
            _lock: Mutex::new(()),
        })
    }

    pub fn init(
        root: impl Into<PathBuf>,
        repository_id: RepositoryId,
    ) -> Result<Self, RepoError> {
        let root = root.into();
        RepositoryInitializer::new(&root).init(repository_id, BackendType::Local)?;
        Ok(Self {
            root,
            _lock: Mutex::new(()),
        })
    }

    fn chunk_path(&self, hash: &ChunkHash) -> PathBuf {
        let bucket = bucket_name(hash);
        let filename = chunk_filename(hash);
        self.root.join("chunks").join(&bucket).join(&filename)
    }

    fn manifest_path(&self, version_id: &VersionId) -> PathBuf {
        self.root.join("manifests").join(manifest_filename(version_id))
    }

    fn staging_path(&self, version_id: &VersionId) -> PathBuf {
        self.root
            .join("manifests")
            .join(".staging")
            .join(manifest_filename(version_id))
    }

    fn lock_path(&self, lock_id: &Uuid) -> PathBuf {
        self.root.join("locks").join(format!("{}.lock", lock_id))
    }
}

impl IBackupRepository for LocalRepository {
    fn write_chunk(
        &self,
        hash: &ChunkHash,
        encrypted: &EncryptedChunk,
    ) -> Result<ChunkLocation, RepoError> {
        let path = self.chunk_path(hash);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        let data = encode_encrypted_chunk(encrypted);
        fs::write(&path, &data)?;

        Ok(ChunkLocation {
            bucket: bucket_name(hash),
            path: chunk_filename(hash),
        })
    }

    fn read_chunk(&self, location: &ChunkLocation) -> Result<EncryptedChunk, RepoError> {
        let path = self.root.join("chunks").join(&location.bucket).join(&location.path);
        let data = fs::read(&path)?;
        decode_encrypted_chunk(&data).map_err(RepoError::Io)
    }

    fn chunk_exists(&self, hash: &ChunkHash) -> Result<bool, RepoError> {
        let path = self.chunk_path(hash);
        Ok(path.exists())
    }

    fn find_chunk(&self, hash: &ChunkHash) -> Result<ChunkLocation, RepoError> {
        let path = self.chunk_path(hash);
        if !path.exists() {
            return Err(RepoError::Failed("chunk not found".into()));
        }
        Ok(ChunkLocation {
            bucket: bucket_name(hash),
            path: chunk_filename(hash),
        })
    }

    fn delete_chunk(&self, location: &ChunkLocation) -> Result<(), RepoError> {
        let path = self.root.join("chunks").join(&location.bucket).join(&location.path);
        if path.exists() {
            fs::remove_file(&path)?;
        }
        Ok(())
    }

    fn write_manifest(
        &self,
        version_id: &VersionId,
        manifest: &Manifest,
    ) -> Result<(), RepoError> {
        let staging = self.staging_path(version_id);
        let final_path = self.manifest_path(version_id);

        let data = serde_json::to_vec(manifest)?;
        fs::write(&staging, &data)?;

        fs::rename(&staging, &final_path)?;

        Ok(())
    }

    fn read_manifest(&self, version_id: &VersionId) -> Result<Manifest, RepoError> {
        let path = self.manifest_path(version_id);
        let data = fs::read(&path)?;
        let manifest: Manifest = serde_json::from_slice(&data)?;
        Ok(manifest)
    }

    fn list_versions(&self) -> Result<Vec<VersionSummary>, RepoError> {
        let manifests_dir = self.root.join("manifests");
        let mut versions = Vec::new();

        if !manifests_dir.exists() {
            return Ok(versions);
        }

        for entry in fs::read_dir(&manifests_dir)? {
            let entry = entry?;
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            let filename = path.file_name().unwrap().to_string_lossy().to_string();
            if !filename.ends_with(".manifest") {
                continue;
            }

            let id_str = &filename[..filename.len() - ".manifest".len()];
            if let Ok(uuid) = Uuid::parse_str(id_str) {
                let data = fs::read(&path)?;
                if let Ok(manifest) = serde_json::from_slice::<Manifest>(&data) {
                    versions.push(VersionSummary {
                        version_id: uuid,
                        version_number: manifest.version_number,
                        timestamp: manifest.timestamp,
                        backup_type: manifest.backup_type,
                        total_size: 0,
                        stored_size: 0,
                    });
                }
            }
        }

        Ok(versions)
    }

    fn acquire_lock(
        &self,
        operation: LockOperation,
        _timeout: Duration,
    ) -> Result<RepoLock, RepoError> {
        let lock_id = Uuid::new_v4();
        let lock_path = self.lock_path(&lock_id);

        let lock = RepoLock {
            lock_id,
            holder: format!("{:?}", operation),
            acquired_at: chrono::Utc::now(),
            ttl: Duration::from_secs(300),
        };

        let data = serde_json::to_vec(&serde_json::json!({
            "lock_id": lock_id,
            "holder": lock.holder,
            "acquired_at": lock.acquired_at,
            "ttl_secs": lock.ttl.as_secs(),
        }))?;

        fs::write(&lock_path, &data)?;

        Ok(lock)
    }
}

pub mod config;
pub mod s3;
pub mod webdav;
pub mod sftp;
pub mod ftp;
pub mod smb;

#[cfg(test)]
mod tests {
    use super::*;
    use hbx_core::pipeline::IBackupRepository;

    fn setup_repo() -> (tempfile::TempDir, LocalRepository) {
        let tmp = tempfile::tempdir().unwrap();
        let repo = LocalRepository::init(tmp.path(), RepositoryId(Uuid::new_v4())).unwrap();
        (tmp, repo)
    }

    #[test]
    fn test_write_read_chunk() {
        let (_tmp, repo) = setup_repo();
        let hash = ChunkHash([0xab; 32]);
        let encrypted = EncryptedChunk {
            ciphertext: vec![1, 2, 3, 4, 5],
            nonce: [0u8; 12],
            auth_tag: [0xff; 16],
        };

        let location = repo.write_chunk(&hash, &encrypted).unwrap();
        assert_eq!(location.bucket, "ab");

        let read = repo.read_chunk(&location).unwrap();
        assert_eq!(read.ciphertext, encrypted.ciphertext);
        assert_eq!(read.nonce, encrypted.nonce);
        assert_eq!(read.auth_tag, encrypted.auth_tag);
    }

    #[test]
    fn test_chunk_exists() {
        let (_tmp, repo) = setup_repo();
        let hash = ChunkHash([0xcd; 32]);
        let encrypted = EncryptedChunk {
            ciphertext: vec![0],
            nonce: [0u8; 12],
            auth_tag: [0u8; 16],
        };

        assert!(!repo.chunk_exists(&hash).unwrap());
        repo.write_chunk(&hash, &encrypted).unwrap();
        assert!(repo.chunk_exists(&hash).unwrap());
    }

    #[test]
    fn test_delete_chunk() {
        let (_tmp, repo) = setup_repo();
        let hash = ChunkHash([0xef; 32]);
        let encrypted = EncryptedChunk {
            ciphertext: vec![0],
            nonce: [0u8; 12],
            auth_tag: [0u8; 16],
        };

        let location = repo.write_chunk(&hash, &encrypted).unwrap();
        assert!(repo.chunk_exists(&hash).unwrap());

        repo.delete_chunk(&location).unwrap();
        assert!(!repo.chunk_exists(&hash).unwrap());
    }

    #[test]
    fn test_write_read_manifest() {
        let (_tmp, repo) = setup_repo();
        let version_id = VersionId(Uuid::new_v4());
        let manifest = Manifest {
            version_id: version_id.clone(),
            timestamp: chrono::Utc::now(),
            parent_version_id: None,
            version_number: 1,
            backup_type: hbx_core::domain::backup::BackupType::Full,
            files: vec![],
            chunk_refs: vec![],
            hashes: hbx_core::domain::repository::ManifestHashes {
                manifest_hash: [0u8; 32],
                file_index_hash: [0u8; 32],
                chunk_index_hash: [0u8; 32],
                repo_hash: [0u8; 32],
            },
        };

        repo.write_manifest(&version_id, &manifest).unwrap();
        let read = repo.read_manifest(&version_id).unwrap();
        assert_eq!(read.version_id, version_id);
    }

    #[test]
    fn test_list_versions() {
        let (_tmp, repo) = setup_repo();

        for _ in 0..3 {
            let version_id = VersionId(Uuid::new_v4());
            let manifest = Manifest {
                version_id: version_id.clone(),
                timestamp: chrono::Utc::now(),
                parent_version_id: None,
                version_number: 1,
                backup_type: hbx_core::domain::backup::BackupType::Full,
                files: vec![],
                chunk_refs: vec![],
                hashes: hbx_core::domain::repository::ManifestHashes {
                    manifest_hash: [0u8; 32],
                    file_index_hash: [0u8; 32],
                    chunk_index_hash: [0u8; 32],
                    repo_hash: [0u8; 32],
                },
            };
            repo.write_manifest(&version_id, &manifest).unwrap();
        }

        let versions = repo.list_versions().unwrap();
        assert_eq!(versions.len(), 3);
    }

    #[test]
    fn test_acquire_lock() {
        let (_tmp, repo) = setup_repo();
        let lock = repo.acquire_lock(LockOperation::Backup, Duration::from_secs(60)).unwrap();
        assert_eq!(lock.holder, "Backup");
    }
}