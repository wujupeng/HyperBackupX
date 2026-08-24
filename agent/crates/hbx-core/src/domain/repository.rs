use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::common::{FileAttributes, HashDigest, RepositoryId, VersionId};
use super::chunk::{ChunkHash, ChunkReference};
use super::backup::BackupType;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Repository {
    pub repository_id: RepositoryId,
    pub name: String,
    pub backend_type: BackendType,
    pub connection_config: ConnectionConfig,
    pub format_version: u32,
    pub status: RepositoryStatus,
    pub total_capacity: Option<u64>,
    pub used_capacity: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BackendType {
    Local,
    Smb,
    Sftp,
    Webdav,
    Ftp,
    Ftps,
    S3,
    Obs,
    Minio,
    AzureBlob,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RepositoryStatus {
    Active,
    Readonly,
    Disabled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionConfig {
    pub endpoint: Option<String>,
    pub credential_ref: String,
    pub extra: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepositoryVolume {
    pub volume_id: String,
    pub repository_id: RepositoryId,
    pub bucket_prefix: String,
    pub chunk_count: u64,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Manifest {
    pub version_id: VersionId,
    pub timestamp: DateTime<Utc>,
    pub parent_version_id: Option<VersionId>,
    pub version_number: u64,
    pub backup_type: BackupType,
    pub files: Vec<FileEntry>,
    pub chunk_refs: Vec<ChunkReference>,
    pub hashes: ManifestHashes,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileEntry {
    pub path: String,
    pub size: u64,
    pub modified_at: DateTime<Utc>,
    pub attributes: FileAttributes,
    pub chunks: Vec<ChunkHash>,
    pub file_hash: HashDigest,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManifestHashes {
    pub manifest_hash: HashDigest,
    pub file_index_hash: HashDigest,
    pub chunk_index_hash: HashDigest,
    pub repo_hash: HashDigest,
}