use std::path::PathBuf;
use std::time::Duration;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::common::{
    CompressionProfile, EncryptionProfileRef, FilterRule, ExecutionId, JobId, HashDigest,
    RepositoryId, RetentionPolicyRef, ScheduleRef, VersionId,
};
use super::chunking::ChunkingProfile;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BackupType {
    Full,
    Incremental,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum VersionStatus {
    Success,
    Failed,
    Incomplete,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum JobStatus {
    Active,
    Paused,
    Disabled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupJob {
    pub job_id: JobId,
    pub name: String,
    pub source: BackupSource,
    pub destination: BackupDestination,
    pub schedule: ScheduleRef,
    pub retention_policy: RetentionPolicyRef,
    pub encryption_profile: EncryptionProfileRef,
    pub compression_profile: CompressionProfile,
    #[serde(default)]
    pub chunking_profile: ChunkingProfile,
    pub status: JobStatus,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupSource {
    pub paths: Vec<PathBuf>,
    pub include_rules: Vec<FilterRule>,
    pub exclude_rules: Vec<FilterRule>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupDestination {
    pub repository_id: RepositoryId,
    pub logical_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupVersion {
    pub version_id: VersionId,
    pub job_id: JobId,
    pub version_number: u64,
    pub timestamp: DateTime<Utc>,
    pub backup_type: BackupType,
    pub parent_version_id: Option<VersionId>,
    pub status: VersionStatus,
    pub file_count: u64,
    pub total_size: u64,
    pub stored_size: u64,
    pub manifest_hash: HashDigest,
    pub file_index_hash: HashDigest,
    pub chunk_index_hash: HashDigest,
    pub repo_hash: HashDigest,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupExecution {
    pub execution_id: ExecutionId,
    pub job_id: JobId,
    pub started_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub state: ExecutionState,
    pub progress: f64,
    pub checkpoint: Option<super::common::Checkpoint>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExecutionState {
    Pending,
    Scanning,
    Chunking,
    Encrypting,
    Uploading,
    Committing,
    Verifying,
    Success,
    Failed,
    Paused,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupResult {
    pub version_id: Option<VersionId>,
    pub data_processed: u64,
    pub data_stored: u64,
    pub dedup_ratio: f64,
    pub chunk_count: u64,
    pub file_count: u64,
    pub duration: Duration,
    pub skipped_files: Vec<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupError {
    pub code: String,
    pub message: String,
    pub file_path: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileSnapshotEntry {
    pub path: String,
    pub size: u64,
    pub mtime: DateTime<Utc>,
    pub file_hash: HashDigest,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupSnapshot {
    pub version_id: VersionId,
    pub timestamp: DateTime<Utc>,
    pub files: Vec<FileSnapshotEntry>,
}