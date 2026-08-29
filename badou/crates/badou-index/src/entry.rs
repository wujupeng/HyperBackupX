use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChunkIndexEntry {
    pub bucket: String,
    pub path: String,
    pub ref_count: u32,
    pub size: u64,
    pub stored_size: u64,
    pub created_at: DateTime<Utc>,
    pub status: ChunkIndexStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChunkIndexStatus {
    Active,
    GcPending,
    Purged,
}

impl ChunkIndexStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::GcPending => "gc_pending",
            Self::Purged => "purged",
        }
    }
}