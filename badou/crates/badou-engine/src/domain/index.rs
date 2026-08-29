use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use hbx_core::domain::common::RepositoryId;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Index {
    pub repo_id: RepositoryId,
    pub chunk_hash_entries: u64,
    pub updated_at: DateTime<Utc>,
    pub consistent: bool,
}

impl Index {
    pub fn new(repo_id: RepositoryId) -> Self {
        Self {
            repo_id,
            chunk_hash_entries: 0,
            updated_at: Utc::now(),
            consistent: true,
        }
    }

    pub fn mark_inconsistent(&mut self) {
        self.consistent = false;
        self.updated_at = Utc::now();
    }

    pub fn mark_consistent(&mut self) {
        self.consistent = true;
        self.updated_at = Utc::now();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    #[test]
    fn index_new() {
        let idx = Index::new(RepositoryId(Uuid::new_v4()));
        assert!(idx.consistent);
        assert_eq!(idx.chunk_hash_entries, 0);
    }
}