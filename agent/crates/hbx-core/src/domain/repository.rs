use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::common::{FileAttributes, HashDigest, RepositoryId, VersionId};
use super::chunk::{ChunkHash, ChunkLocation, ChunkReference};
use std::collections::HashMap;
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
    GoogleCloudStorage,
    OpenStack,
    BaDou,
}

impl std::fmt::Display for BackendType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BackendType::Local => write!(f, "local"),
            BackendType::Smb => write!(f, "smb"),
            BackendType::Sftp => write!(f, "sftp"),
            BackendType::Webdav => write!(f, "webdav"),
            BackendType::Ftp => write!(f, "ftp"),
            BackendType::Ftps => write!(f, "ftps"),
            BackendType::S3 => write!(f, "s3"),
            BackendType::Obs => write!(f, "obs"),
            BackendType::Minio => write!(f, "minio"),
            BackendType::AzureBlob => write!(f, "azure-blob"),
            BackendType::GoogleCloudStorage => write!(f, "gcs"),
            BackendType::OpenStack => write!(f, "openstack"),
            BackendType::BaDou => write!(f, "badou"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObjectInfo {
    pub key: String,
    pub size: u64,
    pub last_modified: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObjectListPage {
    pub objects: Vec<ObjectInfo>,
    pub next_token: Option<PageToken>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PageToken(pub String);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConnectionTestResult {
    Passed,
    Failed,
    NotSupported,
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
    #[serde(default)]
    pub chunk_locations: HashMap<String, ChunkLocation>,
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