//! HyperBackup X Agent crate
//!
//! 提供 Windows Service 生命周期、崩溃自动重启、内存预算控制。

pub mod service;
pub mod recovery;
pub mod memory_budget;
pub mod resource_collector;

pub use service::{ServiceConfig, ServiceError, ServiceState};
pub use recovery::{
    RecoveryAction, RecoveryActionType, RecoveryConfig, RecoveryError,
    ThreadGuard, ScopedThread, JoinThread,
};
pub use memory_budget::{
    MemoryBudget, MemorySnapshot, MemoryBudgetEnforcer, BudgetAction, CacheBudget,
};
pub use resource_collector::{ResourceCollector, ResourceMetrics, LowMemoryMode, ResourceTargetReport};
