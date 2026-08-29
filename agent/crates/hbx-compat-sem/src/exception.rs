use serde::{Deserialize, Serialize};

use super::SemanticError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExceptionType {
    NetworkBreak,
    DiskFull,
    PermissionDenied,
    FileLocked,
    SourceMissing,
    RepoUnavailable,
    ProcessKilled,
    PowerOff,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExceptionDecision {
    Retry,
    Skip,
    Abort,
    Continue,
    MarkFailed,
    Resume,
    SelfCheckAndIsolate,
}

pub fn align_exception_decision(
    exception: ExceptionType,
) -> Result<ExceptionDecision, SemanticError> {
    let decision = match exception {
        ExceptionType::NetworkBreak => ExceptionDecision::Retry,
        ExceptionType::DiskFull => ExceptionDecision::Abort,
        ExceptionType::PermissionDenied => ExceptionDecision::Skip,
        ExceptionType::FileLocked => ExceptionDecision::Retry,
        ExceptionType::SourceMissing => ExceptionDecision::Skip,
        ExceptionType::RepoUnavailable => ExceptionDecision::Retry,
        ExceptionType::ProcessKilled => ExceptionDecision::Resume,
        ExceptionType::PowerOff => ExceptionDecision::SelfCheckAndIsolate,
    };
    Ok(decision)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_network_break_retries() {
        let decision = align_exception_decision(ExceptionType::NetworkBreak).unwrap();
        assert_eq!(decision, ExceptionDecision::Retry);
    }

    #[test]
    fn test_disk_full_aborts() {
        let decision = align_exception_decision(ExceptionType::DiskFull).unwrap();
        assert_eq!(decision, ExceptionDecision::Abort);
    }

    #[test]
    fn test_permission_denied_skips() {
        let decision = align_exception_decision(ExceptionType::PermissionDenied).unwrap();
        assert_eq!(decision, ExceptionDecision::Skip);
    }

    #[test]
    fn test_file_locked_retries() {
        let decision = align_exception_decision(ExceptionType::FileLocked).unwrap();
        assert_eq!(decision, ExceptionDecision::Retry);
    }

    #[test]
    fn test_source_missing_skips() {
        let decision = align_exception_decision(ExceptionType::SourceMissing).unwrap();
        assert_eq!(decision, ExceptionDecision::Skip);
    }

    #[test]
    fn test_repo_unavailable_retries() {
        let decision = align_exception_decision(ExceptionType::RepoUnavailable).unwrap();
        assert_eq!(decision, ExceptionDecision::Retry);
    }

    #[test]
    fn test_process_killed_resumes() {
        let decision = align_exception_decision(ExceptionType::ProcessKilled).unwrap();
        assert_eq!(decision, ExceptionDecision::Resume);
    }

    #[test]
    fn test_power_off_self_check() {
        let decision = align_exception_decision(ExceptionType::PowerOff).unwrap();
        assert_eq!(decision, ExceptionDecision::SelfCheckAndIsolate);
    }

    #[test]
    fn test_all_exceptions_covered() {
        let exceptions = [
            ExceptionType::NetworkBreak,
            ExceptionType::DiskFull,
            ExceptionType::PermissionDenied,
            ExceptionType::FileLocked,
            ExceptionType::SourceMissing,
            ExceptionType::RepoUnavailable,
            ExceptionType::ProcessKilled,
            ExceptionType::PowerOff,
        ];
        for e in &exceptions {
            let result = align_exception_decision(*e);
            assert!(result.is_ok(), "exception {:?} should have a decision", e);
        }
    }
}