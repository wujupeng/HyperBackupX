pub mod adapter;
pub mod dual_repo_checker;
pub mod engine;
pub mod restore;
pub mod state_machine;

pub use adapter::CompatibilityRepoAdapter;
pub use dual_repo_checker::{
    ConsistencyConclusion, DualRepoConsistencyChecker, DualRepoError, DualRepoInconsistentEvent,
    DualRepoMode, FileComparison, IDualRepositoryConsistencyChecker,
};
pub use engine::{
    CompatibilityBackupEngine, CompatibilityBackupError, CompatibilityJob,
    CompatibilityRestoreError, CompatibilityVersion, ICompatibilityBackupEngine,
};
pub use restore::{
    CompatibilityRestoreEngine, CompatibilityRestoreJob, CompatibilityRestoreResult,
};
pub use state_machine::{
    CompatCheckpoint, CompatExecutionState, CompatExecutionTracker, CompatibilityExecution,
    ExceptionAction, decide_exception_action, retry_backoff,
};
