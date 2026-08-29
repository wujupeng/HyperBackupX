use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use hbx_core::domain::chunk::ChunkHash;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChunkRef {
    pub chunk_hash: ChunkHash,
    pub offset: u64,
    pub size: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Manifest {
    pub manifest_id: Uuid,
    pub snapshot_id: Uuid,
    pub file_tree: Vec<u8>,
    pub chunk_refs: Vec<ChunkRef>,
    pub created_at: DateTime<Utc>,
}

impl Manifest {
    pub fn new(snapshot_id: Uuid, file_tree: Vec<u8>, chunk_refs: Vec<ChunkRef>) -> Self {
        Self {
            manifest_id: Uuid::new_v4(),
            snapshot_id,
            file_tree,
            chunk_refs,
            created_at: Utc::now(),
        }
    }

    pub fn chunk_count(&self) -> usize {
        self.chunk_refs.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_new() {
        let m = Manifest::new(Uuid::new_v4(), vec![], vec![]);
        assert_eq!(m.chunk_count(), 0);
    }
}