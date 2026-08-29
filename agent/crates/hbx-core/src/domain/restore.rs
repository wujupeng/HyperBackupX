use std::path::PathBuf;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::common::{RestoreId, VersionId};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RestoreJob {
    pub restore_id: RestoreId,
    pub source_version_id: VersionId,
    pub file_selection: FileSelection,
    pub restore_mode: RestoreMode,
    pub target_location: PathBuf,
    pub status: RestoreStatus,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub failed_files: Vec<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FileSelection {
    All,
    FileList(Vec<PathBuf>),
    Glob(String),
    Search(String),
    PathPrefix(String),
    DateRange {
        from: DateTime<Utc>,
        to: DateTime<Utc>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OverwritePolicy {
    #[serde(rename = "skip")]
    Skip,
    #[serde(rename = "overwrite")]
    Overwrite,
    #[serde(rename = "rename")]
    Rename,
}

impl Default for OverwritePolicy {
    fn default() -> Self {
        OverwritePolicy::Skip
    }
}

impl std::str::FromStr for OverwritePolicy {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "skip" => Ok(OverwritePolicy::Skip),
            "overwrite" => Ok(OverwritePolicy::Overwrite),
            "rename" => Ok(OverwritePolicy::Rename),
            _ => Err(format!("invalid overwrite policy: {}", s)),
        }
    }
}

impl From<OverwritePolicy> for RestoreMode {
    fn from(policy: OverwritePolicy) -> Self {
        match policy {
            OverwritePolicy::Skip => RestoreMode::Skip,
            OverwritePolicy::Overwrite => RestoreMode::Overwrite,
            OverwritePolicy::Rename => RestoreMode::Rename,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RestoreMode {
    Overwrite,
    Skip,
    Rename,
    NewLocation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RestoreStatus {
    Pending,
    Running,
    Success,
    PartialFailed,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RestorePoint {
    pub version_id: VersionId,
    pub timestamp: DateTime<Utc>,
    pub version_number: u64,
}