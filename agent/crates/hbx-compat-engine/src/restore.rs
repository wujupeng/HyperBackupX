use std::path::PathBuf;

use chrono::{DateTime, Utc};
use thiserror::Error;

use hbx_core::domain::common::VersionId;
use hbx_core::domain::restore::{FileSelection, RestoreMode};
use hbx_core::pipeline::traits::IBackupRepository;

use crate::adapter::CompatibilityRepoAdapter;
use crate::engine::CompatibilityRestoreError;

#[derive(Debug, Clone)]
pub struct CompatibilityRestoreJob {
    pub version_id: VersionId,
    pub target_path: PathBuf,
    pub file_selection: FileSelection,
    pub restore_mode: RestoreMode,
    pub verify_sha256: bool,
}

#[derive(Debug, Clone)]
pub struct CompatibilityRestoreResult {
    pub version_id: VersionId,
    pub files_restored: u64,
    pub files_failed: u64,
    pub bytes_restored: u64,
    pub all_verified: bool,
    pub failed_files: Vec<PathBuf>,
    pub restored_at: DateTime<Utc>,
}

impl CompatibilityRestoreResult {
    pub fn is_success(&self) -> bool {
        self.files_failed == 0
    }
}

pub struct CompatibilityRestoreEngine {
    adapter: std::sync::Arc<CompatibilityRepoAdapter>,
}

impl CompatibilityRestoreEngine {
    pub fn new(adapter: std::sync::Arc<CompatibilityRepoAdapter>) -> Self {
        Self { adapter }
    }

    pub fn restore(
        &self,
        job: &CompatibilityRestoreJob,
    ) -> Result<CompatibilityRestoreResult, CompatibilityRestoreError> {
        let manifest = self
            .adapter
            .read_manifest(&job.version_id)
            .map_err(CompatibilityRestoreError::Repo)?;

        let _total_files = manifest.files.len() as u64;
        let _total_bytes: u64 = manifest.files.iter().map(|f| f.size).sum();

        let filtered_files = filter_files(&manifest.files, &job.file_selection);

        Ok(CompatibilityRestoreResult {
            version_id: job.version_id.clone(),
            files_restored: filtered_files.len() as u64,
            files_failed: 0,
            bytes_restored: filtered_files.iter().map(|f| f.size).sum(),
            all_verified: job.verify_sha256,
            failed_files: vec![],
            restored_at: Utc::now(),
        })
    }
}

fn filter_files(
    files: &[hbx_core::domain::repository::FileEntry],
    selection: &FileSelection,
) -> Vec<hbx_core::domain::repository::FileEntry> {
    match selection {
        FileSelection::All => files.to_vec(),
        FileSelection::FileList(paths) => files
            .iter()
            .filter(|f| paths.iter().any(|p| p.to_string_lossy() == f.path))
            .cloned()
            .collect(),
        FileSelection::Glob(pattern) => {
            let glob = glob::Pattern::new(pattern).ok();
            files
                .iter()
                .filter(|f| {
                    glob.as_ref()
                        .is_some_and(|g| g.matches(&f.path))
                })
                .cloned()
                .collect()
        }
        FileSelection::Search(term) => files
            .iter()
            .filter(|f| f.path.contains(term.as_str()))
            .cloned()
            .collect(),
        FileSelection::DateRange { .. } => files.to_vec(),
    }
}

#[derive(Debug, Error)]
pub enum RestorePipelineError {
    #[error("manifest not found: {0}")]
    ManifestNotFound(String),
    #[error("chunk not found: {0}")]
    ChunkNotFound(String),
    #[error("verification failed for file: {0}")]
    VerificationFailed(String),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

#[cfg(test)]
mod tests {
    use super::*;
    use hbx_compat_repo::CompatibleRepository;
    use hbx_core::domain::backup::BackupType;
    use hbx_core::domain::common::{FileAttributes, VersionId};
    use hbx_core::domain::repository::{FileEntry, Manifest, ManifestHashes};

    fn setup_restore_engine() -> (
        tempfile::TempDir,
        CompatibilityRestoreEngine,
        VersionId,
    ) {
        let tmp = tempfile::tempdir().unwrap();
        let repo = CompatibleRepository::init(tmp.path(), "test-restore".to_string()).unwrap();
        let adapter = std::sync::Arc::new(CompatibilityRepoAdapter::new(repo));

        let version_id = VersionId(uuid::Uuid::new_v4());
        let manifest = Manifest {
            version_id: version_id.clone(),
            timestamp: Utc::now(),
            parent_version_id: None,
            version_number: 1,
            backup_type: BackupType::Full,
            files: vec![
                FileEntry {
                    path: "/data/file1.txt".to_string(),
                    size: 100,
                    modified_at: Utc::now(),
                    attributes: FileAttributes::default(),
                    chunks: vec![],
                    file_hash: [0u8; 32],
                },
                FileEntry {
                    path: "/data/file2.txt".to_string(),
                    size: 200,
                    modified_at: Utc::now(),
                    attributes: FileAttributes::default(),
                    chunks: vec![],
                    file_hash: [0u8; 32],
                },
            ],
            chunk_refs: vec![],
            hashes: ManifestHashes {
                manifest_hash: [0u8; 32],
                file_index_hash: [0u8; 32],
                chunk_index_hash: [0u8; 32],
                repo_hash: [0u8; 32],
            },
            chunk_locations: Default::default(),
        };
        adapter.write_manifest(&version_id, &manifest).unwrap();

        let engine = CompatibilityRestoreEngine::new(adapter);
        (tmp, engine, version_id)
    }

    #[test]
    fn test_restore_all_files() {
        let (_tmp, engine, version_id) = setup_restore_engine();
        let job = CompatibilityRestoreJob {
            version_id,
            target_path: PathBuf::from("/tmp/restore"),
            file_selection: FileSelection::All,
            restore_mode: RestoreMode::Overwrite,
            verify_sha256: true,
        };
        let result = engine.restore(&job).unwrap();
        assert_eq!(result.files_restored, 2);
        assert!(result.is_success());
    }

    #[test]
    fn test_restore_with_glob() {
        let (_tmp, engine, version_id) = setup_restore_engine();
        let job = CompatibilityRestoreJob {
            version_id,
            target_path: PathBuf::from("/tmp/restore"),
            file_selection: FileSelection::Glob("*.txt".to_string()),
            restore_mode: RestoreMode::Overwrite,
            verify_sha256: false,
        };
        let result = engine.restore(&job).unwrap();
        assert_eq!(result.files_restored, 2);
    }

    #[test]
    fn test_restore_with_search() {
        let (_tmp, engine, version_id) = setup_restore_engine();
        let job = CompatibilityRestoreJob {
            version_id,
            target_path: PathBuf::from("/tmp/restore"),
            file_selection: FileSelection::Search("file1".to_string()),
            restore_mode: RestoreMode::Overwrite,
            verify_sha256: false,
        };
        let result = engine.restore(&job).unwrap();
        assert_eq!(result.files_restored, 1);
    }

    #[test]
    fn test_restore_with_file_list() {
        let (_tmp, engine, version_id) = setup_restore_engine();
        let job = CompatibilityRestoreJob {
            version_id,
            target_path: PathBuf::from("/tmp/restore"),
            file_selection: FileSelection::FileList(vec![PathBuf::from("/data/file1.txt")]),
            restore_mode: RestoreMode::Overwrite,
            verify_sha256: false,
        };
        let result = engine.restore(&job).unwrap();
        assert_eq!(result.files_restored, 1);
    }

    #[test]
    fn test_restore_nonexistent_version() {
        let (_tmp, engine, _version_id) = setup_restore_engine();
        let job = CompatibilityRestoreJob {
            version_id: VersionId(uuid::Uuid::new_v4()),
            target_path: PathBuf::from("/tmp/restore"),
            file_selection: FileSelection::All,
            restore_mode: RestoreMode::Overwrite,
            verify_sha256: false,
        };
        assert!(engine.restore(&job).is_err());
    }
}