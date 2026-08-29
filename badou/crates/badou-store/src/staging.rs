//! 两阶段提交 staging 区与原子切换。

use std::path::{Path, PathBuf};
use hbx_core::domain::common::RepositoryId;
use badou_engine::format::BadouDataLayout;
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum StagingError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("staging already exists for version: {0}")]
    AlreadyExists(Uuid),
    #[error("staging not found for version: {0}")]
    NotFound(Uuid),
    #[error("atomic commit failed: {0}")]
    CommitFailed(String),
}

pub struct StagingDir {
    pub version_id: Uuid,
    pub manifest_staging: PathBuf,
    pub snapshot_staging: PathBuf,
}

pub struct StagingManager {
    layout: BadouDataLayout,
}

impl StagingManager {
    pub fn new(layout: BadouDataLayout) -> Self {
        Self { layout }
    }

    pub fn create_staging(
        &self,
        repo_id: &RepositoryId,
        version_id: Uuid,
    ) -> Result<StagingDir, StagingError> {
        let manifest_staging = self.layout.manifests_dir(repo_id)
            .join(".staging")
            .join(version_id.to_string());
        let snapshot_staging = self.layout.snapshots_dir(repo_id)
            .join(".staging")
            .join(version_id.to_string());

        if manifest_staging.exists() || snapshot_staging.exists() {
            return Err(StagingError::AlreadyExists(version_id));
        }

        std::fs::create_dir_all(&manifest_staging)?;
        std::fs::create_dir_all(&snapshot_staging)?;

        Ok(StagingDir {
            version_id,
            manifest_staging,
            snapshot_staging,
        })
    }

    pub fn write_to_staging(
        &self,
        staging: &StagingDir,
        file_name: &str,
        data: &[u8],
    ) -> Result<PathBuf, StagingError> {
        let manifest_path = staging.manifest_staging.join(file_name);
        let snapshot_path = staging.snapshot_staging.join(file_name);

        if file_name.ends_with(".manifest") {
            std::fs::write(&manifest_path, data)?;
            Ok(manifest_path)
        } else if file_name.ends_with(".snapshot") {
            std::fs::write(&snapshot_path, data)?;
            Ok(snapshot_path)
        } else {
            std::fs::write(&manifest_path, data)?;
            Ok(manifest_path)
        }
    }

    pub fn atomic_commit(
        &self,
        repo_id: &RepositoryId,
        staging: &StagingDir,
    ) -> Result<(), StagingError> {
        self.commit_dir(&staging.manifest_staging, &self.layout.manifests_dir(repo_id))?;
        self.commit_dir(&staging.snapshot_staging, &self.layout.snapshots_dir(repo_id))?;
        self.cleanup_staging(repo_id, staging.version_id)?;
        Ok(())
    }

    fn commit_dir(&self, staging: &Path, target: &Path) -> Result<(), StagingError> {
        if !staging.exists() {
            return Ok(());
        }
        for entry in std::fs::read_dir(staging)? {
            let entry = entry?;
            let src = entry.path();
            if src.is_file() {
                let file_name = entry.file_name();
                let dst = target.join(&file_name);
                std::fs::rename(&src, &dst)
                    .map_err(|e| StagingError::CommitFailed(e.to_string()))?;
            }
        }
        Ok(())
    }

    pub fn cleanup_staging(
        &self,
        repo_id: &RepositoryId,
        version_id: Uuid,
    ) -> Result<(), StagingError> {
        let manifest_staging = self.layout.manifests_dir(repo_id)
            .join(".staging")
            .join(version_id.to_string());
        let snapshot_staging = self.layout.snapshots_dir(repo_id)
            .join(".staging")
            .join(version_id.to_string());

        if manifest_staging.exists() {
            std::fs::remove_dir_all(&manifest_staging)?;
        }
        if snapshot_staging.exists() {
            std::fs::remove_dir_all(&snapshot_staging)?;
        }
        Ok(())
    }

    pub fn staging_exists(
        &self,
        repo_id: &RepositoryId,
        version_id: Uuid,
    ) -> bool {
        let manifest_staging = self.layout.manifests_dir(repo_id)
            .join(".staging")
            .join(version_id.to_string());
        let snapshot_staging = self.layout.snapshots_dir(repo_id)
            .join(".staging")
            .join(version_id.to_string());
        manifest_staging.exists() || snapshot_staging.exists()
    }

    pub fn list_pending_staging(
        &self,
        repo_id: &RepositoryId,
    ) -> Result<Vec<Uuid>, StagingError> {
        let manifest_staging_root = self.layout.manifests_dir(repo_id).join(".staging");
        let mut result = Vec::new();
        if manifest_staging_root.exists() {
            for entry in std::fs::read_dir(&manifest_staging_root)? {
                let entry = entry?;
                if let Ok(uuid) = entry.file_name().to_string_lossy().parse::<Uuid>() {
                    result.push(uuid);
                }
            }
        }
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_layout() -> (BadouDataLayout, RepositoryId) {
        let tmp = tempfile::tempdir().unwrap();
        let layout = BadouDataLayout::new(tmp.path());
        let repo_id = RepositoryId(Uuid::new_v4());
        std::fs::create_dir_all(layout.manifests_dir(&repo_id)).unwrap();
        std::fs::create_dir_all(layout.snapshots_dir(&repo_id)).unwrap();
        std::mem::forget(tmp);
        (layout, repo_id)
    }

    #[test]
    fn create_staging_succeeds() {
        let (layout, repo_id) = make_layout();
        let mgr = StagingManager::new(layout);
        let version_id = Uuid::new_v4();
        let staging = mgr.create_staging(&repo_id, version_id).unwrap();
        assert!(staging.manifest_staging.exists());
        assert!(staging.snapshot_staging.exists());
    }

    #[test]
    fn create_staging_duplicate_fails() {
        let (layout, repo_id) = make_layout();
        let mgr = StagingManager::new(layout);
        let version_id = Uuid::new_v4();
        mgr.create_staging(&repo_id, version_id).unwrap();
        let result = mgr.create_staging(&repo_id, version_id);
        assert!(result.is_err());
    }

    #[test]
    fn write_to_staging_and_commit() {
        let (layout, repo_id) = make_layout();
        let manifests_dir = layout.manifests_dir(&repo_id);
        let snapshots_dir = layout.snapshots_dir(&repo_id);
        let mgr = StagingManager::new(layout);
        let version_id = Uuid::new_v4();
        let staging = mgr.create_staging(&repo_id, version_id).unwrap();

        let manifest_id = Uuid::new_v4();
        let manifest_data = b"manifest content";
        mgr.write_to_staging(&staging, &format!("{}.manifest", manifest_id), manifest_data).unwrap();

        let snapshot_id = Uuid::new_v4();
        let snapshot_data = b"snapshot content";
        mgr.write_to_staging(&staging, &format!("{}.snapshot", snapshot_id), snapshot_data).unwrap();

        mgr.atomic_commit(&repo_id, &staging).unwrap();

        let manifest_final = manifests_dir.join(format!("{}.manifest", manifest_id));
        let snapshot_final = snapshots_dir.join(format!("{}.snapshot", snapshot_id));
        assert!(manifest_final.exists());
        assert!(snapshot_final.exists());
        assert_eq!(std::fs::read(&manifest_final).unwrap(), manifest_data);
        assert_eq!(std::fs::read(&snapshot_final).unwrap(), snapshot_data);
        assert!(!mgr.staging_exists(&repo_id, version_id));
    }

    #[test]
    fn staging_not_visible_before_commit() {
        let (layout, repo_id) = make_layout();
        let manifests_dir = layout.manifests_dir(&repo_id);
        let mgr = StagingManager::new(layout);
        let version_id = Uuid::new_v4();
        let staging = mgr.create_staging(&repo_id, version_id).unwrap();

        let manifest_id = Uuid::new_v4();
        mgr.write_to_staging(&staging, &format!("{}.manifest", manifest_id), b"data").unwrap();

        let manifest_final = manifests_dir.join(format!("{}.manifest", manifest_id));
        assert!(!manifest_final.exists());
    }

    #[test]
    fn concurrent_staging_isolated() {
        let (layout, repo_id) = make_layout();
        let manifests_dir = layout.manifests_dir(&repo_id);
        let mgr = StagingManager::new(layout);
        let v1 = Uuid::new_v4();
        let v2 = Uuid::new_v4();
        let s1 = mgr.create_staging(&repo_id, v1).unwrap();
        let s2 = mgr.create_staging(&repo_id, v2).unwrap();

        let m1 = Uuid::new_v4();
        let m2 = Uuid::new_v4();
        mgr.write_to_staging(&s1, &format!("{}.manifest", m1), b"v1").unwrap();
        mgr.write_to_staging(&s2, &format!("{}.manifest", m2), b"v2").unwrap();

        mgr.atomic_commit(&repo_id, &s1).unwrap();
        mgr.atomic_commit(&repo_id, &s2).unwrap();

        let f1 = manifests_dir.join(format!("{}.manifest", m1));
        let f2 = manifests_dir.join(format!("{}.manifest", m2));
        assert_eq!(std::fs::read(&f1).unwrap(), b"v1");
        assert_eq!(std::fs::read(&f2).unwrap(), b"v2");
    }

    #[test]
    fn cleanup_staging_removes_dirs() {
        let (layout, repo_id) = make_layout();
        let mgr = StagingManager::new(layout);
        let version_id = Uuid::new_v4();
        mgr.create_staging(&repo_id, version_id).unwrap();
        assert!(mgr.staging_exists(&repo_id, version_id));
        mgr.cleanup_staging(&repo_id, version_id).unwrap();
        assert!(!mgr.staging_exists(&repo_id, version_id));
    }

    #[test]
    fn list_pending_staging_returns_versions() {
        let (layout, repo_id) = make_layout();
        let mgr = StagingManager::new(layout);
        let v1 = Uuid::new_v4();
        let v2 = Uuid::new_v4();
        mgr.create_staging(&repo_id, v1).unwrap();
        mgr.create_staging(&repo_id, v2).unwrap();
        let pending = mgr.list_pending_staging(&repo_id).unwrap();
        assert_eq!(pending.len(), 2);
    }
}