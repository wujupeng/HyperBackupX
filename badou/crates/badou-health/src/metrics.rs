//! Prometheus 指标暴露（手动实现文本格式，不依赖 prometheus crate）。
//!
//! 映射 spec.md §4.4 规则 2、design.md §2.1.2.1 badou-health。

use std::collections::HashMap;
use std::sync::Arc;
use parking_lot::RwLock;


/// 指标类型。
#[derive(Debug, Clone, Copy)]
pub enum MetricType {
    Counter,
    Gauge,
    Histogram,
}

/// 单个指标。
#[derive(Debug, Clone)]
pub struct Metric {
    pub name: String,
    pub help: String,
    pub metric_type: MetricType,
    pub value: f64,
    pub labels: HashMap<String, String>,
}

impl Metric {
    pub fn counter(name: &str, help: &str, value: f64) -> Self {
        Self {
            name: name.to_string(),
            help: help.to_string(),
            metric_type: MetricType::Counter,
            value,
            labels: HashMap::new(),
        }
    }

    pub fn gauge(name: &str, help: &str, value: f64) -> Self {
        Self {
            name: name.to_string(),
            help: help.to_string(),
            metric_type: MetricType::Gauge,
            value,
            labels: HashMap::new(),
        }
    }

    pub fn with_label(mut self, key: &str, value: &str) -> Self {
        self.labels.insert(key.to_string(), value.to_string());
        self
    }
}

/// 指标注册表。
pub struct MetricsRegistry {
    metrics: RwLock<Vec<Metric>>,
}

impl MetricsRegistry {
    pub fn new() -> Self {
        Self {
            metrics: RwLock::new(Vec::new()),
        }
    }

    /// 注册或更新指标。
    pub fn set(&self, metric: Metric) {
        let mut metrics = self.metrics.write();
        if let Some(existing) = metrics.iter_mut().find(|m| m.name == metric.name && m.labels == metric.labels) {
            existing.value = metric.value;
        } else {
            metrics.push(metric);
        }
    }

    /// 递增计数器。
    pub fn increment(&self, name: &str, help: &str, delta: f64) {
        let mut metrics = self.metrics.write();
        if let Some(existing) = metrics.iter_mut().find(|m| m.name == name) {
            existing.value += delta;
        } else {
            metrics.push(Metric::counter(name, help, delta));
        }
    }

    /// 渲染为 Prometheus 文本格式。
    pub fn render(&self) -> String {
        let metrics = self.metrics.read();
        let mut output = String::new();
        let mut seen_names = std::collections::HashSet::new();

        for metric in metrics.iter() {
            if seen_names.insert(metric.name.clone()) {
                output.push_str(&format!("# HELP {} {}\n", metric.name, metric.help));
                let type_str = match metric.metric_type {
                    MetricType::Counter => "counter",
                    MetricType::Gauge => "gauge",
                    MetricType::Histogram => "histogram",
                };
                output.push_str(&format!("# TYPE {} {}\n", metric.name, type_str));
            }

            if metric.labels.is_empty() {
                output.push_str(&format!("{} {}\n", metric.name, metric.value));
            } else {
                let labels: Vec<String> = metric.labels.iter()
                    .map(|(k, v)| format!("{}=\"{}\"", k, v))
                    .collect();
                output.push_str(&format!("{}{{{}}} {}\n", metric.name, labels.join(","), metric.value));
            }
        }

        output
    }

    /// 获取所有指标。
    pub fn all(&self) -> Vec<Metric> {
        self.metrics.read().clone()
    }
}

impl Default for MetricsRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// 八斗指标收集器：从健康报告收集指标。
pub struct BadouMetrics {
    registry: Arc<MetricsRegistry>,
}

impl BadouMetrics {
    pub fn new() -> Self {
        Self {
            registry: Arc::new(MetricsRegistry::new()),
        }
    }

    pub fn registry(&self) -> &Arc<MetricsRegistry> {
        &self.registry
    }

    /// 从健康报告更新指标。
    pub fn update_from_health(&self, report: &crate::health::HealthReport) {
        for disk in &report.disks {
            self.registry.set(
                Metric::gauge("badou_disk_usage_percent", "Disk usage percentage", disk.usage_percent)
                    .with_label("path", &disk.path)
            );
            self.registry.set(
                Metric::gauge("badou_disk_available_bytes", "Available disk bytes", disk.available_bytes as f64)
                    .with_label("path", &disk.path)
            );
        }

        self.registry.set(
            Metric::gauge("badou_journal_entries", "Total journal entries", report.journal.entries as f64)
        );
        self.registry.set(
            Metric::gauge("badou_journal_corrupted", "Journal corrupted (0/1)", if report.journal.corrupted { 1.0 } else { 0.0 })
        );

        self.registry.set(
            Metric::counter("badou_commit_total", "Total commits", report.recent_commit.total_commits as f64)
        );
        self.registry.set(
            Metric::counter("badou_commit_failed_total", "Failed commits", report.recent_commit.failed_commits as f64)
        );

        self.registry.set(
            Metric::gauge("badou_gc_running", "GC running (0/1)", if report.gc.gc_running { 1.0 } else { 0.0 })
        );
        self.registry.set(
            Metric::counter("badou_gc_collected_total", "Total chunks collected by GC", report.gc.last_gc_collected as f64)
        );
        self.registry.set(
            Metric::counter("badou_gc_freed_bytes_total", "Total bytes freed by GC", report.gc.last_gc_freed_bytes as f64)
        );

        let health_value = match report.level {
            crate::health::HealthLevel::Healthy => 1.0,
            crate::health::HealthLevel::Warning => 0.5,
            crate::health::HealthLevel::Critical => 0.0,
        };
        self.registry.set(
            Metric::gauge("badou_node_health", "Node health (1=healthy, 0.5=warning, 0=critical)", health_value)
        );
    }

    /// 记录 Commit 吞吐。
    pub fn record_commit_throughput(&self, bytes: u64, duration_ms: u64) {
        let throughput = if duration_ms > 0 {
            (bytes as f64) / (duration_ms as f64 / 1000.0)
        } else {
            0.0
        };
        self.registry.set(
            Metric::gauge("badou_commit_throughput", "Commit throughput (bytes/sec)", throughput)
        );
    }

    /// 记录恢复吞吐。
    pub fn record_recovery_throughput(&self, bytes: u64, duration_ms: u64) {
        let throughput = if duration_ms > 0 {
            (bytes as f64) / (duration_ms as f64 / 1000.0)
        } else {
            0.0
        };
        self.registry.set(
            Metric::gauge("badou_recovery_throughput", "Recovery throughput (bytes/sec)", throughput)
        );
    }

    /// 设置 Chunk 总数。
    pub fn set_chunk_total(&self, total: u64) {
        self.registry.set(
            Metric::gauge("badou_chunk_total", "Total chunks", total as f64)
        );
    }

    /// 设置 Version 总数。
    pub fn set_version_total(&self, total: u64) {
        self.registry.set(
            Metric::gauge("badou_version_total", "Total versions", total as f64)
        );
    }

    /// 设置校验结果。
    pub fn set_verify_result(&self, passed: bool, total_checked: u64, total_failed: u64) {
        self.registry.set(
            Metric::gauge("badou_verify_passed", "Last verify passed (0/1)", if passed { 1.0 } else { 0.0 })
        );
        self.registry.set(
            Metric::counter("badou_verify_checked_total", "Total items checked", total_checked as f64)
        );
        self.registry.set(
            Metric::counter("badou_verify_failed_total", "Total items failed", total_failed as f64)
        );
    }

    /// 渲染 Prometheus 格式。
    pub fn render(&self) -> String {
        self.registry.render()
    }
}

impl Default for BadouMetrics {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::health::{HealthChecker, HealthLevel};

    #[test]
    fn render_empty_registry() {
        let registry = MetricsRegistry::new();
        let output = registry.render();
        assert!(output.is_empty());
    }

    #[test]
    fn render_with_metrics() {
        let registry = MetricsRegistry::new();
        registry.set(Metric::counter("test_counter", "A test counter", 42.0));
        registry.set(Metric::gauge("test_gauge", "A test gauge", 2.71));

        let output = registry.render();
        assert!(output.contains("# HELP test_counter A test counter"));
        assert!(output.contains("# TYPE test_counter counter"));
        assert!(output.contains("test_counter 42"));
        assert!(output.contains("# HELP test_gauge A test gauge"));
        assert!(output.contains("# TYPE test_gauge gauge"));
        assert!(output.contains("test_gauge 2.71"));
    }

    #[test]
    fn render_with_labels() {
        let registry = MetricsRegistry::new();
        registry.set(
            Metric::gauge("disk_usage", "Disk usage", 75.0)
                .with_label("path", "/data")
        );

        let output = registry.render();
        assert!(output.contains("disk_usage{path=\"/data\"} 75"));
    }

    #[test]
    fn increment_counter() {
        let registry = MetricsRegistry::new();
        registry.increment("commits", "Total commits", 1.0);
        registry.increment("commits", "Total commits", 1.0);
        registry.increment("commits", "Total commits", 1.0);

        let output = registry.render();
        assert!(output.contains("commits 3"));
    }

    #[test]
    fn update_from_health_report() {
        let checker = HealthChecker::new("node-1");
        checker.record_commit(true, Some("v1".to_string()));
        checker.record_commit(false, None);
        let report = checker.check();

        let metrics = BadouMetrics::new();
        metrics.update_from_health(&report);

        let output = metrics.render();
        assert!(output.contains("badou_commit_total 2"));
        assert!(output.contains("badou_commit_failed_total 1"));
        assert!(output.contains("badou_node_health"));
    }

    #[test]
    fn record_throughput() {
        let metrics = BadouMetrics::new();
        metrics.record_commit_throughput(1_000_000, 1000);
        let output = metrics.render();
        assert!(output.contains("badou_commit_throughput"));
    }

    #[test]
    fn set_chunk_and_version_total() {
        let metrics = BadouMetrics::new();
        metrics.set_chunk_total(500);
        metrics.set_version_total(10);

        let output = metrics.render();
        assert!(output.contains("badou_chunk_total 500"));
        assert!(output.contains("badou_version_total 10"));
    }

    #[test]
    fn set_verify_result() {
        let metrics = BadouMetrics::new();
        metrics.set_verify_result(true, 100, 0);

        let output = metrics.render();
        assert!(output.contains("badou_verify_passed 1"));
        assert!(output.contains("badou_verify_checked_total 100"));
        assert!(output.contains("badou_verify_failed_total 0"));
    }

    #[test]
    fn health_level_to_metric() {
        let checker = HealthChecker::new("node-1");
        let report = checker.check();
        assert_eq!(report.level, HealthLevel::Healthy);

        let metrics = BadouMetrics::new();
        metrics.update_from_health(&report);
        let output = metrics.render();
        assert!(output.contains("badou_node_health 1"));
    }
}