//! 不可变保留与 GC 冲突仲裁。

use hbx_core::domain::common::VersionId;
use badou_engine::domain::version::VersionStatus;
use badou_ops::version_ops::VersionOps;
use chrono::{DateTime, Utc};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ImmutableGcError {
    #[error("version {0:?} is immutable until {1}")]
    ImmutableBlocked(VersionId, DateTime<Utc>),
    #[error("version not found: {0:?}")]
    NotFound(VersionId),
    #[error("version ops error: {0}")]
    VersionOps(#[from] badou_ops::VersionOpsError),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GcDecision {
    Allow,
    Block { reason: String },
    Defer { until: DateTime<Utc> },
}

pub struct ImmutableGcGuard<'a> {
    version_ops: &'a VersionOps,
}

impl<'a> ImmutableGcGuard<'a> {
    pub fn new(version_ops: &'a VersionOps) -> Self {
        Self { version_ops }
    }

    pub fn check_version(&self, version_id: &VersionId) -> Result<GcDecision, ImmutableGcError> {
        let version = self.version_ops.get_version(version_id)?;

        if let Some(immutable_until) = version.immutable_until {
            if immutable_until > Utc::now() {
                return Ok(GcDecision::Block {
                    reason: format!("immutable until {}", immutable_until),
                });
            }
        }

        if version.status == VersionStatus::Sealed {
            return Ok(GcDecision::Allow);
        }

        Ok(GcDecision::Allow)
    }

    pub fn check_chunk_deletion(
        &self,
        version_id: &VersionId,
        chunk_ref_count: u32,
    ) -> Result<GcDecision, ImmutableGcError> {
        if chunk_ref_count > 0 {
            return Ok(GcDecision::Block {
                reason: format!("chunk has {} references", chunk_ref_count),
            });
        }

        self.check_version(version_id)
    }

    pub fn filter_gc_candidates(
        &self,
        candidates: &[(String, u32, Option<VersionId>)],
    ) -> Result<Vec<String>, ImmutableGcError> {
        let mut allowed = Vec::new();
        for (chunk_hash, ref_count, version_id) in candidates {
            if *ref_count > 0 {
                continue;
            }

            if let Some(vid) = version_id {
                match self.check_version(vid)? {
                    GcDecision::Allow => allowed.push(chunk_hash.clone()),
                    GcDecision::Block { .. } => {}
                    GcDecision::Defer { .. } => {}
                }
            } else {
                allowed.push(chunk_hash.clone());
            }
        }
        Ok(allowed)
    }

    pub fn priority_order() -> &'static [&'static str] {
        &["immutable_retention", "retention_policy", "gc", "admin_delete"]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hbx_core::domain::common::RepositoryId;
    use uuid::Uuid;

    fn make_ops() -> VersionOps {
        VersionOps::new()
    }

    #[test]
    fn allow_non_immutable_version() {
        let ops = make_ops();
        let guard = ImmutableGcGuard::new(&ops);
        let repo_id = RepositoryId(Uuid::new_v4());
        let version = ops.create_version(&repo_id, None).unwrap();
        let decision = guard.check_version(&version.version_id).unwrap();
        assert_eq!(decision, GcDecision::Allow);
    }

    #[test]
    fn block_chunk_with_references() {
        let ops = make_ops();
        let guard = ImmutableGcGuard::new(&ops);
        let repo_id = RepositoryId(Uuid::new_v4());
        let version = ops.create_version(&repo_id, None).unwrap();
        let decision = guard.check_chunk_deletion(&version.version_id, 2).unwrap();
        assert!(matches!(decision, GcDecision::Block { .. }));
    }

    #[test]
    fn allow_zero_ref_non_immutable() {
        let ops = make_ops();
        let guard = ImmutableGcGuard::new(&ops);
        let repo_id = RepositoryId(Uuid::new_v4());
        let version = ops.create_version(&repo_id, None).unwrap();
        let decision = guard.check_chunk_deletion(&version.version_id, 0).unwrap();
        assert_eq!(decision, GcDecision::Allow);
    }

    #[test]
    fn filter_removes_referenced_chunks() {
        let ops = make_ops();
        let guard = ImmutableGcGuard::new(&ops);
        let repo_id = RepositoryId(Uuid::new_v4());
        let version = ops.create_version(&repo_id, None).unwrap();

        let candidates = vec![
            ("hash1".to_string(), 0, Some(version.version_id.clone())),
            ("hash2".to_string(), 1, Some(version.version_id.clone())),
            ("hash3".to_string(), 0, None),
        ];

        let allowed = guard.filter_gc_candidates(&candidates).unwrap();
        assert!(allowed.contains(&"hash1".to_string()));
        assert!(!allowed.contains(&"hash2".to_string()));
        assert!(allowed.contains(&"hash3".to_string()));
    }

    #[test]
    fn priority_order_correct() {
        let order = ImmutableGcGuard::priority_order();
        assert_eq!(order[0], "immutable_retention");
        assert_eq!(order[3], "admin_delete");
    }

    #[test]
    fn check_nonexistent_version_fails() {
        let ops = make_ops();
        let guard = ImmutableGcGuard::new(&ops);
        let fake_id = VersionId(Uuid::new_v4());
        let result = guard.check_version(&fake_id);
        assert!(result.is_err());
    }
}