//! 健康检查 + Prometheus 指标。
//!
//! 模块：
//! - `health`: 健康检查接口（节点/磁盘/Journal/Commit/GC 状态）
//! - `metrics`: Prometheus 指标暴露（手动实现文本格式）
//!
//! 映射 spec.md §4.4、design.md §2.1.2.1 badou-health。

pub mod health;
pub mod metrics;

pub use health::{
    HealthChecker, HealthReport, HealthLevel, HealthIssue,
    DiskStatus, JournalStatus, RecentCommitStatus, GcStatus,
};
pub use metrics::{
    MetricsRegistry, Metric, MetricType, BadouMetrics,
};
