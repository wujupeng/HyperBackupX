use std::path::PathBuf;
use std::time::Duration;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub type HashDigest = [u8; 32];

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Base64Bytes(pub Vec<u8>);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Checkpoint {
    pub progress: f64,
    pub pending_files: usize,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScheduleRef(pub Uuid);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetentionPolicyRef(pub Uuid);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptionProfileRef(pub Uuid);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RepositoryId(pub Uuid);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompressionProfile {
    pub algorithm: CompressionAlgorithm,
    pub level: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CompressionAlgorithm {
    Zstd,
    Lz4,
    None,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimit {
    pub upload_bytes_per_sec: Option<u64>,
    pub download_bytes_per_sec: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionSummary {
    pub version_id: Uuid,
    pub version_number: u64,
    pub timestamp: DateTime<Utc>,
    pub backup_type: super::backup::BackupType,
    pub total_size: u64,
    pub stored_size: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoLock {
    pub lock_id: Uuid,
    pub holder: String,
    pub acquired_at: DateTime<Utc>,
    pub ttl: Duration,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LockOperation {
    Backup,
    Restore,
    Verify,
    Compact,
    Migrate,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanEstimate {
    pub total_files: u64,
    pub total_size: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum FilterRule {
    Glob(String),
    Regex(String),
    PathPrefix(String),
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FileAttributes {
    pub is_directory: bool,
    pub is_hidden: bool,
    pub is_system: bool,
    pub is_read_only: bool,
    pub windows_acl: Option<Vec<u8>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct JobId(pub Uuid);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct VersionId(pub Uuid);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ExecutionId(pub Uuid);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ProfileId(pub Uuid);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ScheduleId(pub Uuid);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PolicyId(pub Uuid);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DeviceId(pub Uuid);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RestoreId(pub Uuid);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct UserId(pub Uuid);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RoleId(pub Uuid);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct OrganizationId(pub Uuid);

pub fn path_bufs() -> Vec<PathBuf> {
    Vec::new()
}