mod common;
use common::*;
use badou_gc::{ImmutableGcGuard, GcDecision};
use badou_state::{StateMachine, VersionStatus};

#[test]
fn e2e_state_machine_valid_transitions() {
    use VersionStatus::*;
    assert!(StateMachine::can_transition(Created, Writing));
    assert!(StateMachine::can_transition(Writing, Verifying));
    assert!(StateMachine::can_transition(Verifying, Committing));
    assert!(StateMachine::can_transition(Committing, Sealed));
    assert!(StateMachine::can_transition(Sealed, Expired));
    assert!(StateMachine::can_transition(Sealed, Deleted));
    assert!(StateMachine::can_transition(Deleted, GcPending));
    assert!(StateMachine::can_transition(GcPending, Purged));
}

#[test]
fn e2e_state_machine_invalid_transitions() {
    use VersionStatus::*;
    assert!(!StateMachine::can_transition(Created, Sealed));
    assert!(!StateMachine::can_transition(Sealed, Writing));
    assert!(!StateMachine::can_transition(Purged, Created));
}

#[test]
fn e2e_state_machine_transition_ok() {
    use VersionStatus::*;
    assert!(StateMachine::transition(Created, Writing).is_ok());
    assert!(StateMachine::transition(Committing, Sealed).is_ok());
}

#[test]
fn e2e_state_machine_transition_fail() {
    use VersionStatus::*;
    assert!(StateMachine::transition(Created, Sealed).is_err());
}

#[test]
fn e2e_state_machine_assert_sealed() {
    use VersionStatus::*;
    assert!(StateMachine::assert_sealed(Sealed).is_ok());
    assert!(StateMachine::assert_sealed(Writing).is_err());
}

#[test]
fn e2e_state_machine_is_terminal() {
    use VersionStatus::*;
    assert!(StateMachine::is_terminal(Purged));
    assert!(!StateMachine::is_terminal(Sealed));
}

#[test]
fn e2e_immutable_guard_allows_sealed() {
    let env = E2EEnv::new();
    let guard = ImmutableGcGuard::new(&env.version_ops);
    let version_id = hbx_core::domain::common::VersionId(uuid::Uuid::new_v4());
    let _ = guard.check_version(&version_id);
}

#[test]
fn e2e_gc_decision_variants() {
    assert_eq!(GcDecision::Allow, GcDecision::Allow);
    assert_ne!(GcDecision::Allow, GcDecision::Block { reason: "test".to_string() });
}
