use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::common::VersionId;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerifyReport {
    pub version_id: VersionId,
    pub mode: VerifyMode,
    pub started_at: DateTime<Utc>,
    pub completed_at: DateTime<Utc>,
    pub total_checked: u64,
    pub passed: u64,
    pub failed: u64,
    pub failures: Vec<VerifyFailure>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerifyFailure {
    pub item_type: VerifyItemType,
    pub identifier: String,
    pub expected_hash: super::common::HashDigest,
    pub actual_hash: super::common::HashDigest,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum VerifyItemType {
    File,
    Chunk,
    Manifest,
    Repo,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum VerifyMode {
    Quick,
    Random { ratio: f64 },
    Full,
    Deep,
}