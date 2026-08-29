//! 索引：chunk_hash → location / ref_count。
//!
//! 使用 HashMap + RwLock 实现并发安全索引，持久化到文件。

pub mod entry;

pub use entry::{ChunkIndexEntry, ChunkIndexStatus};

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum IndexError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("serde error: {0}")]
    Serde(#[from] serde_json::Error),
    #[error("chunk not found: {0}")]
    NotFound(String),
}

#[derive(Clone)]
pub struct BadouIndex {
    chunk_index: Arc<RwLock<HashMap<String, ChunkIndexEntry>>>,
    version_chunks: Arc<RwLock<HashMap<Uuid, Vec<String>>>>,
    manifest_hashes: Arc<RwLock<HashMap<Uuid, String>>>,
    consistent: Arc<RwLock<bool>>,
    persist_path: Option<PathBuf>,
}

impl BadouIndex {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, IndexError> {
        let path = path.as_ref();
        if path.exists() {
            let data = std::fs::read(path)?;
            let persisted: PersistedIndex = serde_json::from_slice(&data)?;
            Ok(Self {
                chunk_index: Arc::new(RwLock::new(persisted.chunk_index)),
                version_chunks: Arc::new(RwLock::new(persisted.version_chunks)),
                manifest_hashes: Arc::new(RwLock::new(persisted.manifest_hashes)),
                consistent: Arc::new(RwLock::new(persisted.consistent)),
                persist_path: Some(path.to_path_buf()),
            })
        } else {
            Ok(Self {
                chunk_index: Arc::new(RwLock::new(HashMap::new())),
                version_chunks: Arc::new(RwLock::new(HashMap::new())),
                manifest_hashes: Arc::new(RwLock::new(HashMap::new())),
                consistent: Arc::new(RwLock::new(true)),
                persist_path: Some(path.to_path_buf()),
            })
        }
    }

    pub fn in_memory() -> Self {
        Self {
            chunk_index: Arc::new(RwLock::new(HashMap::new())),
            version_chunks: Arc::new(RwLock::new(HashMap::new())),
            manifest_hashes: Arc::new(RwLock::new(HashMap::new())),
            consistent: Arc::new(RwLock::new(true)),
            persist_path: None,
        }
    }

    pub fn lookup(&self, chunk_hash: &str) -> Option<ChunkIndexEntry> {
        self.chunk_index.read().get(chunk_hash).cloned()
    }

    pub fn register(&self, chunk_hash: &str, entry: ChunkIndexEntry) -> Result<(), IndexError> {
        self.chunk_index.write().insert(chunk_hash.to_string(), entry);
        self.persist()
    }

    pub fn increment_ref_count(&self, chunk_hash: &str) -> Result<u32, IndexError> {
        let mut guard = self.chunk_index.write();
        let entry = guard.get_mut(chunk_hash).ok_or(IndexError::NotFound(chunk_hash.to_string()))?;
        entry.ref_count += 1;
        let count = entry.ref_count;
        drop(guard);
        self.persist()?;
        Ok(count)
    }

    pub fn decrement_ref_count(&self, chunk_hash: &str) -> Result<u32, IndexError> {
        let mut guard = self.chunk_index.write();
        let entry = guard.get_mut(chunk_hash).ok_or(IndexError::NotFound(chunk_hash.to_string()))?;
        if entry.ref_count > 0 {
            entry.ref_count -= 1;
        }
        if entry.ref_count == 0 {
            entry.status = ChunkIndexStatus::GcPending;
        }
        let count = entry.ref_count;
        drop(guard);
        self.persist()?;
        Ok(count)
    }

    pub fn update_status(&self, chunk_hash: &str, status: ChunkIndexStatus) -> Result<(), IndexError> {
        let mut guard = self.chunk_index.write();
        let entry = guard.get_mut(chunk_hash).ok_or(IndexError::NotFound(chunk_hash.to_string()))?;
        entry.status = status;
        drop(guard);
        self.persist()
    }

    pub fn register_version_chunks(&self, version_id: Uuid, chunk_hashes: Vec<String>) -> Result<(), IndexError> {
        self.version_chunks.write().insert(version_id, chunk_hashes);
        self.persist()
    }

    pub fn get_version_chunks(&self, version_id: Uuid) -> Vec<String> {
        self.version_chunks.read().get(&version_id).cloned().unwrap_or_default()
    }

    pub fn register_manifest_hash(&self, manifest_id: Uuid, hash: String) -> Result<(), IndexError> {
        self.manifest_hashes.write().insert(manifest_id, hash);
        self.persist()
    }

    pub fn verify_manifest_hash(&self, manifest_id: Uuid, expected_hash: &str) -> bool {
        self.manifest_hashes.read()
            .get(&manifest_id)
            .map(|h| h == expected_hash)
            .unwrap_or(false)
    }

    pub fn mark_inconsistent(&self) -> Result<(), IndexError> {
        *self.consistent.write() = false;
        self.persist()
    }

    pub fn mark_consistent(&self) -> Result<(), IndexError> {
        *self.consistent.write() = true;
        self.persist()
    }

    pub fn is_consistent(&self) -> bool {
        *self.consistent.read()
    }

    pub fn gc_candidates(&self) -> Vec<(String, ChunkIndexEntry)> {
        self.chunk_index.read()
            .iter()
            .filter(|(_, e)| e.ref_count == 0 && e.status == ChunkIndexStatus::GcPending)
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect()
    }

    pub fn chunk_count(&self) -> usize {
        self.chunk_index.read().len()
    }

    fn persist(&self) -> Result<(), IndexError> {
        if let Some(path) = &self.persist_path {
            let data = PersistedIndex {
                chunk_index: self.chunk_index.read().clone(),
                version_chunks: self.version_chunks.read().clone(),
                manifest_hashes: self.manifest_hashes.read().clone(),
                consistent: *self.consistent.read(),
            };
            let bytes = serde_json::to_vec(&data)?;
            std::fs::write(path, bytes)?;
        }
        Ok(())
    }
}

#[derive(Serialize, Deserialize)]
struct PersistedIndex {
    chunk_index: HashMap<String, ChunkIndexEntry>,
    version_chunks: HashMap<Uuid, Vec<String>>,
    manifest_hashes: HashMap<Uuid, String>,
    consistent: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn make_entry() -> ChunkIndexEntry {
        ChunkIndexEntry {
            bucket: "ab".to_string(),
            path: "chunks/ab/abcdef.chunk".to_string(),
            ref_count: 1,
            size: 1024,
            stored_size: 512,
            created_at: Utc::now(),
            status: ChunkIndexStatus::Active,
        }
    }

    #[test]
    fn register_and_lookup() {
        let idx = BadouIndex::in_memory();
        idx.register("abcdef", make_entry()).unwrap();
        let entry = idx.lookup("abcdef").unwrap();
        assert_eq!(entry.size, 1024);
        assert_eq!(entry.ref_count, 1);
    }

    #[test]
    fn increment_decrement_ref_count() {
        let idx = BadouIndex::in_memory();
        idx.register("hash123", make_entry()).unwrap();
        assert_eq!(idx.increment_ref_count("hash123").unwrap(), 2);
        assert_eq!(idx.increment_ref_count("hash123").unwrap(), 3);
        assert_eq!(idx.decrement_ref_count("hash123").unwrap(), 2);
        assert_eq!(idx.decrement_ref_count("hash123").unwrap(), 1);
        assert_eq!(idx.decrement_ref_count("hash123").unwrap(), 0);
        let entry = idx.lookup("hash123").unwrap();
        assert_eq!(entry.status, ChunkIndexStatus::GcPending);
    }

    #[test]
    fn version_chunks() {
        let idx = BadouIndex::in_memory();
        let version_id = Uuid::new_v4();
        let chunks = vec!["hash1".to_string(), "hash2".to_string(), "hash3".to_string()];
        idx.register_version_chunks(version_id, chunks.clone()).unwrap();
        let retrieved = idx.get_version_chunks(version_id);
        assert_eq!(retrieved, chunks);
    }

    #[test]
    fn manifest_hash_verify() {
        let idx = BadouIndex::in_memory();
        let manifest_id = Uuid::new_v4();
        idx.register_manifest_hash(manifest_id, "abc123".to_string()).unwrap();
        assert!(idx.verify_manifest_hash(manifest_id, "abc123"));
        assert!(!idx.verify_manifest_hash(manifest_id, "wrong"));
    }

    #[test]
    fn gc_candidates_finds_orphaned() {
        let idx = BadouIndex::in_memory();
        let mut entry = make_entry();
        entry.ref_count = 0;
        entry.status = ChunkIndexStatus::GcPending;
        idx.register("orphan", entry).unwrap();
        idx.register("active", make_entry()).unwrap();
        let candidates = idx.gc_candidates();
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].0, "orphan");
    }

    #[test]
    fn consistency_flag() {
        let idx = BadouIndex::in_memory();
        assert!(idx.is_consistent());
        idx.mark_inconsistent().unwrap();
        assert!(!idx.is_consistent());
        idx.mark_consistent().unwrap();
        assert!(idx.is_consistent());
    }

    #[test]
    fn persist_and_reload() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("index.json");
        {
            let idx = BadouIndex::open(&path).unwrap();
            idx.register("persisted", make_entry()).unwrap();
        }
        let idx2 = BadouIndex::open(&path).unwrap();
        let entry = idx2.lookup("persisted").unwrap();
        assert_eq!(entry.size, 1024);
    }
}
