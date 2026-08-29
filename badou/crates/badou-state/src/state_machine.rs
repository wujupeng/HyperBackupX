use serde::{Deserialize, Serialize};
use thiserror::Error;
use std::str::FromStr;

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
    pub fn as_str(&self) -> &'static str {
        use VersionStatus::*;
        match self {
            Created => "created",
            Writing => "writing",
            Verifying => "verifying",
            Committing => "committing",
            Sealed => "sealed",
            Expired => "expired",
            Deleted => "deleted",
            GcPending => "gc_pending",
            Purged => "purged",
        }
    }
}

impl FromStr for VersionStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        use VersionStatus::*;
        match s {
            "created" => Ok(Created),
            "writing" => Ok(Writing),
            "verifying" => Ok(Verifying),
            "committing" => Ok(Committing),
            "sealed" => Ok(Sealed),
            "expired" => Ok(Expired),
            "deleted" => Ok(Deleted),
            "gc_pending" => Ok(GcPending),
            "purged" => Ok(Purged),
            _ => Err(format!("unknown version status: {}", s)),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SnapshotStatus {
    Created,
    Writing,
    Sealed,
    Corrupt,
    Deleted,
}

impl SnapshotStatus {
    pub fn as_str(&self) -> &'static str {
        use SnapshotStatus::*;
        match self {
            Created => "created",
            Writing => "writing",
            Sealed => "sealed",
            Corrupt => "corrupt",
            Deleted => "deleted",
        }
    }
}

#[derive(Debug, Error)]
pub enum StateTransitionError {
    #[error("invalid state transition: {from:?} -> {to:?}")]
    InvalidTransition { from: VersionStatus, to: VersionStatus },
    #[error("version not sealed: current status is {0:?}")]
    NotSealed(VersionStatus),
    #[error("cannot skip verifying: {from:?} -> sealed is forbidden")]
    SkipVerifying { from: VersionStatus },
}

pub struct StateMachine;

impl StateMachine {
    pub fn can_transition(from: VersionStatus, to: VersionStatus) -> bool {
        use VersionStatus::*;
        matches!(
            (from, to),
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

    pub fn transition(
        from: VersionStatus,
        to: VersionStatus,
    ) -> Result<(), StateTransitionError> {
        if !Self::can_transition(from, to) {
            if to == VersionStatus::Sealed && from != VersionStatus::Committing {
                return Err(StateTransitionError::SkipVerifying { from });
            }
            return Err(StateTransitionError::InvalidTransition { from, to });
        }
        Ok(())
    }

    pub fn assert_sealed(status: VersionStatus) -> Result<(), StateTransitionError> {
        if status != VersionStatus::Sealed {
            return Err(StateTransitionError::NotSealed(status));
        }
        Ok(())
    }

    pub fn is_terminal(status: VersionStatus) -> bool {
        status == VersionStatus::Purged
    }

    pub fn is_visible(status: VersionStatus) -> bool {
        use VersionStatus::*;
        !matches!(status, Created | Writing | Verifying | Committing | Purged)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_full_lifecycle() {
        use VersionStatus::*;
        let steps = [
            (Created, Writing),
            (Writing, Verifying),
            (Verifying, Committing),
            (Committing, Sealed),
            (Sealed, Expired),
            (Expired, GcPending),
            (GcPending, Purged),
        ];
        for (from, to) in steps {
            assert!(StateMachine::can_transition(from, to),
                "transition {:?} -> {:?} should be valid", from, to);
        }
    }

    #[test]
    fn invalid_skip_to_sealed() {
        let result = StateMachine::transition(VersionStatus::Created, VersionStatus::Sealed);
        assert!(result.is_err());
    }

    #[test]
    fn invalid_skip_verifying() {
        let result = StateMachine::transition(VersionStatus::Writing, VersionStatus::Sealed);
        assert!(result.is_err());
        match result {
            Err(StateTransitionError::SkipVerifying { .. }) => {}
            _ => panic!("expected SkipVerifying error"),
        }
    }

    #[test]
    fn assert_sealed_ok() {
        assert!(StateMachine::assert_sealed(VersionStatus::Sealed).is_ok());
    }

    #[test]
    fn assert_sealed_fail() {
        assert!(StateMachine::assert_sealed(VersionStatus::Writing).is_err());
    }

    #[test]
    fn purged_is_terminal() {
        assert!(StateMachine::is_terminal(VersionStatus::Purged));
        assert!(!StateMachine::is_terminal(VersionStatus::Sealed));
    }

    #[test]
    fn sealed_is_visible() {
        assert!(StateMachine::is_visible(VersionStatus::Sealed));
        assert!(!StateMachine::is_visible(VersionStatus::Writing));
        assert!(!StateMachine::is_visible(VersionStatus::Purged));
    }

    #[test]
    fn deleted_is_visible() {
        assert!(StateMachine::is_visible(VersionStatus::Deleted));
        assert!(StateMachine::is_visible(VersionStatus::Expired));
    }

    #[test]
    fn rollback_writing_to_created() {
        assert!(StateMachine::can_transition(VersionStatus::Writing, VersionStatus::Created));
    }
}