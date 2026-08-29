use std::time::Duration;

use hbx_compat_repo::{
    CompatChunkLocation, CompatFileEntry, CompatRepoError, CompatibleRepository,
    CompatibilityHashes, CompatibilityManifest, CompatibilityVersionSummary,
};
use hbx_core::domain::backup::BackupType;
use hbx_core::domain::chunk::{ChunkHash, ChunkLocation};
use hbx_core::domain::common::{FileAttributes, LockOperation, RepoLock, VersionId, VersionSummary};
use hbx_core::domain::encryption::EncryptedChunk;
use hbx_core::domain::repository::{FileEntry, Manifest, ManifestHashes};
use hbx_core::pipeline::traits::{IBackupRepository, RepoError};

pub struct CompatibilityRepoAdapter {
    repo: CompatibleRepository,
}

impl CompatibilityRepoAdapter {
    pub fn new(repo: CompatibleRepository) -> Self {
        Self { repo }
    }

    pub fn inner(&self) -> &CompatibleRepository {
        &self.repo
    }
}

fn compat_err_to_repo_err(e: CompatRepoError) -> RepoError {
    match e {
        CompatRepoError::NotFound(s) => RepoError::NotFound(s),
        CompatRepoError::Io(e) => RepoError::Io(e),
        CompatRepoError::Serialize(e) => RepoError::Serialize(e),
        CompatRepoError::Failed(s) => RepoError::Failed(s),
    }
}

fn backup_type_to_string(bt: &BackupType) -> String {
    match bt {
        BackupType::Full => "full".to_string(),
        BackupType::Incremental => "incremental".to_string(),
    }
}

fn string_to_backup_type(s: &str) -> BackupType {
    match s {
        "incremental" => BackupType::Incremental,
        _ => BackupType::Full,
    }
}

fn file_entry_to_compat(fe: &FileEntry) -> CompatFileEntry {
    CompatFileEntry {
        path: fe.path.clone(),
        size: fe.size,
        modified_at: fe.modified_at,
        chunks: fe.chunks.clone(),
        file_hash: fe.file_hash,
    }
}

fn compat_to_file_entry(cfe: &CompatFileEntry) -> FileEntry {
    FileEntry {
        path: cfe.path.clone(),
        size: cfe.size,
        modified_at: cfe.modified_at,
        attributes: FileAttributes::default(),
        chunks: cfe.chunks.clone(),
        file_hash: cfe.file_hash,
    }
}

fn manifest_to_compat(manifest: &Manifest) -> CompatibilityManifest {
    CompatibilityManifest {
        version_id: manifest.version_id.0.to_string(),
        timestamp: manifest.timestamp,
        parent_version_id: manifest.parent_version_id.as_ref().map(|v| v.0.to_string()),
        version_number: manifest.version_number,
        backup_type: backup_type_to_string(&manifest.backup_type),
        files: manifest.files.iter().map(file_entry_to_compat).collect(),
        chunk_refs: manifest.chunk_refs.clone(),
        hashes: CompatibilityHashes {
            manifest_hash: manifest.hashes.manifest_hash,
            file_index_hash: manifest.hashes.file_index_hash,
            chunk_index_hash: manifest.hashes.chunk_index_hash,
        },
    }
}

fn compat_to_manifest(cm: &CompatibilityManifest) -> Result<Manifest, RepoError> {
    let version_uuid = uuid::Uuid::parse_str(&cm.version_id)
        .map_err(|e| RepoError::Failed(format!("invalid version_id UUID: {e}")))?;
    let parent_version_id = match &cm.parent_version_id {
        Some(s) => Some(VersionId(
            uuid::Uuid::parse_str(s)
                .map_err(|e| RepoError::Failed(format!("invalid parent_version_id UUID: {e}")))?,
        )),
        None => None,
    };
    Ok(Manifest {
        version_id: VersionId(version_uuid),
        timestamp: cm.timestamp,
        parent_version_id,
        version_number: cm.version_number,
        backup_type: string_to_backup_type(&cm.backup_type),
        files: cm.files.iter().map(compat_to_file_entry).collect(),
        chunk_refs: cm.chunk_refs.clone(),
        hashes: ManifestHashes {
            manifest_hash: cm.hashes.manifest_hash,
            file_index_hash: cm.hashes.file_index_hash,
            chunk_index_hash: cm.hashes.chunk_index_hash,
            repo_hash: [0u8; 32],
        },
        chunk_locations: Default::default(),
    })
}

fn compat_version_to_summary(cv: &CompatibilityVersionSummary) -> Result<VersionSummary, RepoError> {
    let version_uuid = uuid::Uuid::parse_str(&cv.version_id)
        .map_err(|e| RepoError::Failed(format!("invalid version_id UUID: {e}")))?;
    Ok(VersionSummary {
        version_id: version_uuid,
        version_number: cv.version_number,
        timestamp: cv.timestamp,
        backup_type: string_to_backup_type(&cv.backup_type),
        total_size: cv.total_size,
        stored_size: cv.total_size,
    })
}

impl IBackupRepository for CompatibilityRepoAdapter {
    fn write_chunk(
        &self,
        hash: &ChunkHash,
        encrypted: &EncryptedChunk,
    ) -> Result<ChunkLocation, RepoError> {
        let data = serde_json::to_vec(encrypted).map_err(RepoError::Serialize)?;
        let loc = self
            .repo
            .write_compat_chunk(hash, &data)
            .map_err(compat_err_to_repo_err)?;
        Ok(ChunkLocation {
            bucket: loc.bucket,
            path: loc.path,
        })
    }

    fn read_chunk(&self, location: &ChunkLocation) -> Result<EncryptedChunk, RepoError> {
        let compat_loc = CompatChunkLocation {
            bucket: location.bucket.clone(),
            path: location.path.clone(),
        };
        let data = self
            .repo
            .read_compat_chunk(&compat_loc)
            .map_err(compat_err_to_repo_err)?;
        serde_json::from_slice(&data).map_err(RepoError::Serialize)
    }

    fn chunk_exists(&self, hash: &ChunkHash) -> Result<bool, RepoError> {
        self.repo
            .chunk_exists(hash)
            .map_err(compat_err_to_repo_err)
    }

    fn find_chunk(&self, hash: &ChunkHash) -> Result<ChunkLocation, RepoError> {
        let bucket = format!("{:02x}", hash.0[0]);
        let path = format!("{}.dblock", hex::encode(hash.0));

        if self.repo.chunk_exists(hash).map_err(compat_err_to_repo_err)? {
            Ok(ChunkLocation { bucket, path })
        } else {
            Err(RepoError::NotFound(format!("chunk not found: {}/{}", bucket, path)))
        }
    }

    fn delete_chunk(&self, location: &ChunkLocation) -> Result<(), RepoError> {
        let compat_loc = CompatChunkLocation {
            bucket: location.bucket.clone(),
            path: location.path.clone(),
        };
        self.repo
            .delete_compat_chunk(&compat_loc)
            .map_err(compat_err_to_repo_err)
    }

    fn write_manifest(
        &self,
        version_id: &VersionId,
        manifest: &Manifest,
    ) -> Result<(), RepoError> {
        let vid = version_id.0.to_string();
        let compat_manifest = manifest_to_compat(manifest);
        self.repo
            .write_compat_manifest(&vid, &compat_manifest)
            .map_err(compat_err_to_repo_err)
    }

    fn read_manifest(&self, version_id: &VersionId) -> Result<Manifest, RepoError> {
        let vid = version_id.0.to_string();
        let compat_manifest = self
            .repo
            .read_compat_manifest(&vid)
            .map_err(compat_err_to_repo_err)?;
        compat_to_manifest(&compat_manifest)
    }

    fn list_versions(&self) -> Result<Vec<VersionSummary>, RepoError> {
        let compat_versions = self
            .repo
            .list_compat_versions()
            .map_err(compat_err_to_repo_err)?;
        compat_versions
            .iter()
            .map(compat_version_to_summary)
            .collect()
    }

    fn acquire_lock(
        &self,
        operation: LockOperation,
        timeout: Duration,
    ) -> Result<RepoLock, RepoError> {
        self.repo
            .acquire_lock(operation, timeout)
            .map_err(compat_err_to_repo_err)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hbx_core::domain::common::VersionId;

    fn setup() -> (tempfile::TempDir, CompatibilityRepoAdapter) {
        let tmp = tempfile::tempdir().unwrap();
        let repo = CompatibleRepository::init(tmp.path(), "test-adapter".to_string()).unwrap();
        (tmp, CompatibilityRepoAdapter::new(repo))
    }

    #[test]
    fn test_adapter_write_read_chunk() {
        let (_tmp, adapter) = setup();
        let hash = ChunkHash([0xab; 32]);
        let encrypted = EncryptedChunk {
            ciphertext: b"test data".to_vec(),
            nonce: [1u8; 12],
            auth_tag: [2u8; 16],
        };
        let loc = adapter.write_chunk(&hash, &encrypted).unwrap();
        assert!(adapter.chunk_exists(&hash).unwrap());

        let read = adapter.read_chunk(&loc).unwrap();
        assert_eq!(read.ciphertext, encrypted.ciphertext);
        assert_eq!(read.nonce, encrypted.nonce);
        assert_eq!(read.auth_tag, encrypted.auth_tag);
    }

    #[test]
    fn test_adapter_find_chunk() {
        let (_tmp, adapter) = setup();
        let hash = ChunkHash([0xcd; 32]);
        let encrypted = EncryptedChunk {
            ciphertext: b"find test".to_vec(),
            nonce: [0u8; 12],
            auth_tag: [0u8; 16],
        };
        assert!(adapter.find_chunk(&hash).is_err());
        adapter.write_chunk(&hash, &encrypted).unwrap();
        let loc = adapter.find_chunk(&hash).unwrap();
        assert_eq!(loc.bucket, "cd");
    }

    #[test]
    fn test_adapter_delete_chunk() {
        let (_tmp, adapter) = setup();
        let hash = ChunkHash([0xef; 32]);
        let encrypted = EncryptedChunk {
            ciphertext: b"delete test".to_vec(),
            nonce: [0u8; 12],
            auth_tag: [0u8; 16],
        };
        let loc = adapter.write_chunk(&hash, &encrypted).unwrap();
        assert!(adapter.chunk_exists(&hash).unwrap());
        adapter.delete_chunk(&loc).unwrap();
        assert!(!adapter.chunk_exists(&hash).unwrap());
    }

    #[test]
    fn test_adapter_write_read_manifest() {
        let (_tmp, adapter) = setup();
        let version_id = VersionId(uuid::Uuid::new_v4());
        let manifest = Manifest {
            version_id: version_id.clone(),
            timestamp: chrono::Utc::now(),
            parent_version_id: None,
            version_number: 1,
            backup_type: BackupType::Full,
            files: vec![FileEntry {
                path: "/test/file.txt".to_string(),
                size: 100,
                modified_at: chrono::Utc::now(),
                attributes: FileAttributes::default(),
                chunks: vec![ChunkHash([0u8; 32])],
                file_hash: [0u8; 32],
            }],
            chunk_refs: vec![],
            hashes: ManifestHashes {
                manifest_hash: [0u8; 32],
                file_index_hash: [0u8; 32],
                chunk_index_hash: [0u8; 32],
                repo_hash: [0u8; 32],
            },
            chunk_locations: Default::default(),
        };
        adapter.write_manifest(&version_id, &manifest).unwrap();
        let read = adapter.read_manifest(&version_id).unwrap();
        assert_eq!(read.version_id, version_id);
        assert_eq!(read.files.len(), 1);
        assert_eq!(read.files[0].path, "/test/file.txt");
    }

    #[test]
    fn test_adapter_list_versions() {
        let (_tmp, adapter) = setup();
        for i in 1..=3 {
            let vid = VersionId(uuid::Uuid::new_v4());
            let manifest = Manifest {
                version_id: vid,
                timestamp: chrono::Utc::now(),
                parent_version_id: None,
                version_number: i,
                backup_type: BackupType::Full,
                files: vec![],
                chunk_refs: vec![],
                hashes: ManifestHashes {
                    manifest_hash: [0u8; 32],
                    file_index_hash: [0u8; 32],
                    chunk_index_hash: [0u8; 32],
                    repo_hash: [0u8; 32],
                },
                chunk_locations: Default::default(),
            };
            adapter.write_manifest(&VersionId(uuid::Uuid::new_v4()), &manifest).unwrap();
        }
        let versions = adapter.list_versions().unwrap();
        assert_eq!(versions.len(), 3);
    }

    #[test]
    fn test_adapter_acquire_lock() {
        let (_tmp, adapter) = setup();
        let lock = adapter
            .acquire_lock(LockOperation::Backup, Duration::from_secs(60))
            .unwrap();
        assert_eq!(lock.holder, "Backup");
    }
}