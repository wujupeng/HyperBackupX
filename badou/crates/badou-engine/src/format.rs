use std::path::{Path, PathBuf};
use thiserror::Error;
use hbx_core::domain::chunk::ChunkHash;
use hbx_core::domain::common::RepositoryId;

pub const BADOU_FORMAT_VERSION: u32 = 1;

#[derive(Debug, Error)]
pub enum FormatError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("repository already exists: {0}")]
    AlreadyExists(PathBuf),
}

#[derive(Debug, Clone)]
pub struct BadouDataLayout {
    pub data_root: PathBuf,
}

impl BadouDataLayout {
    pub fn new(data_root: impl AsRef<Path>) -> Self {
        Self {
            data_root: data_root.as_ref().to_path_buf(),
        }
    }

    pub fn repo_root(&self, repo_id: &RepositoryId) -> PathBuf {
        self.data_root.join("repositories").join(repo_id.0.to_string())
    }

    pub fn chunks_dir(&self, repo_id: &RepositoryId) -> PathBuf {
        self.repo_root(repo_id).join("chunks")
    }

    pub fn manifests_dir(&self, repo_id: &RepositoryId) -> PathBuf {
        self.repo_root(repo_id).join("manifests")
    }

    pub fn snapshots_dir(&self, repo_id: &RepositoryId) -> PathBuf {
        self.repo_root(repo_id).join("snapshots")
    }

    pub fn versions_dir(&self, repo_id: &RepositoryId) -> PathBuf {
        self.repo_root(repo_id).join("versions")
    }

    pub fn index_dir(&self, repo_id: &RepositoryId) -> PathBuf {
        self.repo_root(repo_id).join("index")
    }

    pub fn journal_dir(&self, repo_id: &RepositoryId) -> PathBuf {
        self.repo_root(repo_id).join("journal")
    }

    pub fn gc_dir(&self, repo_id: &RepositoryId) -> PathBuf {
        self.repo_root(repo_id).join("gc")
    }

    pub fn locks_dir(&self, repo_id: &RepositoryId) -> PathBuf {
        self.repo_root(repo_id).join("locks")
    }

    pub fn staging_dir(&self, repo_id: &RepositoryId) -> PathBuf {
        self.repo_root(repo_id).join(".staging")
    }

    pub fn meta_dir(&self, repo_id: &RepositoryId) -> PathBuf {
        self.repo_root(repo_id).join(".badou")
    }

    pub fn format_version_path(&self, repo_id: &RepositoryId) -> PathBuf {
        self.meta_dir(repo_id).join("format_version")
    }

    pub fn bucket_path(&self, repo_id: &RepositoryId, chunk_hash: &ChunkHash) -> PathBuf {
        let prefix = hex::encode(&chunk_hash.0[..1]);
        let full_hash = hex::encode(chunk_hash.0);
        self.chunks_dir(repo_id).join(&prefix).join(format!("{}.chunk", full_hash))
    }

    pub fn init_repository(&self, repo_id: &RepositoryId) -> Result<(), FormatError> {
        let repo_root = self.repo_root(repo_id);
        if repo_root.exists() {
            return Err(FormatError::AlreadyExists(repo_root));
        }

        std::fs::create_dir_all(self.manifests_dir(repo_id))?;
        std::fs::create_dir_all(self.snapshots_dir(repo_id))?;
        std::fs::create_dir_all(self.versions_dir(repo_id))?;
        std::fs::create_dir_all(self.index_dir(repo_id))?;
        std::fs::create_dir_all(self.journal_dir(repo_id))?;
        std::fs::create_dir_all(self.gc_dir(repo_id))?;
        std::fs::create_dir_all(self.locks_dir(repo_id))?;
        std::fs::create_dir_all(self.staging_dir(repo_id))?;
        std::fs::create_dir_all(self.meta_dir(repo_id))?;

        let chunks_dir = self.chunks_dir(repo_id);
        for i in 0u8..=255u8 {
            let bucket = hex::encode([i]);
            std::fs::create_dir_all(chunks_dir.join(&bucket))?;
        }

        std::fs::write(
            self.format_version_path(repo_id),
            BADOU_FORMAT_VERSION.to_string(),
        )?;

        Ok(())
    }
}

pub fn bucket_prefix(chunk_hash: &ChunkHash) -> String {
    hex::encode(&chunk_hash.0[..1])
}

pub fn chunk_filename(chunk_hash: &ChunkHash) -> String {
    format!("{}.chunk", hex::encode(chunk_hash.0))
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    #[test]
    fn bucket_prefix_is_2_hex_chars() {
        let hash = ChunkHash([0xab; 32]);
        assert_eq!(bucket_prefix(&hash), "ab");
    }

    #[test]
    fn chunk_filename_is_full_hash() {
        let hash = ChunkHash([0x00; 32]);
        assert_eq!(chunk_filename(&hash), "0000000000000000000000000000000000000000000000000000000000000000.chunk");
    }

    #[test]
    fn init_repository_creates_256_buckets() {
        let tmp = tempfile::tempdir().unwrap();
        let layout = BadouDataLayout::new(tmp.path());
        let repo_id = RepositoryId(Uuid::new_v4());
        layout.init_repository(&repo_id).unwrap();

        let chunks_dir = layout.chunks_dir(&repo_id);
        let bucket_count = std::fs::read_dir(&chunks_dir).unwrap()
            .filter(|e| e.as_ref().unwrap().path().is_dir())
            .count();
        assert_eq!(bucket_count, 256);
    }

    #[test]
    fn init_repository_writes_format_version() {
        let tmp = tempfile::tempdir().unwrap();
        let layout = BadouDataLayout::new(tmp.path());
        let repo_id = RepositoryId(Uuid::new_v4());
        layout.init_repository(&repo_id).unwrap();

        let version = std::fs::read_to_string(layout.format_version_path(&repo_id)).unwrap();
        assert_eq!(version, "1");
    }

    #[test]
    fn init_repository_duplicate_fails() {
        let tmp = tempfile::tempdir().unwrap();
        let layout = BadouDataLayout::new(tmp.path());
        let repo_id = RepositoryId(Uuid::new_v4());
        layout.init_repository(&repo_id).unwrap();
        assert!(layout.init_repository(&repo_id).is_err());
    }

    #[test]
    fn bucket_path_layout() {
        let tmp = tempfile::tempdir().unwrap();
        let layout = BadouDataLayout::new(tmp.path());
        let repo_id = RepositoryId(Uuid::new_v4());
        let hash = ChunkHash([0xab; 32]);
        let path = layout.bucket_path(&repo_id, &hash);
        assert!(path.to_string_lossy().contains("chunks"));
        assert!(path.to_string_lossy().contains("ab"));
        assert!(path.to_string_lossy().ends_with(".chunk"));
    }
}