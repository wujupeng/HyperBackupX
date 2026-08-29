//! 安全删除 Version：移除引用 → 引用计数 → GC 队列。

use hbx_core::domain::common::{RepositoryId, VersionId};
use badou_engine::domain::version::VersionStatus;
use badou_index::{BadouIndex, ChunkIndexStatus};
use badou_ops::version_ops::VersionOps;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum DeleteError {
    #[error("version not found: {0:?}")]
    NotFound(VersionId),
    #[error("version is immutable and cannot be deleted: {0:?}")]
    Immutable(VersionId),
    #[error("index error: {0}")]
    Index(#[from] badou_index::IndexError),
    #[error("version ops error: {0}")]
    VersionOps(#[from] badou_ops::VersionOpsError),
    #[error("invalid state transition: {0:?} -> Deleted")]
    InvalidTransition(VersionStatus),
}

#[derive(Debug, Clone)]
pub struct DeleteResult {
    pub version_id: VersionId,
    pub removed_chunks: Vec<String>,
    pub gc_candidates: Vec<String>,
    pub kept_shared: Vec<String>,
}

pub struct VersionDeleter<'a> {
    #[allow(dead_code)]
    repo_id: &'a RepositoryId,
    index: &'a BadouIndex,
    version_ops: &'a VersionOps,
}

impl<'a> VersionDeleter<'a> {
    pub fn new(repo_id: &'a RepositoryId, index: &'a BadouIndex, version_ops: &'a VersionOps) -> Self {
        Self { repo_id, index, version_ops }
    }

    pub fn delete_version(&self, version_id: &VersionId) -> Result<DeleteResult, DeleteError> {
        let version = self.version_ops.get_version(version_id)?;

        if version.immutable_until.map(|t| t > chrono::Utc::now()).unwrap_or(false) {
            return Err(DeleteError::Immutable(version_id.clone()));
        }

        let chunk_hashes = self.index.get_version_chunks(version_id.0);

        let mut removed_chunks = Vec::new();
        let mut gc_candidates = Vec::new();
        let mut kept_shared = Vec::new();

        for chunk_hash_hex in &chunk_hashes {
            let count = self.index.decrement_ref_count(chunk_hash_hex)?;
            removed_chunks.push(chunk_hash_hex.clone());

            if count == 0 {
                self.index.update_status(chunk_hash_hex, ChunkIndexStatus::GcPending)?;
                gc_candidates.push(chunk_hash_hex.clone());
            } else {
                kept_shared.push(chunk_hash_hex.clone());
            }
        }

        self.index.register_version_chunks(version_id.0, vec![])?;

        Ok(DeleteResult {
            version_id: version_id.clone(),
            removed_chunks,
            gc_candidates,
            kept_shared,
        })
    }

    pub fn remove_references(&self, version_id: &VersionId) -> Result<Vec<String>, DeleteError> {
        let chunk_hashes = self.index.get_version_chunks(version_id.0);
        let mut removed = Vec::new();

        for chunk_hash_hex in &chunk_hashes {
            self.index.decrement_ref_count(chunk_hash_hex)?;
            removed.push(chunk_hash_hex.clone());
        }

        self.index.register_version_chunks(version_id.0, vec![])?;
        Ok(removed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    fn make_env() -> (BadouIndex, VersionOps, RepositoryId) {
        let index = BadouIndex::in_memory();
        let version_ops = VersionOps::new();
        let repo_id = RepositoryId(Uuid::new_v4());
        (index, version_ops, repo_id)
    }

    fn register_chunk(index: &BadouIndex, hash: &str, ref_count: u32) {
        use badou_index::ChunkIndexStatus;
        use chrono::Utc;
        let entry = badou_index::ChunkIndexEntry {
            bucket: "ab".to_string(),
            path: format!("/tmp/ab/{}.chunk", hash),
            ref_count,
            size: 1024,
            stored_size: 512,
            created_at: Utc::now(),
            status: ChunkIndexStatus::Active,
        };
        index.register(hash, entry).unwrap();
    }

    #[test]
    fn delete_version_removes_references() {
        let (index, version_ops, repo_id) = make_env();
        let deleter = VersionDeleter::new(&repo_id, &index, &version_ops);

        let version = version_ops.create_version(&repo_id, None).unwrap();
        register_chunk(&index, "hash1", 1);
        register_chunk(&index, "hash2", 1);
        index.register_version_chunks(version.version_id.0, vec!["hash1".to_string(), "hash2".to_string()]).unwrap();

        let result = deleter.delete_version(&version.version_id).unwrap();
        assert_eq!(result.removed_chunks.len(), 2);
        assert_eq!(result.gc_candidates.len(), 2);
    }

    #[test]
    fn delete_shared_chunk_keeps_it() {
        let (index, version_ops, repo_id) = make_env();
        let deleter = VersionDeleter::new(&repo_id, &index, &version_ops);

        let v1 = version_ops.create_version(&repo_id, None).unwrap();
        let v2 = version_ops.create_version(&repo_id, Some(v1.version_id.clone())).unwrap();

        register_chunk(&index, "shared_hash", 2);
        index.register_version_chunks(v1.version_id.0, vec!["shared_hash".to_string()]).unwrap();
        index.register_version_chunks(v2.version_id.0, vec!["shared_hash".to_string()]).unwrap();

        let result = deleter.delete_version(&v1.version_id).unwrap();
        assert!(result.kept_shared.contains(&"shared_hash".to_string()));
        assert!(!result.gc_candidates.contains(&"shared_hash".to_string()));
    }

    #[test]
    fn delete_nonexistent_fails() {
        let (index, version_ops, repo_id) = make_env();
        let deleter = VersionDeleter::new(&repo_id, &index, &version_ops);
        let fake_id = VersionId(Uuid::new_v4());
        let result = deleter.delete_version(&fake_id);
        assert!(result.is_err());
    }
}