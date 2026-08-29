//! 三级完整性校验：Repository/Version/Chunk。

pub mod checkers;
pub mod scheduler;

pub use checkers::{Verifier, VerifyReport, VerifyStatus, VerifyTarget, VerifyMode, VerifyError};
pub use scheduler::{VerifyScheduler, VerifySchedule, VerifyRecord, VerifySchedulerError};
