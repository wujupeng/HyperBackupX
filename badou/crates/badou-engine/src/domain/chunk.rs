use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use hbx_core::domain::chunk::ChunkHash;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChunkStatus {
    Active,
    GcPending,
    Purged,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyRef {
    pub key_id: Uuid,
    pub key_source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BadouChunk {
    pub chunk_hash: ChunkHash,
    pub size: u64,
    pub stored_size: u64,
    pub ref_count: u32,
    pub status: ChunkStatus,
    pub created_at: DateTime<Utc>,
    pub encryption_ref: Option<KeyRef>,
}

impl BadouChunk {
    pub fn new(chunk_hash: ChunkHash, size: u64, stored_size: u64) -> Self {
        Self {
            chunk_hash,
            size,
            stored_size,
            ref_count: 0,
            status: ChunkStatus::Active,
            created_at: Utc::now(),
            encryption_ref: None,
        }
    }

    pub fn increment_ref(&mut self) {
        self.ref_count += 1;
    }

    pub fn decrement_ref(&mut self) {
        if self.ref_count > 0 {
            self.ref_count -= 1;
        }
        if self.ref_count == 0 {
            self.status = ChunkStatus::GcPending;
        }
    }

    pub fn is_orphaned(&self) -> bool {
        self.ref_count == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chunk_ref_count() {
        let mut c = BadouChunk::new(ChunkHash([0u8; 32]), 1024, 512);
        assert_eq!(c.ref_count, 0);
        c.increment_ref();
        c.increment_ref();
        assert_eq!(c.ref_count, 2);
        c.decrement_ref();
        assert_eq!(c.ref_count, 1);
        assert_eq!(c.status, ChunkStatus::Active);
        c.decrement_ref();
        assert_eq!(c.ref_count, 0);
        assert_eq!(c.status, ChunkStatus::GcPending);
        assert!(c.is_orphaned());
    }
}