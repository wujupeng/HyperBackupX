use std::fs;

use std::path::{Path, PathBuf};
use std::time::Duration;

use chrono::{DateTime, Utc};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};

use thiserror::Error;
use uuid::Uuid;

use hbx_core::domain::chunk::{ChunkHash, ChunkReference};
use hbx_core::domain::common::{LockOperation, RepoLock};

use crate::CompatRepoMetadata;
use crate::COMPAT_FORMAT_VERSION;
use crate::DUPLICATI_SEMANTIC_VERSION;

#[derive(Debug, Error)]
pub enum CompatRepoError {
    #[error("not found: {0}")]
    NotFound(String),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("serialization error: {0}")]
    Serialize(#[from] serde_json::Error),
    #[error("failed: {0}")]
    Failed(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompatRepoConfig {
    pub root_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompatibilityVersion {
    pub version_id: String,
    pub version_number: u64,
    pub timestamp: DateTime<Utc>,
    pub parent_version_id: Option<String>,
    pub backup_type: String,
    pub file_count: u64,
    pub total_size: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompatibilityVersionSummary {
    pub version_id: String,
    pub version_number: u64,
    pub timestamp: DateTime<Utc>,
    pub backup_type: String,
    pub file_count: u64,
    pub total_size: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompatibilityManifest {
    pub version_id: String,
    pub timestamp: DateTime<Utc>,
    pub parent_version_id: Option<String>,
    pub version_number: u64,
    pub backup_type: String,
    pub files: Vec<CompatFileEntry>,
    pub chunk_refs: Vec<ChunkReference>,
    pub hashes: CompatibilityHashes,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompatFileEntry {
    pub path: String,
    pub size: u64,
    pub modified_at: DateTime<Utc>,
    pub chunks: Vec<ChunkHash>,
    pub file_hash: [u8; 32],
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompatChunkLocation {
    pub bucket: String,
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompatibilityHashes {
    pub manifest_hash: [u8; 32],
    pub file_index_hash: [u8; 32],
    pub chunk_index_hash: [u8; 32],
}

pub struct CompatibleRepository {
    root: PathBuf,
    _lock: Mutex<()>,
}

impl CompatibleRepository {
    pub fn open(root: impl Into<PathBuf>) -> Result<Self, CompatRepoError> {
        let root = root.into();
        let metadata_path = root.join("compat_repository.json");
        if !metadata_path.exists() {
            return Err(CompatRepoError::NotFound(
                "compat_repository.json not found".to_string(),
            ));
        }
        Ok(Self {
            root,
            _lock: Mutex::new(()),
        })
    }

    pub fn init(
        root: impl Into<PathBuf>,
        repository_id: String,
    ) -> Result<Self, CompatRepoError> {
        let root = root.into();

        for dir in &["config", "dblocks", "dlists", "dindex", "deleteq", "locks"] {
            fs::create_dir_all(root.join(dir))?;
        }

        for i in 0u8..=255u8 {
            let bucket = format!("{:02x}", i);
            fs::create_dir_all(root.join("dblocks").join(&bucket))?;
        }

        let compat_dir = root.join(".hbx-compat");
        fs::create_dir_all(&compat_dir)?;
        fs::write(
            compat_dir.join("format_version"),
            COMPAT_FORMAT_VERSION.to_string(),
        )?;

        let metadata = CompatRepoMetadata {
            repository_id,
            format_version: COMPAT_FORMAT_VERSION,
            duplicati_semantic_version: DUPLICATI_SEMANTIC_VERSION.to_string(),
            created_at: Utc::now(),
        };
        fs::write(
            root.join("compat_repository.json"),
            serde_json::to_vec_pretty(&metadata)?,
        )?;

        Ok(Self {
            root,
            _lock: Mutex::new(()),
        })
    }

    fn chunk_path(&self, hash: &ChunkHash) -> PathBuf {
        let bucket = format!("{:02x}", hash.0[0]);
        let filename = format!("{}.dblock", hex::encode(hash.0));
        self.root.join("dblocks").join(&bucket).join(&filename)
    }

    fn manifest_path(&self, version_id: &str) -> PathBuf {
        self.root.join("dlists").join(format!("{}.dlist", version_id))
    }

    fn staging_path(&self, version_id: &str) -> PathBuf {
        self.root
            .join("dlists")
            .join(".staging")
            .join(format!("{}.dlist", version_id))
    }

    fn lock_path(&self, lock_id: &Uuid) -> PathBuf {
        self.root.join("locks").join(format!("{}.lock", lock_id))
    }

    pub fn write_compat_chunk(
        &self,
        hash: &ChunkHash,
        data: &[u8],
    ) -> Result<CompatChunkLocation, CompatRepoError> {
        let path = self.chunk_path(hash);
        if path.exists() {
            return Ok(CompatChunkLocation {
                bucket: format!("{:02x}", hash.0[0]),
                path: format!("{}.dblock", hex::encode(hash.0)),
            });
        }
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&path, data)?;
        Ok(CompatChunkLocation {
            bucket: format!("{:02x}", hash.0[0]),
            path: format!("{}.dblock", hex::encode(hash.0)),
        })
    }

    pub fn read_compat_chunk(
        &self,
        location: &CompatChunkLocation,
    ) -> Result<Vec<u8>, CompatRepoError> {
        let path = self
            .root
            .join("dblocks")
            .join(&location.bucket)
            .join(&location.path);
        if !path.exists() {
            return Err(CompatRepoError::NotFound(format!(
                "chunk not found: {}/{}",
                location.bucket, location.path
            )));
        }
        Ok(fs::read(&path)?)
    }

    pub fn chunk_exists(&self, hash: &ChunkHash) -> Result<bool, CompatRepoError> {
        Ok(self.chunk_path(hash).exists())
    }

    pub fn write_compat_manifest(
        &self,
        version_id: &str,
        manifest: &CompatibilityManifest,
    ) -> Result<(), CompatRepoError> {
        let staging_dir = self.root.join("dlists").join(".staging");
        fs::create_dir_all(&staging_dir)?;
        let staging = self.staging_path(version_id);
        let final_path = self.manifest_path(version_id);

        let data = serde_json::to_vec(manifest)?;
        fs::write(&staging, &data)?;

        fs::rename(&staging, &final_path)?;

        Ok(())
    }

    pub fn read_compat_manifest(
        &self,
        version_id: &str,
    ) -> Result<CompatibilityManifest, CompatRepoError> {
        let path = self.manifest_path(version_id);
        if !path.exists() {
            return Err(CompatRepoError::NotFound(format!(
                "manifest not found: {version_id}"
            )));
        }
        let data = fs::read(&path)?;
        Ok(serde_json::from_slice(&data)?)
    }

    pub fn list_compat_versions(
        &self,
    ) -> Result<Vec<CompatibilityVersionSummary>, CompatRepoError> {
        let dlists_dir = self.root.join("dlists");
        let mut versions = Vec::new();

        if !dlists_dir.exists() {
            return Ok(versions);
        }

        for entry in fs::read_dir(&dlists_dir)? {
            let entry = entry?;
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            let filename = path
                .file_name()
                .unwrap()
                .to_string_lossy()
                .to_string();
            if !filename.ends_with(".dlist") {
                continue;
            }

            let vid = &filename[..filename.len() - ".dlist".len()];
            let data = fs::read(&path)?;
            if let Ok(manifest) = serde_json::from_slice::<CompatibilityManifest>(&data) {
                versions.push(CompatibilityVersionSummary {
                    version_id: vid.to_string(),
                    version_number: manifest.version_number,
                    timestamp: manifest.timestamp,
                    backup_type: manifest.backup_type,
                    file_count: manifest.files.len() as u64,
                    total_size: manifest.files.iter().map(|f| f.size).sum(),
                });
            }
        }

        versions.sort_by_key(|v| std::cmp::Reverse(v.version_number));
        Ok(versions)
    }

    pub fn delete_compat_chunk(
        &self,
        location: &CompatChunkLocation,
    ) -> Result<(), CompatRepoError> {
        let path = self
            .root
            .join("dblocks")
            .join(&location.bucket)
            .join(&location.path);
        if path.exists() {
            fs::remove_file(&path)?;
        }
        Ok(())
    }

    pub fn acquire_lock(
        &self,
        operation: LockOperation,
        _timeout: Duration,
    ) -> Result<RepoLock, CompatRepoError> {
        let lock_id = Uuid::new_v4();
        let lock_path = self.lock_path(&lock_id);

        let lock = RepoLock {
            lock_id,
            holder: format!("{:?}", operation),
            acquired_at: Utc::now(),
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

    pub fn root(&self) -> &Path {
        &self.root
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    fn setup_repo() -> (tempfile::TempDir, CompatibleRepository) {
        let tmp = tempfile::tempdir().unwrap();
        let repo = CompatibleRepository::init(tmp.path(), "test-repo".to_string()).unwrap();
        (tmp, repo)
    }

    #[test]
    fn test_init_creates_directory_structure() {
        let (tmp, _repo) = setup_repo();
        assert!(tmp.path().join("compat_repository.json").exists());
        assert!(tmp.path().join("config").exists());
        assert!(tmp.path().join("dblocks").exists());
        assert!(tmp.path().join("dlists").exists());
        assert!(tmp.path().join("dindex").exists());
        assert!(tmp.path().join("deleteq").exists());
        assert!(tmp.path().join("locks").exists());
        assert!(tmp.path().join(".hbx-compat").join("format_version").exists());

        for i in 0u8..=255u8 {
            let bucket = format!("{:02x}", i);
            assert!(tmp.path().join("dblocks").join(&bucket).exists());
        }
    }

    #[test]
    fn test_write_read_chunk() {
        let (_tmp, repo) = setup_repo();
        let hash = ChunkHash([0xab; 32]);
        let data = b"test chunk data";

        let loc = repo.write_compat_chunk(&hash, data).unwrap();
        assert_eq!(loc.bucket, "ab");

        let read = repo.read_compat_chunk(&loc).unwrap();
        assert_eq!(read, data);
    }

    #[test]
    fn test_chunk_idempotent_write() {
        let (_tmp, repo) = setup_repo();
        let hash = ChunkHash([0xcd; 32]);
        repo.write_compat_chunk(&hash, b"first").unwrap();
        repo.write_compat_chunk(&hash, b"second").unwrap();

        let loc = CompatChunkLocation {
            bucket: "cd".to_string(),
            path: format!("{}.dblock", hex::encode(hash.0)),
        };
        let read = repo.read_compat_chunk(&loc).unwrap();
        assert_eq!(read, b"first");
    }

    #[test]
    fn test_chunk_exists() {
        let (_tmp, repo) = setup_repo();
        let hash = ChunkHash([0xef; 32]);
        assert!(!repo.chunk_exists(&hash).unwrap());
        repo.write_compat_chunk(&hash, b"data").unwrap();
        assert!(repo.chunk_exists(&hash).unwrap());
    }

    #[test]
    fn test_delete_chunk() {
        let (_tmp, repo) = setup_repo();
        let hash = ChunkHash([0x11; 32]);
        let loc = repo.write_compat_chunk(&hash, b"data").unwrap();
        assert!(repo.chunk_exists(&hash).unwrap());
        repo.delete_compat_chunk(&loc).unwrap();
        assert!(!repo.chunk_exists(&hash).unwrap());
    }

    fn make_test_manifest(version_id: &str, version_number: u64) -> CompatibilityManifest {
        CompatibilityManifest {
            version_id: version_id.to_string(),
            timestamp: Utc::now(),
            parent_version_id: None,
            version_number,
            backup_type: "full".to_string(),
            files: vec![CompatFileEntry {
                path: "/test/file.txt".to_string(),
                size: 100,
                modified_at: Utc::now(),
                chunks: vec![],
                file_hash: [0u8; 32],
            }],
            chunk_refs: vec![],
            hashes: CompatibilityHashes {
                manifest_hash: [0u8; 32],
                file_index_hash: [0u8; 32],
                chunk_index_hash: [0u8; 32],
            },
        }
    }

    #[test]
    fn test_write_read_manifest() {
        let (_tmp, repo) = setup_repo();
        let manifest = make_test_manifest("v1", 1);
        repo.write_compat_manifest("v1", &manifest).unwrap();
        let read = repo.read_compat_manifest("v1").unwrap();
        assert_eq!(read.version_id, "v1");
        assert_eq!(read.files.len(), 1);
    }

    #[test]
    fn test_list_versions() {
        let (_tmp, repo) = setup_repo();
        for i in 1..=3 {
            let vid = format!("v{}", i);
            let manifest = make_test_manifest(&vid, i);
            repo.write_compat_manifest(&vid, &manifest).unwrap();
        }
        let versions = repo.list_compat_versions().unwrap();
        assert_eq!(versions.len(), 3);
        assert_eq!(versions[0].version_number, 3);
    }

    #[test]
    fn test_two_phase_commit_no_staging_left() {
        let (_tmp, repo) = setup_repo();
        let manifest = make_test_manifest("v1", 1);
        repo.write_compat_manifest("v1", &manifest).unwrap();

        let staging = repo.root.join("dlists").join(".staging").join("v1.dlist");
        assert!(!staging.exists(), "staging file should be renamed");
        assert!(repo.root.join("dlists").join("v1.dlist").exists());
    }

    #[test]
    fn test_acquire_lock() {
        let (_tmp, repo) = setup_repo();
        let lock = repo
            .acquire_lock(LockOperation::Backup, Duration::from_secs(60))
            .unwrap();
        assert_eq!(lock.holder, "Backup");
    }

    #[test]
    fn test_open_nonexistent_repo() {
        let tmp = tempfile::tempdir().unwrap();
        let result = CompatibleRepository::open(tmp.path());
        assert!(result.is_err());
    }

    #[test]
    fn test_256_buckets_created() {
        let (tmp, _) = setup_repo();
        let dblocks = tmp.path().join("dblocks");
        let mut count = 0;
        for entry in fs::read_dir(&dblocks).unwrap() {
            let entry = entry.unwrap();
            if entry.path().is_dir() {
                count += 1;
            }
        }
        assert_eq!(count, 256);
    }

    #[test]
    fn test_read_nonexistent_chunk() {
        let (_tmp, repo) = setup_repo();
        let loc = CompatChunkLocation {
            bucket: "ab".to_string(),
            path: "nonexistent.dblock".to_string(),
        };
        let result = repo.read_compat_chunk(&loc);
        assert!(result.is_err());
    }

    #[test]
    fn test_read_nonexistent_manifest() {
        let (_tmp, repo) = setup_repo();
        let result = repo.read_compat_manifest("nonexistent");
        assert!(result.is_err());
    }
}