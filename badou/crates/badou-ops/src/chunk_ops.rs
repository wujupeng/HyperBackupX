//! Chunk 操作编排：Put/Get/Exists/BatchPut + 去重。

use hbx_core::domain::chunk::ChunkHash;
use hbx_core::domain::common::RepositoryId;
use badou_store::{ChunkStore, ChunkLocation, ChunkStoreError};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ChunkOpsError {
    #[error("chunk store error: {0}")]
    Store(#[from] ChunkStoreError),
    #[error("chunk is referenced and cannot be deleted: {0}")]
    Referenced(String),
    #[error("chunk not found: {0}")]
    NotFound(String),
}

pub struct ChunkOps<'a> {
    repo_id: &'a RepositoryId,
    chunk_store: &'a ChunkStore,
}

impl<'a> ChunkOps<'a> {
    pub fn new(repo_id: &'a RepositoryId, chunk_store: &'a ChunkStore) -> Self {
        Self { repo_id, chunk_store }
    }

    pub fn put_chunk(
        &self,
        chunk_hash: &ChunkHash,
        encrypted_data: &[u8],
    ) -> Result<ChunkLocation, ChunkOpsError> {
        Ok(self.chunk_store.write_chunk(self.repo_id, chunk_hash, encrypted_data)?)
    }

    pub fn get_chunk(&self, chunk_hash: &ChunkHash) -> Result<Vec<u8>, ChunkOpsError> {
        self.chunk_store.read_chunk(self.repo_id, chunk_hash)
            .map_err(|e| match e {
                ChunkStoreError::NotFound(h) => ChunkOpsError::NotFound(h),
                other => ChunkOpsError::Store(other),
            })
    }

    pub fn chunk_exists(&self, chunk_hash: &ChunkHash) -> (bool, u32) {
        let count = self.chunk_store.ref_count(chunk_hash);
        (count > 0, count)
    }

    pub fn batch_put_chunk(
        &self,
        chunks: &[(ChunkHash, Vec<u8>)],
    ) -> Result<Vec<ChunkLocation>, ChunkOpsError> {
        let mut results = Vec::with_capacity(chunks.len());
        for (hash, data) in chunks {
            results.push(self.put_chunk(hash, data)?);
        }
        Ok(results)
    }

    pub fn delete_chunk(&self, chunk_hash: &ChunkHash) -> Result<(), ChunkOpsError> {
        let (_, ref_count) = self.chunk_exists(chunk_hash);
        if ref_count > 0 {
            return Err(ChunkOpsError::Referenced(hex::encode(chunk_hash.0)));
        }
        self.chunk_store.delete_chunk_physical(self.repo_id, chunk_hash)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use badou_engine::format::BadouDataLayout;
    use badou_index::BadouIndex;
    use uuid::Uuid;

    fn make_chunk_store() -> (ChunkStore, RepositoryId) {
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
    fn put_and_get_chunk() {
        let (store, repo_id) = make_chunk_store();
        let ops = ChunkOps::new(&repo_id, &store);
        let data = b"test chunk data";
        let hash = make_hash(data);
        ops.put_chunk(&hash, data).unwrap();
        let retrieved = ops.get_chunk(&hash).unwrap();
        assert_eq!(retrieved, data);
    }

    #[test]
    fn dedup_increments_ref() {
        let (store, repo_id) = make_chunk_store();
        let ops = ChunkOps::new(&repo_id, &store);
        let data = b"dedup test";
        let hash = make_hash(data);
        ops.put_chunk(&hash, data).unwrap();
        let loc = ops.put_chunk(&hash, data).unwrap();
        assert!(loc.deduplicated);
        let (exists, count) = ops.chunk_exists(&hash);
        assert!(exists);
        assert_eq!(count, 2);
    }

    #[test]
    fn delete_referenced_fails() {
        let (store, repo_id) = make_chunk_store();
        let ops = ChunkOps::new(&repo_id, &store);
        let data = b"referenced chunk";
        let hash = make_hash(data);
        ops.put_chunk(&hash, data).unwrap();
        let result = ops.delete_chunk(&hash);
        assert!(result.is_err());
    }

    #[test]
    fn batch_put_works() {
        let (store, repo_id) = make_chunk_store();
        let ops = ChunkOps::new(&repo_id, &store);
        let data1 = b"chunk one";
        let data2 = b"chunk two";
        let hash1 = make_hash(data1);
        let hash2 = make_hash(data2);
        let chunks = vec![(hash1, data1.to_vec()), (hash2, data2.to_vec())];
        let results = ops.batch_put_chunk(&chunks).unwrap();
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn get_nonexistent_fails() {
        let (store, repo_id) = make_chunk_store();
        let ops = ChunkOps::new(&repo_id, &store);
        let hash = ChunkHash([0xff; 32]);
        let result = ops.get_chunk(&hash);
        assert!(result.is_err());
    }
}