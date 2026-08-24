use chrono::{DateTime, Utc};

use hbx_core::domain::common::{VersionId, VersionSummary};
use hbx_core::domain::restore::RestorePoint;
use hbx_core::pipeline::traits::{IBackupRepository, RepoError};

pub struct RestorePointResolver<'a> {
    repo: &'a dyn IBackupRepository,
}

impl<'a> RestorePointResolver<'a> {
    pub fn new(repo: &'a dyn IBackupRepository) -> Self {
        Self { repo }
    }

    pub fn resolve_by_version_id(
        &self,
        version_id: &VersionId,
    ) -> Result<RestorePoint, RepoError> {
        let versions = self.repo.list_versions()?;
        let v = versions
            .iter()
            .find(|v| v.version_id == version_id.0)
            .ok_or_else(|| RepoError::Failed(format!("version {:?} not found", version_id)))?;

        Ok(RestorePoint {
            version_id: version_id.clone(),
            timestamp: v.timestamp,
            version_number: v.version_number,
        })
    }

    pub fn resolve_by_timestamp(
        &self,
        point_in_time: DateTime<Utc>,
    ) -> Result<Option<RestorePoint>, RepoError> {
        let versions = self.repo.list_versions()?;

        let candidate = versions
            .iter()
            .filter(|v| v.timestamp <= point_in_time)
            .max_by_key(|v| v.timestamp);

        Ok(candidate.map(|v| RestorePoint {
            version_id: VersionId(v.version_id),
            timestamp: v.timestamp,
            version_number: v.version_number,
        }))
    }

    pub fn resolve_latest(&self) -> Result<Option<RestorePoint>, RepoError> {
        let versions = self.repo.list_versions()?;

        Ok(versions
            .first()
            .map(|v| RestorePoint {
                version_id: VersionId(v.version_id),
                timestamp: v.timestamp,
                version_number: v.version_number,
            }))
    }

    pub fn list_restore_points(&self) -> Result<Vec<RestorePoint>, RepoError> {
        let versions = self.repo.list_versions()?;

        Ok(versions
            .iter()
            .map(|v| RestorePoint {
                version_id: VersionId(v.version_id),
                timestamp: v.timestamp,
                version_number: v.version_number,
            })
            .collect())
    }
}

pub fn filter_versions_before(
    versions: &[VersionSummary],
    point_in_time: DateTime<Utc>,
) -> Vec<&VersionSummary> {
    versions
        .iter()
        .filter(|v| v.timestamp <= point_in_time)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use hbx_core::domain::backup::BackupType;
    use hbx_core::domain::chunk::ChunkLocation;
    use hbx_core::domain::common::{RepoLock, VersionSummary};
    use hbx_core::domain::encryption::EncryptedChunk as EncChunk;
    use hbx_core::domain::repository::Manifest;
    use hbx_core::pipeline::traits::RepoError;
    use std::time::Duration;
    use uuid::Uuid;

    fn make_version_summary(days_ago: i64, number: u64) -> VersionSummary {
        VersionSummary {
            version_id: Uuid::new_v4(),
            version_number: number,
            timestamp: Utc::now() - chrono::Duration::days(days_ago),
            backup_type: BackupType::Full,
            total_size: 1000,
            stored_size: 500,
        }
    }

    struct MockRepo {
        versions: Vec<VersionSummary>,
    }

    impl IBackupRepository for MockRepo {
        fn write_chunk(
            &self,
            _hash: &hbx_core::domain::chunk::ChunkHash,
            _encrypted: &EncChunk,
        ) -> Result<ChunkLocation, RepoError> {
            Err(RepoError::Failed("not implemented".into()))
        }

        fn read_chunk(&self, _location: &ChunkLocation) -> Result<EncChunk, RepoError> {
            Err(RepoError::Failed("not implemented".into()))
        }

        fn chunk_exists(
            &self,
            _hash: &hbx_core::domain::chunk::ChunkHash,
        ) -> Result<bool, RepoError> {
            Ok(false)
        }

        fn delete_chunk(&self, _location: &ChunkLocation) -> Result<(), RepoError> {
            Ok(())
        }

        fn write_manifest(
            &self,
            _version_id: &VersionId,
            _manifest: &Manifest,
        ) -> Result<(), RepoError> {
            Ok(())
        }

        fn read_manifest(&self, _version_id: &VersionId) -> Result<Manifest, RepoError> {
            Err(RepoError::Failed("not found".into()))
        }

        fn list_versions(&self) -> Result<Vec<VersionSummary>, RepoError> {
            let mut v = self.versions.clone();
            v.sort_by_key(|vs| std::cmp::Reverse(vs.timestamp));
            Ok(v)
        }

        fn acquire_lock(
            &self,
            _operation: hbx_core::domain::common::LockOperation,
            _timeout: Duration,
        ) -> Result<RepoLock, RepoError> {
            Ok(RepoLock {
                lock_id: Uuid::new_v4(),
                holder: "test".into(),
                acquired_at: Utc::now(),
                ttl: Duration::from_secs(300),
            })
        }
    }

    #[test]
    fn test_resolve_by_version_id_found() {
        let v = make_version_summary(1, 1);
        let repo = MockRepo {
            versions: vec![v.clone()],
        };
        let resolver = RestorePointResolver::new(&repo);
        let result = resolver
            .resolve_by_version_id(&VersionId(v.version_id))
            .unwrap();
        assert_eq!(result.version_id.0, v.version_id);
        assert_eq!(result.version_number, v.version_number);
    }

    #[test]
    fn test_resolve_by_version_id_not_found() {
        let repo = MockRepo {
            versions: vec![make_version_summary(1, 1)],
        };
        let resolver = RestorePointResolver::new(&repo);
        let result = resolver.resolve_by_version_id(&VersionId(Uuid::new_v4()));
        assert!(result.is_err());
    }

    #[test]
    fn test_resolve_by_timestamp_finds_closest() {
        let v1 = make_version_summary(10, 1);
        let v2 = make_version_summary(5, 2);
        let v3 = make_version_summary(1, 3);

        let repo = MockRepo {
            versions: vec![v1.clone(), v2.clone(), v3.clone()],
        };
        let resolver = RestorePointResolver::new(&repo);

        let point = v2.timestamp + chrono::Duration::seconds(1);
        let result = resolver.resolve_by_timestamp(point).unwrap();
        assert!(result.is_some());
        assert_eq!(result.unwrap().version_id.0, v2.version_id);
    }

    #[test]
    fn test_resolve_by_timestamp_none_when_all_after() {
        let v = make_version_summary(0, 1);
        let repo = MockRepo {
            versions: vec![v],
        };
        let resolver = RestorePointResolver::new(&repo);

        let point = Utc::now() - chrono::Duration::days(10);
        let result = resolver.resolve_by_timestamp(point).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_resolve_latest() {
        let v1 = make_version_summary(5, 1);
        let v2 = make_version_summary(1, 2);

        let repo = MockRepo {
            versions: vec![v1, v2.clone()],
        };
        let resolver = RestorePointResolver::new(&repo);

        let result = resolver.resolve_latest().unwrap();
        assert!(result.is_some());
        assert_eq!(result.unwrap().version_id.0, v2.version_id);
    }

    #[test]
    fn test_resolve_latest_empty() {
        let repo = MockRepo {
            versions: vec![],
        };
        let resolver = RestorePointResolver::new(&repo);
        let result = resolver.resolve_latest().unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_list_restore_points() {
        let repo = MockRepo {
            versions: vec![
                make_version_summary(3, 1),
                make_version_summary(2, 2),
                make_version_summary(1, 3),
            ],
        };
        let resolver = RestorePointResolver::new(&repo);
        let points = resolver.list_restore_points().unwrap();
        assert_eq!(points.len(), 3);
    }

    #[test]
    fn test_filter_versions_before() {
        let v1 = make_version_summary(10, 1);
        let v2 = make_version_summary(5, 2);
        let v3 = make_version_summary(1, 3);
        let versions = vec![v1.clone(), v2.clone(), v3.clone()];

        let cutoff = v2.timestamp + chrono::Duration::seconds(1);
        let filtered = filter_versions_before(&versions, cutoff);
        assert_eq!(filtered.len(), 2);
    }
}