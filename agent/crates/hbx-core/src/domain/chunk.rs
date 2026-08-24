use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::common::VersionId;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ChunkHash(pub [u8; 32]);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ChunkId(pub Uuid);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Chunk {
    pub chunk_id: ChunkId,
    pub hash: ChunkHash,
    pub size: u64,
    pub stored_size: u64,
    pub storage_location: ChunkLocation,
    pub reference_count: u64,
    pub created_at: DateTime<Utc>,
    pub encryption_tag: [u8; 16],
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ChunkLocation {
    pub bucket: String,
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChunkReference {
    pub hash: ChunkHash,
    pub version_id: VersionId,
    pub file_path: String,
    pub offset: u64,
}