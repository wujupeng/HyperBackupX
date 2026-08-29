//! 引用计数 GC 与不可变冲突仲裁。

pub mod delete;
pub mod executor;
pub mod scheduler;
pub mod immutable_guard;

pub use delete::{VersionDeleter, DeleteResult, DeleteError};
pub use executor::{GcExecutor, GcReport, GcExecutorError};
pub use scheduler::{GcScheduler, GcScheduleConfig, GcTriggerType, GcTriggerRecord, SchedulerError};
pub use immutable_guard::{ImmutableGcGuard, GcDecision, ImmutableGcError};
