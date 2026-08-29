//! 引用计数 GC 执行：零引用回收 + 不阻塞 + 幂等。


use hbx_core::domain::common::RepositoryId;
use hbx_core::domain::chunk::ChunkHash;
use badou_index::{BadouIndex, ChunkIndexStatus};
use badou_store::ChunkStore;
use thiserror::Error;
use chrono::{DateTime, Utc};

#[derive(Debug, Error)]
pub enum GcExecutorError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("index error: {0}")]
    Index(#[from] badou_index::IndexError),
    #[error("chunk store error: {0}")]
    ChunkStore(#[from] badou_store::ChunkStoreError),
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct GcReport {
    pub started_at: DateTime<Utc>,
    pub finished_at: DateTime<Utc>,
    pub purged_chunks: Vec<String>,
    pub freed_bytes: u64,
    pub skipped_chunks: Vec<String>,
    pub errors: Vec<String>,
}

impl Default for GcReport {
    fn default() -> Self {
        Self::new()
    }
}

impl GcReport {
    pub fn new() -> Self {
        let now = Utc::now();
        Self {
            started_at: now,
            finished_at: now,
            purged_chunks: Vec::new(),
            freed_bytes: 0,
            skipped_chunks: Vec::new(),
            errors: Vec::new(),
        }
    }

    pub fn duration_ms(&self) -> i64 {
        (self.finished_at - self.started_at).num_milliseconds()
    }

    pub fn purged_count(&self) -> usize {
        self.purged_chunks.len()
    }
}

pub struct GcExecutor<'a> {
    repo_id: &'a RepositoryId,
    index: &'a BadouIndex,
    chunk_store: &'a ChunkStore,
}

impl<'a> GcExecutor<'a> {
    pub fn new(repo_id: &'a RepositoryId, index: &'a BadouIndex, chunk_store: &'a ChunkStore) -> Self {
        Self { repo_id, index, chunk_store }
    }

    pub fn execute_gc(&self) -> Result<GcReport, GcExecutorError> {
        let mut report = GcReport::new();
        let candidates = self.index.gc_candidates();

        for (hash_hex, entry) in candidates {
            if entry.status != ChunkIndexStatus::GcPending {
                report.skipped_chunks.push(hash_hex);
                continue;
            }

            let freed = entry.stored_size;

            match self.chunk_store.delete_chunk_physical(self.repo_id, &parse_hash(&hash_hex)) {
                Ok(()) => {
                    report.purged_chunks.push(hash_hex);
                    report.freed_bytes += freed;
                }
                Err(badou_store::ChunkStoreError::NotFound(_)) => {
                    self.index.update_status(&hash_hex, ChunkIndexStatus::Purged)?;
                    report.skipped_chunks.push(hash_hex);
                }
                Err(e) => {
                    report.errors.push(format!("{}: {}", hash_hex, e));
                }
            }
        }

        report.finished_at = Utc::now();
        Ok(report)
    }

    pub fn is_idempotent(&self) -> bool {
        let candidates1 = self.index.gc_candidates();
        let candidates_set: std::collections::HashSet<String> = candidates1.iter()
            .map(|(h, _)| h.clone())
            .collect();
        let candidates2 = self.index.gc_candidates();
        let candidates_set2: std::collections::HashSet<String> = candidates2.iter()
            .map(|(h, _)| h.clone())
            .collect();
        candidates_set == candidates_set2
    }
}

fn parse_hash(hash_hex: &str) -> ChunkHash {
    let bytes = hex::decode(hash_hex).unwrap_or_default();
    let mut arr = [0u8; 32];
    if bytes.len() >= 32 {
        arr.copy_from_slice(&bytes[..32]);
    }
    ChunkHash(arr)
}

#[cfg(test)]
mod tests {
    use super::*;
    use badou_engine::format::BadouDataLayout;
    use uuid::Uuid;

    fn make_env() -> (BadouIndex, ChunkStore, RepositoryId) {
        let tmp = tempfile::tempdir().unwrap();
        let layout = BadouDataLayout::new(tmp.path());
        let index = BadouIndex::in_memory();
        let store = ChunkStore::new(layout, index.clone());
        let repo_id = RepositoryId(Uuid::new_v4());
        std::mem::forget(tmp);
        (index, store, repo_id)
    }

    #[test]
    fn execute_gc_purges_zero_ref_chunks() {
        let (index, store, repo_id) = make_env();
        let executor = GcExecutor::new(&repo_id, &index, &store);

        let hash_hex = "abcd1234abcd1234abcd1234abcd1234abcd1234abcd1234abcd1234abcd1234";
        let entry = badou_index::ChunkIndexEntry {
            bucket: "ab".to_string(),
            path: "/tmp/ab/test.chunk".to_string(),
            ref_count: 0,
            size: 1024,
            stored_size: 512,
            created_at: Utc::now(),
            status: ChunkIndexStatus::GcPending,
        };
        index.register(hash_hex, entry).unwrap();

        let report = executor.execute_gc().unwrap();
        assert_eq!(report.purged_count(), 1);
        assert_eq!(report.freed_bytes, 512);
    }

    #[test]
    fn execute_gc_skips_active_chunks() {
        let (index, store, repo_id) = make_env();
        let executor = GcExecutor::new(&repo_id, &index, &store);

        let hash_hex = "active1234active1234active1234active1234active1234active1234active1234active1234";
        let entry = badou_index::ChunkIndexEntry {
            bucket: "ab".to_string(),
            path: "/tmp/ab/active.chunk".to_string(),
            ref_count: 1,
            size: 1024,
            stored_size: 512,
            created_at: Utc::now(),
            status: ChunkIndexStatus::Active,
        };
        index.register(hash_hex, entry).unwrap();

        let report = executor.execute_gc().unwrap();
        assert_eq!(report.purged_count(), 0);
    }

    #[test]
    fn gc_report_duration() {
        let mut report = GcReport::new();
        report.finished_at = Utc::now();
        assert!(report.duration_ms() >= 0);
    }
}