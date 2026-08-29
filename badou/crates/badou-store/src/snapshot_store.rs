//! Snapshot 自描述对象持久化：staging 两阶段 + SEALED 不可变。

use std::path::PathBuf;
use hbx_core::domain::common::RepositoryId;
use badou_engine::format::BadouDataLayout;
use badou_engine::domain::snapshot::{Snapshot, SnapshotStatus};
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum SnapshotStoreError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("serde error: {0}")]
    Serde(#[from] serde_json::Error),
    #[error("snapshot not found: {0}")]
    NotFound(Uuid),
    #[error("snapshot is sealed and cannot be modified: {0}")]
    SealedImmutable(Uuid),
    #[error("verify_info missing for snapshot: {0}")]
    VerifyInfoMissing(Uuid),
}

pub struct SnapshotStore {
    layout: BadouDataLayout,
}

impl SnapshotStore {
    pub fn new(layout: BadouDataLayout) -> Self {
        Self { layout }
    }

    pub fn snapshot_path(&self, repo_id: &RepositoryId, snapshot_id: Uuid) -> PathBuf {
        self.layout.snapshots_dir(repo_id).join(format!("{}.snapshot", snapshot_id))
    }

    pub fn write_snapshot(
        &self,
        repo_id: &RepositoryId,
        snapshot: &Snapshot,
    ) -> Result<Uuid, SnapshotStoreError> {
        let snapshot_id = snapshot.snapshot_id;

        if snapshot.status == SnapshotStatus::Sealed && !snapshot.verify_info.verified {
            return Err(SnapshotStoreError::VerifyInfoMissing(snapshot_id));
        }

        let final_path = self.snapshot_path(repo_id, snapshot_id);
        let staging_path = final_path.with_extension("snapshot.staging");

        let json = serde_json::to_vec(snapshot)?;
        std::fs::write(&staging_path, &json)?;
        std::fs::rename(&staging_path, &final_path)?;

        Ok(snapshot_id)
    }

    pub fn read_snapshot(
        &self,
        repo_id: &RepositoryId,
        snapshot_id: Uuid,
    ) -> Result<Snapshot, SnapshotStoreError> {
        let path = self.snapshot_path(repo_id, snapshot_id);
        if !path.exists() {
            return Err(SnapshotStoreError::NotFound(snapshot_id));
        }
        let data = std::fs::read(&path)?;
        let snapshot: Snapshot = serde_json::from_slice(&data)?;
        Ok(snapshot)
    }

    pub fn seal_snapshot(
        &self,
        repo_id: &RepositoryId,
        snapshot_id: Uuid,
    ) -> Result<(), SnapshotStoreError> {
        let mut snapshot = self.read_snapshot(repo_id, snapshot_id)?;
        if snapshot.status == SnapshotStatus::Sealed {
            return Err(SnapshotStoreError::SealedImmutable(snapshot_id));
        }
        if !snapshot.verify_info.verified {
            return Err(SnapshotStoreError::VerifyInfoMissing(snapshot_id));
        }
        snapshot.status = SnapshotStatus::Sealed;
        let json = serde_json::to_vec(&snapshot)?;
        std::fs::write(self.snapshot_path(repo_id, snapshot_id), &json)?;
        Ok(())
    }

    pub fn snapshot_exists(&self, repo_id: &RepositoryId, snapshot_id: Uuid) -> bool {
        self.snapshot_path(repo_id, snapshot_id).exists()
    }

    pub fn verify_sealed_immutability(
        &self,
        repo_id: &RepositoryId,
        snapshot_id: Uuid,
    ) -> Result<bool, SnapshotStoreError> {
        let snapshot = self.read_snapshot(repo_id, snapshot_id)?;
        Ok(snapshot.status == SnapshotStatus::Sealed)
    }

    pub fn check_verify_info(
        &self,
        repo_id: &RepositoryId,
        snapshot_id: Uuid,
    ) -> Result<bool, SnapshotStoreError> {
        let snapshot = self.read_snapshot(repo_id, snapshot_id)?;
        Ok(snapshot.verify_info.verified)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use badou_engine::domain::snapshot::{SourceMachine, VerifyInfo};
    use hbx_core::domain::common::VersionId;
    use chrono::Utc;

    fn make_layout() -> (BadouDataLayout, RepositoryId) {
        let tmp = tempfile::tempdir().unwrap();
        let layout = BadouDataLayout::new(tmp.path());
        let repo_id = RepositoryId(Uuid::new_v4());
        std::fs::create_dir_all(layout.snapshots_dir(&repo_id)).unwrap();
        std::mem::forget(tmp);
        (layout, repo_id)
    }

    fn make_snapshot() -> Snapshot {
        let mut snap = Snapshot::new(
            Uuid::new_v4(),
            VersionId(Uuid::new_v4()),
            SourceMachine {
                hostname: "test".to_string(),
                os_type: "linux".to_string(),
                agent_version: "0.1.0".to_string(),
            },
        );
        snap.verify_info = VerifyInfo {
            verified: true,
            verified_at: Some(Utc::now()),
            checksum: Some("abc123".to_string()),
        };
        snap
    }

    #[test]
    fn write_and_read_snapshot() {
        let (layout, repo_id) = make_layout();
        let store = SnapshotStore::new(layout);
        let snapshot = make_snapshot();
        let id = store.write_snapshot(&repo_id, &snapshot).unwrap();
        let read_back = store.read_snapshot(&repo_id, id).unwrap();
        assert_eq!(read_back.snapshot_id, snapshot.snapshot_id);
    }

    #[test]
    fn seal_snapshot_succeeds_with_verify() {
        let (layout, repo_id) = make_layout();
        let store = SnapshotStore::new(layout);
        let snapshot = make_snapshot();
        let id = store.write_snapshot(&repo_id, &snapshot).unwrap();
        store.seal_snapshot(&repo_id, id).unwrap();
        assert!(store.verify_sealed_immutability(&repo_id, id).unwrap());
    }

    #[test]
    fn seal_fails_without_verify_info() {
        let (layout, repo_id) = make_layout();
        let store = SnapshotStore::new(layout);
        let mut snapshot = make_snapshot();
        snapshot.verify_info.verified = false;
        let id = store.write_snapshot(&repo_id, &snapshot).unwrap();
        let result = store.seal_snapshot(&repo_id, id);
        assert!(result.is_err());
    }

    #[test]
    fn sealed_snapshot_cannot_be_resealed() {
        let (layout, repo_id) = make_layout();
        let store = SnapshotStore::new(layout);
        let snapshot = make_snapshot();
        let id = store.write_snapshot(&repo_id, &snapshot).unwrap();
        store.seal_snapshot(&repo_id, id).unwrap();
        let result = store.seal_snapshot(&repo_id, id);
        assert!(result.is_err());
    }

    #[test]
    fn read_nonexistent_fails() {
        let (layout, repo_id) = make_layout();
        let store = SnapshotStore::new(layout);
        let result = store.read_snapshot(&repo_id, Uuid::new_v4());
        assert!(result.is_err());
    }
}