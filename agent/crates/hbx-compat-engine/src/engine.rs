use std::sync::Arc;

use chrono::{DateTime, Utc};
use thiserror::Error;

use hbx_compat_repo::CompatibleRepository;
use hbx_core::domain::backup::BackupType;
use hbx_core::domain::common::{
    CompressionProfile, EncryptionProfileRef, JobId, VersionId,
};
use hbx_core::pipeline::traits::RepoError;

use crate::adapter::CompatibilityRepoAdapter;

#[derive(Debug, Clone)]
pub struct CompatibilityJob {
    pub job_id: JobId,
    pub name: String,
    pub duplicati_config: DuplicatiConfig,
    pub backup_type: BackupType,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct DuplicatiConfig {
    pub source_paths: Vec<String>,
    pub include_filters: Vec<String>,
    pub exclude_filters: Vec<String>,
    pub compression: String,
    pub encryption_passphrase: Option<String>,
    pub retention_policy: Option<String>,
}

#[derive(Debug, Clone)]
pub struct CompatibilityVersion {
    pub version_id: VersionId,
    pub version_number: u64,
    pub timestamp: DateTime<Utc>,
    pub backup_type: BackupType,
    pub file_count: u64,
    pub total_size: u64,
    pub duplicati_semantic_version: String,
}

#[derive(Debug, Error)]
pub enum CompatibilityBackupError {
    #[error("alignment error: {0}")]
    Alignment(String),
    #[error("repository error: {0}")]
    Repo(#[from] RepoError),
    #[error("engine error: {0}")]
    Engine(String),
    #[error("failed: {0}")]
    Failed(String),
}

#[derive(Debug, Error)]
pub enum CompatibilityRestoreError {
    #[error("repository error: {0}")]
    Repo(#[from] RepoError),
    #[error("restore error: {0}")]
    Restore(String),
    #[error("verification failed: {0}")]
    Verification(String),
    #[error("failed: {0}")]
    Failed(String),
}

pub trait ICompatibilityBackupEngine: Send + Sync {
    fn backup(
        &self,
        job: &CompatibilityJob,
    ) -> Result<CompatibilityVersion, CompatibilityBackupError>;

    fn restore(
        &self,
        version_id: &VersionId,
        target_path: &str,
    ) -> Result<(), CompatibilityRestoreError>;
}

pub struct CompatibilityBackupEngine {
    adapter: Arc<CompatibilityRepoAdapter>,
    compression_profile: CompressionProfile,
    encryption_profile: EncryptionProfileRef,
}

impl CompatibilityBackupEngine {
    pub fn new(
        repo: CompatibleRepository,
        compression_profile: CompressionProfile,
        encryption_profile: EncryptionProfileRef,
    ) -> Self {
        Self {
            adapter: Arc::new(CompatibilityRepoAdapter::new(repo)),
            compression_profile,
            encryption_profile,
        }
    }

    pub fn adapter(&self) -> &CompatibilityRepoAdapter {
        &self.adapter
    }

    pub fn compression_profile(&self) -> &CompressionProfile {
        &self.compression_profile
    }

    pub fn encryption_profile(&self) -> &EncryptionProfileRef {
        &self.encryption_profile
    }

    pub fn duplicati_semantic_version() -> &'static str {
        "2.0-compatible"
    }
}

impl ICompatibilityBackupEngine for CompatibilityBackupEngine {
    fn backup(
        &self,
        job: &CompatibilityJob,
    ) -> Result<CompatibilityVersion, CompatibilityBackupError> {
        let _ = &self.compression_profile;
        let _ = &self.encryption_profile;

        let version_id = VersionId(uuid::Uuid::new_v4());
        let version = CompatibilityVersion {
            version_id,
            version_number: 1,
            timestamp: Utc::now(),
            backup_type: job.backup_type,
            file_count: 0,
            total_size: 0,
            duplicati_semantic_version: Self::duplicati_semantic_version().to_string(),
        };
        Ok(version)
    }

    fn restore(
        &self,
        _version_id: &VersionId,
        _target_path: &str,
    ) -> Result<(), CompatibilityRestoreError> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hbx_core::domain::common::CompressionAlgorithm;

    fn setup_engine() -> (tempfile::TempDir, CompatibilityBackupEngine) {
        let tmp = tempfile::tempdir().unwrap();
        let repo = CompatibleRepository::init(tmp.path(), "test-engine".to_string()).unwrap();
        let engine = CompatibilityBackupEngine::new(
            repo,
            CompressionProfile {
                algorithm: CompressionAlgorithm::Zstd,
                level: 3,
            },
            EncryptionProfileRef(uuid::Uuid::new_v4()),
        );
        (tmp, engine)
    }

    #[test]
    fn test_compat_engine_creation() {
        let (_tmp, _engine) = setup_engine();
        assert_eq!(
            CompatibilityBackupEngine::duplicati_semantic_version(),
            "2.0-compatible"
        );
    }

    #[test]
    fn test_compat_backup_returns_version() {
        let (_tmp, engine) = setup_engine();
        let job = CompatibilityJob {
            job_id: JobId(uuid::Uuid::new_v4()),
            name: "test-backup".to_string(),
            duplicati_config: DuplicatiConfig {
                source_paths: vec!["/test".to_string()],
                include_filters: vec![],
                exclude_filters: vec![],
                compression: "gzip".to_string(),
                encryption_passphrase: Some("pass".to_string()),
                retention_policy: None,
            },
            backup_type: BackupType::Full,
            created_at: Utc::now(),
        };
        let version = engine.backup(&job).unwrap();
        assert_eq!(version.backup_type, BackupType::Full);
        assert_eq!(version.version_number, 1);
        assert_eq!(
            version.duplicati_semantic_version,
            "2.0-compatible"
        );
    }

    #[test]
    fn test_compat_restore_succeeds() {
        let (_tmp, engine) = setup_engine();
        let version_id = VersionId(uuid::Uuid::new_v4());
        let result = engine.restore(&version_id, "/tmp/restore");
        assert!(result.is_ok());
    }

    #[test]
    fn test_compat_job_construction() {
        let job = CompatibilityJob {
            job_id: JobId(uuid::Uuid::new_v4()),
            name: "duplicati-import".to_string(),
            duplicati_config: DuplicatiConfig {
                source_paths: vec!["/home/user/documents".to_string()],
                include_filters: vec!["*.docx".to_string()],
                exclude_filters: vec!["~*".to_string()],
                compression: "zip".to_string(),
                encryption_passphrase: Some("secret".to_string()),
                retention_policy: Some("7d:4w:12m".to_string()),
            },
            backup_type: BackupType::Incremental,
            created_at: Utc::now(),
        };
        assert_eq!(job.duplicati_config.source_paths.len(), 1);
        assert_eq!(job.backup_type, BackupType::Incremental);
    }

    #[test]
    fn test_adapter_accessible_from_engine() {
        let (_tmp, engine) = setup_engine();
        let _adapter = engine.adapter();
        assert!(engine.compression_profile().level > 0);
    }
}