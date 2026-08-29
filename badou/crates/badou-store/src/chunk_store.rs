//! Chunk 物理持久化：分桶写入 + 内容寻址去重 + 哈希校验。

use std::path::{Path, PathBuf};
use hbx_core::domain::chunk::ChunkHash;
use hbx_core::domain::common::RepositoryId;
use badou_engine::format::BadouDataLayout;
use badou_index::{BadouIndex, ChunkIndexEntry, ChunkIndexStatus};
use thiserror::Error;
use chrono::Utc;

#[derive(Debug, Error)]
pub enum ChunkStoreError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("hash mismatch: expected {expected}, got {actual}")]
    HashMismatch { expected: String, actual: String },
    #[error("index error: {0}")]
    Index(#[from] badou_index::IndexError),
    #[error("chunk not found: {0}")]
    NotFound(String),
}

pub struct ChunkStore {
    layout: BadouDataLayout,
    index: BadouIndex,
}

impl ChunkStore {
    pub fn new(layout: BadouDataLayout, index: BadouIndex) -> Self {
        Self { layout, index }
    }

    pub fn chunk_path(&self, repo_id: &RepositoryId, chunk_hash: &ChunkHash) -> PathBuf {
        self.layout.bucket_path(repo_id, chunk_hash)
    }

    pub fn chunk_exists(&self, chunk_hash: &ChunkHash) -> bool {
        let hash_hex = hex::encode(chunk_hash.0);
        self.index.lookup(&hash_hex).is_some()
    }

    pub fn write_chunk(
        &self,
        repo_id: &RepositoryId,
        chunk_hash: &ChunkHash,
        encrypted_data: &[u8],
    ) -> Result<ChunkLocation, ChunkStoreError> {
        let hash_hex = hex::encode(chunk_hash.0);

        if let Some(existing) = self.index.lookup(&hash_hex) {
            self.index.increment_ref_count(&hash_hex)?;
            return Ok(ChunkLocation {
                bucket: existing.bucket,
                path: existing.path,
                deduplicated: true,
            });
        }

        let chunk_path = self.layout.bucket_path(repo_id, chunk_hash);
        if let Some(parent) = chunk_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&chunk_path, encrypted_data)?;

        let bucket = hex::encode(&chunk_hash.0[..1]);
        let entry = ChunkIndexEntry {
            bucket: bucket.clone(),
            path: chunk_path.to_string_lossy().to_string(),
            ref_count: 1,
            size: encrypted_data.len() as u64,
            stored_size: encrypted_data.len() as u64,
            created_at: Utc::now(),
            status: ChunkIndexStatus::Active,
        };
        self.index.register(&hash_hex, entry)?;

        Ok(ChunkLocation {
            bucket,
            path: chunk_path.to_string_lossy().to_string(),
            deduplicated: false,
        })
    }

    pub fn read_chunk(
        &self,
        _repo_id: &RepositoryId,
        chunk_hash: &ChunkHash,
    ) -> Result<Vec<u8>, ChunkStoreError> {
        let hash_hex = hex::encode(chunk_hash.0);
        let entry = self.index.lookup(&hash_hex)
            .ok_or(ChunkStoreError::NotFound(hash_hex.clone()))?;
        let path = Path::new(&entry.path);
        let data = std::fs::read(path)?;

        let actual_hash = blake3::hash(&data);
        if actual_hash.as_bytes() != chunk_hash.0.as_slice() {
            return Err(ChunkStoreError::HashMismatch {
                expected: hash_hex,
                actual: hex::encode(actual_hash.as_bytes()),
            });
        }

        Ok(data)
    }

    pub fn delete_chunk_physical(
        &self,
        _repo_id: &RepositoryId,
        chunk_hash: &ChunkHash,
    ) -> Result<(), ChunkStoreError> {
        let hash_hex = hex::encode(chunk_hash.0);
        let entry = self.index.lookup(&hash_hex)
            .ok_or(ChunkStoreError::NotFound(hash_hex.clone()))?;

        let path = Path::new(&entry.path);
        if path.exists() {
            std::fs::remove_file(path)?;
        }
        self.index.update_status(&hash_hex, ChunkIndexStatus::Purged)?;

        Ok(())
    }

    pub fn ref_count(&self, chunk_hash: &ChunkHash) -> u32 {
        let hash_hex = hex::encode(chunk_hash.0);
        self.index.lookup(&hash_hex).map(|e| e.ref_count).unwrap_or(0)
    }
}

#[derive(Debug, Clone)]
pub struct ChunkLocation {
    pub bucket: String,
    pub path: String,
    pub deduplicated: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    fn make_store() -> (ChunkStore, RepositoryId) {
        let tmp = tempfile::tempdir().unwrap();
        let layout = BadouDataLayout::new(tmp.path());
        let index = BadouIndex::in_memory();
        let store = ChunkStore::new(layout, index);
        let repo_id = RepositoryId(Uuid::new_v4());
        std::mem::forget(tmp);
        (store, repo_id)
    }

    fn make_hash(data: &[u8]) -> ChunkHash {
        let h = blake3::hash(data);
        let mut arr = [0u8; 32];
        arr.copy_from_slice(h.as_bytes());
        ChunkHash(arr)
    }

    #[test]
    fn write_and_read_chunk() {
        let (store, repo_id) = make_store();
        let data = b"hello world chunk data";
        let hash = make_hash(data);
        let loc = store.write_chunk(&repo_id, &hash, data).unwrap();
        assert!(!loc.deduplicated);
        let read_back = store.read_chunk(&repo_id, &hash).unwrap();
        assert_eq!(read_back, data);
    }

    #[test]
    fn dedup_increments_ref_count() {
        let (store, repo_id) = make_store();
        let data = b"dedup content";
        let hash = make_hash(data);
        let loc1 = store.write_chunk(&repo_id, &hash, data).unwrap();
        assert!(!loc1.deduplicated);
        let loc2 = store.write_chunk(&repo_id, &hash, data).unwrap();
        assert!(loc2.deduplicated);
        assert_eq!(store.ref_count(&hash), 2);
    }

    #[test]
    fn hash_mismatch_rejected() {
        let (store, repo_id) = make_store();
        let data = b"original content";
        let hash = make_hash(data);
        store.write_chunk(&repo_id, &hash, data).unwrap();

        let wrong_data = b"tampered content";
        let wrong_hash = make_hash(wrong_data);

        let result = store.read_chunk(&repo_id, &wrong_hash);
        assert!(result.is_err());
    }

    #[test]
    fn chunk_exists_after_write() {
        let (store, repo_id) = make_store();
        let data = b"existence test";
        let hash = make_hash(data);
        assert!(!store.chunk_exists(&hash));
        store.write_chunk(&repo_id, &hash, data).unwrap();
        assert!(store.chunk_exists(&hash));
    }

    #[test]
    fn delete_chunk_physical_removes_file() {
        let (store, repo_id) = make_store();
        let data = b"to be deleted";
        let hash = make_hash(data);
        store.write_chunk(&repo_id, &hash, data).unwrap();
        assert!(store.chunk_exists(&hash));
        store.delete_chunk_physical(&repo_id, &hash).unwrap();
        let result = store.read_chunk(&repo_id, &hash);
        assert!(result.is_err());
    }
}