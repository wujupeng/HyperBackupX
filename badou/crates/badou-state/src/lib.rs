//! 状态机：PostgreSQL 元数据与 ACID 状态转换。

pub mod schema;
pub mod state_machine;
pub mod immutable;

pub use state_machine::{StateMachine, StateTransitionError, VersionStatus, SnapshotStatus};
pub use immutable::{ImmutableGuard, ImmutableConflict};
