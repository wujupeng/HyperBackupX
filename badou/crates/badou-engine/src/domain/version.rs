use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use hbx_core::domain::common::{RepositoryId, VersionId};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum VersionStatus {
    Created,
    Writing,
    Verifying,
    Committing,
    Sealed,
    Expired,
    Deleted,
    GcPending,
    Purged,
}

impl VersionStatus {
    pub fn can_transition_to(&self, target: VersionStatus) -> bool {
        use VersionStatus::*;
        matches!(
            (self, target),
            (Created, Writing)
                | (Writing, Verifying)
                | (Writing, Created)
                | (Verifying, Committing)
                | (Verifying, Writing)
                | (Committing, Sealed)
                | (Sealed, Expired)
                | (Sealed, Deleted)
                | (Expired, Deleted)
                | (Expired, GcPending)
                | (Deleted, GcPending)
                | (GcPending, Purged)
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Version {
    pub version_id: VersionId,
    pub repo_id: RepositoryId,
    pub snapshot_id: Uuid,
    pub parent_version_id: Option<VersionId>,
    pub sequence: u64,
    pub status: VersionStatus,
    pub created_at: DateTime<Utc>,
    pub sealed_at: Option<DateTime<Utc>>,
    pub immutable_until: Option<DateTime<Utc>>,
}

impl Version {
    pub fn new(
        version_id: VersionId,
        repo_id: RepositoryId,
        snapshot_id: Uuid,
        parent_version_id: Option<VersionId>,
        sequence: u64,
    ) -> Self {
        Self {
            version_id,
            repo_id,
            snapshot_id,
            parent_version_id,
            sequence,
            status: VersionStatus::Created,
            created_at: Utc::now(),
            sealed_at: None,
            immutable_until: None,
        }
    }

    pub fn transition(&mut self, target: VersionStatus) -> Result<(), String> {
        if !self.status.can_transition_to(target) {
            return Err(format!(
                "invalid state transition: {:?} -> {:?}",
                self.status, target
            ));
        }
        if target == VersionStatus::Sealed {
            self.sealed_at = Some(Utc::now());
        }
        self.status = target;
        Ok(())
    }

    pub fn is_sealed(&self) -> bool {
        self.status == VersionStatus::Sealed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_state_machine_valid_transitions() {
        let mut v = Version::new(
            VersionId(Uuid::new_v4()),
            RepositoryId(Uuid::new_v4()),
            Uuid::new_v4(),
            None,
            1,
        );
        assert!(v.transition(VersionStatus::Writing).is_ok());
        assert!(v.transition(VersionStatus::Verifying).is_ok());
        assert!(v.transition(VersionStatus::Committing).is_ok());
        assert!(v.transition(VersionStatus::Sealed).is_ok());
        assert!(v.is_sealed());
        assert!(v.sealed_at.is_some());
    }

    #[test]
    fn version_state_machine_invalid_transition() {
        let mut v = Version::new(
            VersionId(Uuid::new_v4()),
            RepositoryId(Uuid::new_v4()),
            Uuid::new_v4(),
            None,
            1,
        );
        assert!(v.transition(VersionStatus::Sealed).is_err());
    }

    #[test]
    fn version_sequence_starts_at_1() {
        let v = Version::new(
            VersionId(Uuid::new_v4()),
            RepositoryId(Uuid::new_v4()),
            Uuid::new_v4(),
            None,
            1,
        );
        assert_eq!(v.sequence, 1);
    }
}