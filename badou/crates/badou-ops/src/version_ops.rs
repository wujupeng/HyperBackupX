//! Version 创建与父子关系维护。

use std::collections::HashMap;
use hbx_core::domain::common::{RepositoryId, VersionId};
use badou_engine::domain::version::{Version, VersionStatus};
use parking_lot::RwLock;
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum VersionOpsError {
    #[error("version not found: {0:?}")]
    NotFound(VersionId),
    #[error("invalid state transition: {0:?} -> {1:?}")]
    InvalidTransition(VersionStatus, VersionStatus),
    #[error("parent version not found: {0:?}")]
    ParentNotFound(VersionId),
    #[error("state machine error: {0}")]
    StateMachine(String),
}

pub struct VersionOps {
    versions: RwLock<HashMap<VersionId, Version>>,
    repo_versions: RwLock<HashMap<RepositoryId, Vec<VersionId>>>,
    sequences: RwLock<HashMap<RepositoryId, u64>>,
}

impl Default for VersionOps {
    fn default() -> Self {
        Self::new()
    }
}

impl VersionOps {
    pub fn new() -> Self {
        Self {
            versions: RwLock::new(HashMap::new()),
            repo_versions: RwLock::new(HashMap::new()),
            sequences: RwLock::new(HashMap::new()),
        }
    }

    pub fn create_version(
        &self,
        repo_id: &RepositoryId,
        parent_version_id: Option<VersionId>,
    ) -> Result<Version, VersionOpsError> {
        if let Some(ref parent_id) = parent_version_id {
            if !self.versions.read().contains_key(parent_id) {
                return Err(VersionOpsError::ParentNotFound(parent_version_id.unwrap()));
            }
        }

        let mut seqs = self.sequences.write();
        let seq = *seqs.entry(repo_id.clone()).or_insert(0) + 1;
        *seqs.get_mut(repo_id).unwrap() = seq;
        drop(seqs);

        let version_id = VersionId(Uuid::new_v4());
        let snapshot_id = Uuid::new_v4();
        let version = Version::new(
            version_id.clone(),
            repo_id.clone(),
            snapshot_id,
            parent_version_id,
            seq,
        );

        self.versions.write().insert(version_id.clone(), version.clone());
        self.repo_versions.write()
            .entry(repo_id.clone())
            .or_default()
            .push(version_id);

        Ok(version)
    }

    pub fn get_version(&self, version_id: &VersionId) -> Result<Version, VersionOpsError> {
        self.versions.read()
            .get(version_id)
            .cloned()
            .ok_or(VersionOpsError::NotFound(version_id.clone()))
    }

    pub fn list_versions(&self, repo_id: &RepositoryId) -> Vec<Version> {
        let repo_versions = self.repo_versions.read();
        let versions = self.versions.read();
        repo_versions.get(repo_id)
            .map(|ids| ids.iter().filter_map(|id| versions.get(id).cloned()).collect())
            .unwrap_or_default()
    }

    pub fn transition(
        &self,
        version_id: &VersionId,
        target: VersionStatus,
    ) -> Result<Version, VersionOpsError> {
        let mut versions = self.versions.write();
        let version = versions.get_mut(version_id)
            .ok_or(VersionOpsError::NotFound(version_id.clone()))?;

        if !version.status.can_transition_to(target) {
            return Err(VersionOpsError::InvalidTransition(version.status, target));
        }

        version.status = target;
        Ok(version.clone())
    }

    pub fn start_writing(&self, version_id: &VersionId) -> Result<Version, VersionOpsError> {
        self.transition(version_id, VersionStatus::Writing)
    }

    pub fn version_count(&self, repo_id: &RepositoryId) -> usize {
        self.repo_versions.read()
            .get(repo_id)
            .map(|v| v.len())
            .unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_version_starts_at_sequence_1() {
        let ops = VersionOps::new();
        let repo_id = RepositoryId(Uuid::new_v4());
        let version = ops.create_version(&repo_id, None).unwrap();
        assert_eq!(version.sequence, 1);
        assert_eq!(version.status, VersionStatus::Created);
    }

    #[test]
    fn sequence_increments() {
        let ops = VersionOps::new();
        let repo_id = RepositoryId(Uuid::new_v4());
        let v1 = ops.create_version(&repo_id, None).unwrap();
        let v2 = ops.create_version(&repo_id, Some(v1.version_id.clone())).unwrap();
        let v3 = ops.create_version(&repo_id, Some(v2.version_id.clone())).unwrap();
        assert_eq!(v1.sequence, 1);
        assert_eq!(v2.sequence, 2);
        assert_eq!(v3.sequence, 3);
    }

    #[test]
    fn parent_chain_maintained() {
        let ops = VersionOps::new();
        let repo_id = RepositoryId(Uuid::new_v4());
        let v1 = ops.create_version(&repo_id, None).unwrap();
        let v2 = ops.create_version(&repo_id, Some(v1.version_id.clone())).unwrap();
        assert_eq!(v2.parent_version_id, Some(v1.version_id));
    }

    #[test]
    fn create_with_nonexistent_parent_fails() {
        let ops = VersionOps::new();
        let repo_id = RepositoryId(Uuid::new_v4());
        let fake_parent = VersionId(Uuid::new_v4());
        let result = ops.create_version(&repo_id, Some(fake_parent));
        assert!(result.is_err());
    }

    #[test]
    fn start_writing_transition() {
        let ops = VersionOps::new();
        let repo_id = RepositoryId(Uuid::new_v4());
        let v = ops.create_version(&repo_id, None).unwrap();
        let writing = ops.start_writing(&v.version_id).unwrap();
        assert_eq!(writing.status, VersionStatus::Writing);
    }

    #[test]
    fn list_versions_returns_all() {
        let ops = VersionOps::new();
        let repo_id = RepositoryId(Uuid::new_v4());
        ops.create_version(&repo_id, None).unwrap();
        ops.create_version(&repo_id, None).unwrap();
        ops.create_version(&repo_id, None).unwrap();
        let list = ops.list_versions(&repo_id);
        assert_eq!(list.len(), 3);
    }

    #[test]
    fn version_count_correct() {
        let ops = VersionOps::new();
        let repo_id = RepositoryId(Uuid::new_v4());
        assert_eq!(ops.version_count(&repo_id), 0);
        ops.create_version(&repo_id, None).unwrap();
        ops.create_version(&repo_id, None).unwrap();
        assert_eq!(ops.version_count(&repo_id), 2);
    }
}
