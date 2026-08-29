use crate::entry::{BadouJournalEntry, JournalOpType};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct UncompletedOp {
    pub job_id: Uuid,
    pub op_type: JournalOpType,
    pub payload: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecoveryAction {
    CleanStaging,
    RerunGc,
    MarkVerifyIncomplete,
    ContinueStateTransition,
    Skip,
}

pub fn scan_uncompleted(entries: &[BadouJournalEntry]) -> Vec<UncompletedOp> {
    entries
        .iter()
        .filter(|e| !e.committed)
        .map(|e| UncompletedOp {
            job_id: e.job_id,
            op_type: e.op_type,
            payload: e.payload.clone(),
        })
        .collect()
}

pub fn classify_recovery(op: &UncompletedOp) -> RecoveryAction {
    match op.op_type {
        JournalOpType::CommitStep => RecoveryAction::CleanStaging,
        JournalOpType::GcStep => RecoveryAction::RerunGc,
        JournalOpType::VerifyStep => RecoveryAction::MarkVerifyIncomplete,
        JournalOpType::StateTransition => RecoveryAction::ContinueStateTransition,
        JournalOpType::Recovery => RecoveryAction::Skip,
    }
}

pub fn plan_recovery(entries: &[BadouJournalEntry]) -> Vec<(UncompletedOp, RecoveryAction)> {
    let uncompleted = scan_uncompleted(entries);
    uncompleted
        .iter()
        .map(|op| {
            let action = classify_recovery(op);
            (op.clone(), action)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entry::BadouJournalEntry;
    use chrono::Utc;

    fn make_entry(op_type: JournalOpType, committed: bool) -> BadouJournalEntry {
        BadouJournalEntry {
            op_type,
            timestamp: Utc::now(),
            job_id: Uuid::new_v4(),
            payload: vec![1, 2, 3],
            committed,
        }
    }

    #[test]
    fn scan_finds_uncommitted() {
        let entries = vec![
            make_entry(JournalOpType::CommitStep, true),
            make_entry(JournalOpType::GcStep, false),
            make_entry(JournalOpType::VerifyStep, true),
            make_entry(JournalOpType::StateTransition, false),
        ];
        let uncompleted = scan_uncompleted(&entries);
        assert_eq!(uncompleted.len(), 2);
        assert_eq!(uncompleted[0].op_type, JournalOpType::GcStep);
        assert_eq!(uncompleted[1].op_type, JournalOpType::StateTransition);
    }

    #[test]
    fn classify_actions() {
        assert_eq!(classify_recovery(&uncompleted_op(JournalOpType::CommitStep)), RecoveryAction::CleanStaging);
        assert_eq!(classify_recovery(&uncompleted_op(JournalOpType::GcStep)), RecoveryAction::RerunGc);
        assert_eq!(classify_recovery(&uncompleted_op(JournalOpType::VerifyStep)), RecoveryAction::MarkVerifyIncomplete);
        assert_eq!(classify_recovery(&uncompleted_op(JournalOpType::StateTransition)), RecoveryAction::ContinueStateTransition);
        assert_eq!(classify_recovery(&uncompleted_op(JournalOpType::Recovery)), RecoveryAction::Skip);
    }

    fn uncompleted_op(op_type: JournalOpType) -> UncompletedOp {
        UncompletedOp {
            job_id: Uuid::new_v4(),
            op_type,
            payload: vec![],
        }
    }

    #[test]
    fn plan_recovery_empty_when_all_committed() {
        let entries = vec![
            make_entry(JournalOpType::CommitStep, true),
            make_entry(JournalOpType::GcStep, true),
        ];
        let plan = plan_recovery(&entries);
        assert!(plan.is_empty());
    }

    #[test]
    fn plan_recovery_mixed() {
        let entries = vec![
            make_entry(JournalOpType::CommitStep, false),
            make_entry(JournalOpType::GcStep, true),
            make_entry(JournalOpType::VerifyStep, false),
        ];
        let plan = plan_recovery(&entries);
        assert_eq!(plan.len(), 2);
        assert_eq!(plan[0].1, RecoveryAction::CleanStaging);
        assert_eq!(plan[1].1, RecoveryAction::MarkVerifyIncomplete);
    }
}