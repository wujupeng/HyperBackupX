//! Snapshot/Manifest/Version 查询操作。

use hbx_core::domain::common::RepositoryId;
use badou_store::{SnapshotStore, ManifestStore, SnapshotStoreError, ManifestStoreError};
use badou_engine::domain::snapshot::Snapshot;
use badou_engine::domain::manifest::Manifest;
use uuid::Uuid;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum SnapshotOpsError {
    #[error("snapshot store error: {0}")]
    Store(#[from] SnapshotStoreError),
    #[error("manifest store error: {0}")]
    Manifest(#[from] ManifestStoreError),
    #[error("not found: {0}")]
    NotFound(Uuid),
}

pub struct SnapshotOps<'a> {
    repo_id: &'a RepositoryId,
    snapshot_store: &'a SnapshotStore,
    manifest_store: &'a ManifestStore,
}

impl<'a> SnapshotOps<'a> {
    pub fn new(
        repo_id: &'a RepositoryId,
        snapshot_store: &'a SnapshotStore,
        manifest_store: &'a ManifestStore,
    ) -> Self {
        Self { repo_id, snapshot_store, manifest_store }
    }

    pub fn get_snapshot(&self, snapshot_id: Uuid) -> Result<Snapshot, SnapshotOpsError> {
        Ok(self.snapshot_store.read_snapshot(self.repo_id, snapshot_id)?)
    }

    pub fn get_manifest(&self, manifest_id: Uuid) -> Result<Manifest, SnapshotOpsError> {
        Ok(self.manifest_store.read_manifest(self.repo_id, manifest_id)?)
    }

    pub fn snapshot_exists(&self, snapshot_id: Uuid) -> bool {
        self.snapshot_store.snapshot_exists(self.repo_id, snapshot_id)
    }

    pub fn manifest_exists(&self, manifest_id: Uuid) -> bool {
        self.manifest_store.manifest_exists(self.repo_id, manifest_id)
    }

    pub fn is_sealed(&self, snapshot_id: Uuid) -> Result<bool, SnapshotOpsError> {
        Ok(self.snapshot_store.verify_sealed_immutability(self.repo_id, snapshot_id)?)
    }

    pub fn is_verified(&self, snapshot_id: Uuid) -> Result<bool, SnapshotOpsError> {
        Ok(self.snapshot_store.check_verify_info(self.repo_id, snapshot_id)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hbx_core::domain::common::VersionId;
    use badou_engine::format::BadouDataLayout;
    use badou_engine::domain::snapshot::{SourceMachine, VerifyInfo};
    use chrono::Utc;

    fn make_stores() -> (SnapshotStore, ManifestStore, RepositoryId) {
        let tmp = tempfile::tempdir().unwrap();
        let layout = BadouDataLayout::new(tmp.path());
        let repo_id = RepositoryId(Uuid::new_v4());
        std::fs::create_dir_all(layout.snapshots_dir(&repo_id)).unwrap();
        std::fs::create_dir_all(layout.manifests_dir(&repo_id)).unwrap();
        let snap_store = SnapshotStore::new(layout.clone());
        let manifest_store = ManifestStore::new(layout);
        std::mem::forget(tmp);
        (snap_store, manifest_store, repo_id)
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
            checksum: Some("abc".to_string()),
        };
        snap
    }

    #[test]
    fn get_snapshot_succeeds() {
        let (snap_store, manifest_store, repo_id) = make_stores();
        let ops = SnapshotOps::new(&repo_id, &snap_store, &manifest_store);
        let snap = make_snapshot();
        snap_store.write_snapshot(&repo_id, &snap).unwrap();
        let retrieved = ops.get_snapshot(snap.snapshot_id).unwrap();
        assert_eq!(retrieved.snapshot_id, snap.snapshot_id);
    }

    #[test]
    fn snapshot_exists_check() {
        let (snap_store, manifest_store, repo_id) = make_stores();
        let ops = SnapshotOps::new(&repo_id, &snap_store, &manifest_store);
        let snap = make_snapshot();
        assert!(!ops.snapshot_exists(snap.snapshot_id));
        snap_store.write_snapshot(&repo_id, &snap).unwrap();
        assert!(ops.snapshot_exists(snap.snapshot_id));
    }

    #[test]
    fn is_sealed_check() {
        let (snap_store, manifest_store, repo_id) = make_stores();
        let ops = SnapshotOps::new(&repo_id, &snap_store, &manifest_store);
        let snap = make_snapshot();
        snap_store.write_snapshot(&repo_id, &snap).unwrap();
        assert!(!ops.is_sealed(snap.snapshot_id).unwrap());
        snap_store.seal_snapshot(&repo_id, snap.snapshot_id).unwrap();
        assert!(ops.is_sealed(snap.snapshot_id).unwrap());
    }
}