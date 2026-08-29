use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use hbx_core::domain::common::VersionId;
use hbx_core::domain::chunk::ChunkHash;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SnapshotStatus {
    Created,
    Writing,
    Sealed,
    Corrupt,
    Deleted,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceMachine {
    pub hostname: String,
    pub os_type: String,
    pub agent_version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupPolicy {
    pub paths: Vec<String>,
    pub excludes: Vec<String>,
    pub includes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptionInfo {
    pub enabled: bool,
    pub algorithm: String,
    pub key_ref: Option<KeyRef>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompressionInfo {
    pub algorithm: String,
    pub level: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerifyInfo {
    pub verified: bool,
    pub verified_at: Option<DateTime<Utc>>,
    pub checksum: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileTree {
    pub root: String,
    pub entries: Vec<FileEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileEntry {
    pub path: String,
    pub size: u64,
    pub is_directory: bool,
    pub chunk_hashes: Vec<ChunkHash>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChunkMapping {
    pub mappings: Vec<ChunkMappingEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChunkMappingEntry {
    pub file_path: String,
    pub offset: u64,
    pub chunk_hash: ChunkHash,
    pub size: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyRef {
    pub key_id: Uuid,
    pub key_source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Snapshot {
    pub snapshot_id: Uuid,
    pub version_id: VersionId,
    pub source_machine: SourceMachine,
    pub backup_policy: BackupPolicy,
    pub file_tree: FileTree,
    pub chunk_mapping: ChunkMapping,
    pub encryption_info: EncryptionInfo,
    pub compression_info: CompressionInfo,
    pub verify_info: VerifyInfo,
    pub status: SnapshotStatus,
    pub created_at: DateTime<Utc>,
    pub total_size: u64,
    pub stored_size: u64,
    pub file_count: u32,
    pub chunk_count: u32,
}

impl Snapshot {
    pub fn new(
        snapshot_id: Uuid,
        version_id: VersionId,
        source_machine: SourceMachine,
    ) -> Self {
        Self {
            snapshot_id,
            version_id,
            source_machine,
            backup_policy: BackupPolicy {
                paths: Vec::new(),
                excludes: Vec::new(),
                includes: Vec::new(),
            },
            file_tree: FileTree {
                root: String::new(),
                entries: Vec::new(),
            },
            chunk_mapping: ChunkMapping {
                mappings: Vec::new(),
            },
            encryption_info: EncryptionInfo {
                enabled: false,
                algorithm: String::new(),
                key_ref: None,
            },
            compression_info: CompressionInfo {
                algorithm: "zstd".to_string(),
                level: 3,
            },
            verify_info: VerifyInfo {
                verified: false,
                verified_at: None,
                checksum: None,
            },
            status: SnapshotStatus::Created,
            created_at: Utc::now(),
            total_size: 0,
            stored_size: 0,
            file_count: 0,
            chunk_count: 0,
        }
    }

    pub fn seal(&mut self) {
        self.status = SnapshotStatus::Sealed;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_new_defaults() {
        let snap = Snapshot::new(
            Uuid::new_v4(),
            VersionId(Uuid::new_v4()),
            SourceMachine {
                hostname: "pc-001".to_string(),
                os_type: "windows".to_string(),
                agent_version: "0.1.0".to_string(),
            },
        );
        assert_eq!(snap.status, SnapshotStatus::Created);
        assert!(!snap.encryption_info.enabled);
        assert_eq!(snap.compression_info.algorithm, "zstd");
    }

    #[test]
    fn snapshot_seal() {
        let mut snap = Snapshot::new(
            Uuid::new_v4(),
            VersionId(Uuid::new_v4()),
            SourceMachine {
                hostname: "pc-001".to_string(),
                os_type: "linux".to_string(),
                agent_version: "0.1.0".to_string(),
            },
        );
        snap.seal();
        assert_eq!(snap.status, SnapshotStatus::Sealed);
    }
}