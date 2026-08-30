use std::collections::VecDeque;
use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use sysinfo::{Networks, Pid, ProcessRefreshKind, ProcessesToUpdate, System};

const WINDOW_SIZE: usize = 12;
const COLLECTION_INTERVAL_SECS: u64 = 5;
const WINDOW_DURATION_SECS: u64 = 60;

#[derive(Debug, Clone, serde::Serialize)]
pub struct ResourceMetrics {
    pub startup_time_ms: u64,
    pub rss_bytes: u64,
    pub cpu_usage_percent: f64,
    pub io_read_bytes: u64,
    pub io_write_bytes: u64,
    pub network_rx_bytes: u64,
    pub network_tx_bytes: u64,
    pub backup_throughput_mbps: f64,
    pub peak_memory_bytes: u64,
    pub avg_memory_bytes: u64,
    pub open_handles: u64,
    pub db_connections: u64,
    pub http_connections: u64,
    pub data_dir_bytes: u64,
    pub log_dir_bytes: u64,
    pub tmp_dir_bytes: u64,
    pub collected_at: u64,
}

impl ResourceMetrics {
    pub fn metric_count() -> usize {
        14
    }

    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "startup_time_ms": self.startup_time_ms,
            "rss_bytes": self.rss_bytes,
            "cpu_usage_percent": self.cpu_usage_percent,
            "io_read_bytes": self.io_read_bytes,
            "io_write_bytes": self.io_write_bytes,
            "network_rx_bytes": self.network_rx_bytes,
            "network_tx_bytes": self.network_tx_bytes,
            "backup_throughput_mbps": self.backup_throughput_mbps,
            "peak_memory_bytes": self.peak_memory_bytes,
            "avg_memory_bytes": self.avg_memory_bytes,
            "open_handles": self.open_handles,
            "db_connections": self.db_connections,
            "http_connections": self.http_connections,
            "data_dir_bytes": self.data_dir_bytes,
            "log_dir_bytes": self.log_dir_bytes,
            "tmp_dir_bytes": self.tmp_dir_bytes,
        })
    }
}


#[derive(Debug, Clone)]
struct MemorySample {
    timestamp: Instant,
    rss_bytes: u64,
}

pub struct ResourceCollector {
    pid: Pid,
    process_start: Instant,
    heartbeat_success: Mutex<Option<Instant>>,
    system: Mutex<System>,
    networks: Mutex<Networks>,
    memory_window: Mutex<VecDeque<MemorySample>>,
    last_io_read: Mutex<u64>,
    last_io_write: Mutex<u64>,
    last_network_rx: Mutex<u64>,
    last_network_tx: Mutex<u64>,
    backup_data_processed: Mutex<u64>,
    backup_start: Mutex<Option<Instant>>,
}

impl ResourceCollector {
    pub fn new() -> Self {
        let pid = sysinfo::get_current_pid().unwrap_or(Pid::from(0));
        let system = System::new();
        let networks = Networks::new_with_refreshed_list();

        Self {
            pid,
            process_start: Instant::now(),
            heartbeat_success: Mutex::new(None),
            system: Mutex::new(system),
            networks: Mutex::new(networks),
            memory_window: Mutex::new(VecDeque::with_capacity(WINDOW_SIZE)),
            last_io_read: Mutex::new(0),
            last_io_write: Mutex::new(0),
            last_network_rx: Mutex::new(0),
            last_network_tx: Mutex::new(0),
            backup_data_processed: Mutex::new(0),
            backup_start: Mutex::new(None),
        }
    }

    pub fn record_heartbeat_success(&self) {
        let mut guard = self.heartbeat_success.lock().unwrap();
        if guard.is_none() {
            *guard = Some(Instant::now());
        }
    }

    pub fn start_backup(&self) {
        let mut guard = self.backup_start.lock().unwrap();
        *guard = Some(Instant::now());
        let mut data = self.backup_data_processed.lock().unwrap();
        *data = 0;
    }

    pub fn record_backup_data(&self, bytes: u64) {
        let mut data = self.backup_data_processed.lock().unwrap();
        *data += bytes;
    }

    pub fn collect(&self) -> ResourceMetrics {
        let mut system = self.system.lock().unwrap();
        system.refresh_processes_specifics(ProcessesToUpdate::Some(&[self.pid]), true, ProcessRefreshKind::everything());

        let startup_time_ms = {
            let guard = self.heartbeat_success.lock().unwrap();
            match *guard {
                Some(success_time) => success_time
                    .duration_since(self.process_start)
                    .as_millis() as u64,
                None => 0,
            }
        };

        let (rss_bytes, cpu_usage_percent, io_read_bytes, io_write_bytes) = {
            if let Some(proc_info) = system.process(self.pid) {
                (
                    proc_info.memory(),
                    proc_info.cpu_usage() as f64,
                    proc_info.disk_usage().read_bytes,
                    proc_info.disk_usage().written_bytes,
                )
            } else {
                (0, 0.0, 0, 0)
            }
        };

        let (network_rx_bytes, network_tx_bytes) = {
            let mut networks = self.networks.lock().unwrap();
            networks.refresh();
            let mut total_rx = 0u64;
            let mut total_tx = 0u64;
            for (_, net) in &*networks {
                total_rx += net.received();
                total_tx += net.transmitted();
            }
            (total_rx, total_tx)
        };

        {
            let mut window = self.memory_window.lock().unwrap();
            window.push_back(MemorySample {
                timestamp: Instant::now(),
                rss_bytes,
            });
            while window.len() > WINDOW_SIZE {
                window.pop_front();
            }
            let cutoff = Instant::now() - Duration::from_secs(WINDOW_DURATION_SECS);
            while window.front().map_or(false, |s| s.timestamp < cutoff) {
                window.pop_front();
            }
        }

        let (peak_memory_bytes, avg_memory_bytes) = {
            let window = self.memory_window.lock().unwrap();
            if window.is_empty() {
                (rss_bytes, rss_bytes)
            } else {
                let peak = window.iter().map(|s| s.rss_bytes).max().unwrap_or(rss_bytes);
                let sum: u64 = window.iter().map(|s| s.rss_bytes).sum();
                let avg = sum / window.len() as u64;
                (peak, avg)
            }
        };

        let backup_throughput_mbps = {
            let data = self.backup_data_processed.lock().unwrap();
            let start = self.backup_start.lock().unwrap();
            match *start {
                Some(start_time) => {
                    let duration_secs = start_time.elapsed().as_secs_f64();
                    if duration_secs > 0.0 {
                        let bytes = *data as f64;
                        let mb = bytes / (1024.0 * 1024.0);
                        mb / duration_secs
                    } else {
                        0.0
                    }
                }
                None => 0.0,
            }
        };

        let collected_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        ResourceMetrics {
            startup_time_ms,
            rss_bytes,
            cpu_usage_percent,
            io_read_bytes,
            io_write_bytes,
            network_rx_bytes,
            network_tx_bytes,
            backup_throughput_mbps,
            peak_memory_bytes,
            avg_memory_bytes,
            open_handles: count_open_handles(self.pid),
            db_connections: 0,
            http_connections: 0,
            data_dir_bytes: dir_size_bytes(&std::env::var("HBX_DATA_DIR").unwrap_or_default()),
            log_dir_bytes: dir_size_bytes(&std::env::var("HBX_LOG_DIR").unwrap_or_default()),
            tmp_dir_bytes: dir_size_bytes(&std::env::var("HBX_TMP_DIR").unwrap_or_default()),
            collected_at,
        }
    }

    pub fn collection_interval() -> Duration {
        Duration::from_secs(COLLECTION_INTERVAL_SECS)
    }

    pub fn window_duration_secs() -> u64 {
        WINDOW_DURATION_SECS
    }
}

fn count_open_handles(pid: Pid) -> u64 {
    #[cfg(target_os = "linux")]
    {
        let path = format!("/proc/{}/fd", pid.as_u32());
        match std::fs::read_dir(&path) {
            Ok(entries) => entries.count() as u64,
            Err(_) => 0,
        }
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = pid;
        0
    }
}

fn dir_size_bytes(path: &str) -> u64 {
    if path.is_empty() {
        return 0;
    }
    let p = std::path::Path::new(path);
    if !p.exists() {
        return 0;
    }
    let mut total = 0u64;
    let mut stack = vec![p.to_path_buf()];
    while let Some(dir) = stack.pop() {
        if let Ok(entries) = std::fs::read_dir(&dir) {
            for entry in entries.flatten() {
                let entry_path = entry.path();
                if entry_path.is_dir() {
                    stack.push(entry_path);
                } else if let Ok(meta) = entry.metadata() {
                    total += meta.len();
                }
            }
        }
    }
    total
}

impl Default for ResourceCollector {
    fn default() -> Self {
        Self::new()
    }
}

const LOW_MEMORY_THRESHOLD_BYTES: u64 = 4 * 1024 * 1024 * 1024;

#[derive(Debug, Clone, serde::Serialize)]
pub struct LowMemoryMode {
    pub enabled: bool,
    pub available_memory_bytes: u64,
    pub threshold_bytes: u64,
    pub reduced_buffer_size: u64,
    pub max_concurrency: u32,
    pub memory_budget_bytes: u64,
}

impl LowMemoryMode {
    pub fn detect() -> Self {
        let mut sys = System::new();
        sys.refresh_memory();
        let available = sys.available_memory();

        if available < LOW_MEMORY_THRESHOLD_BYTES {
            Self {
                enabled: true,
                available_memory_bytes: available,
                threshold_bytes: LOW_MEMORY_THRESHOLD_BYTES,
                reduced_buffer_size: 64 * 1024 * 1024,
                max_concurrency: 1,
                memory_budget_bytes: (available as f64 * 0.7) as u64,
            }
        } else {
            Self {
                enabled: false,
                available_memory_bytes: available,
                threshold_bytes: LOW_MEMORY_THRESHOLD_BYTES,
                reduced_buffer_size: 256 * 1024 * 1024,
                max_concurrency: 4,
                memory_budget_bytes: (available as f64 * 0.8) as u64,
            }
        }
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ResourceTargetReport {
    pub target_name: String,
    pub measured_value: f64,
    pub target_value: f64,
    pub unit: String,
    pub frozen: bool,
    pub status: String,
}

impl ResourceTargetReport {
    pub fn new(name: &str, measured: f64, target: f64, unit: &str, frozen: bool) -> Self {
        let status = if !frozen {
            "pending".to_string()
        } else if target == 0.0 {
            "not_tested".to_string()
        } else if measured <= target {
            "pass".to_string()
        } else {
            "exceed".to_string()
        };

        Self {
            target_name: name.to_string(),
            measured_value: measured,
            target_value: target,
            unit: unit.to_string(),
            frozen,
            status,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resource_collector_creation() {
        let collector = ResourceCollector::new();
        let metrics = collector.collect();
        assert!(metrics.rss_bytes >= 0);
        assert!(metrics.cpu_usage_percent >= 0.0);
    }

    #[test]
    fn test_startup_time_before_heartbeat() {
        let collector = ResourceCollector::new();
        let metrics = collector.collect();
        assert_eq!(metrics.startup_time_ms, 0);
    }

    #[test]
    fn test_startup_time_after_heartbeat() {
        let collector = ResourceCollector::new();
        std::thread::sleep(Duration::from_millis(10));
        collector.record_heartbeat_success();
        let metrics = collector.collect();
        assert!(metrics.startup_time_ms > 0);
    }

    #[test]
    fn test_heartbeat_idempotent() {
        let collector = ResourceCollector::new();
        collector.record_heartbeat_success();
        std::thread::sleep(Duration::from_millis(10));
        collector.record_heartbeat_success();
        let metrics = collector.collect();
        assert!(metrics.startup_time_ms < 20);
    }

    #[test]
    fn test_backup_throughput_no_backup() {
        let collector = ResourceCollector::new();
        let metrics = collector.collect();
        assert_eq!(metrics.backup_throughput_mbps, 0.0);
    }

    #[test]
    fn test_backup_throughput_with_data() {
        let collector = ResourceCollector::new();
        collector.start_backup();
        collector.record_backup_data(10 * 1024 * 1024);
        std::thread::sleep(Duration::from_millis(100));
        let metrics = collector.collect();
        assert!(metrics.backup_throughput_mbps > 0.0);
    }

    #[test]
    fn test_peak_memory_tracking() {
        let collector = ResourceCollector::new();
        let metrics1 = collector.collect();
        std::thread::sleep(Duration::from_millis(10));
        let metrics2 = collector.collect();
        assert!(metrics2.peak_memory_bytes >= metrics2.rss_bytes);
    }

    #[test]
    fn test_avg_memory_tracking() {
        let collector = ResourceCollector::new();
        let _ = collector.collect();
        std::thread::sleep(Duration::from_millis(10));
        let metrics = collector.collect();
        assert!(metrics.avg_memory_bytes > 0);
    }

    #[test]
    fn test_metric_count() {
        assert_eq!(ResourceMetrics::metric_count(), 14);
    }

    #[test]
    fn test_collection_interval() {
        assert_eq!(ResourceCollector::collection_interval(), Duration::from_secs(5));
    }

    #[test]
    fn test_window_duration() {
        assert_eq!(ResourceCollector::window_duration_secs(), 60);
    }

    #[test]
    fn test_metrics_serializable() {
        let collector = ResourceCollector::new();
        let metrics = collector.collect();
        let json = serde_json::to_string(&metrics).unwrap();
        assert!(json.contains("rss_bytes"));
        assert!(json.contains("cpu_usage_percent"));
        assert!(json.contains("backup_throughput_mbps"));
    }

    #[test]
    fn test_metrics_to_json() {
        let collector = ResourceCollector::new();
        let metrics = collector.collect();
        let json = metrics.to_json();
        assert!(json.get("startup_time_ms").is_some());
        assert!(json.get("rss_bytes").is_some());
        assert!(json.get("cpu_usage_percent").is_some());
        assert!(json.get("io_read_bytes").is_some());
        assert!(json.get("io_write_bytes").is_some());
        assert!(json.get("network_rx_bytes").is_some());
        assert!(json.get("network_tx_bytes").is_some());
        assert!(json.get("backup_throughput_mbps").is_some());
        assert!(json.get("peak_memory_bytes").is_some());
        assert!(json.get("avg_memory_bytes").is_some());
    }

    #[test]
    fn test_low_memory_mode_detect() {
        let mode = LowMemoryMode::detect();
        assert!(mode.available_memory_bytes > 0);
        assert_eq!(mode.threshold_bytes, 4 * 1024 * 1024 * 1024);
        if mode.enabled {
            assert_eq!(mode.max_concurrency, 1);
            assert!(mode.reduced_buffer_size < 256 * 1024 * 1024);
        } else {
            assert!(mode.max_concurrency >= 1);
        }
    }

    #[test]
    fn test_open_handles_collected() {
        let collector = ResourceCollector::new();
        let metrics = collector.collect();
        #[cfg(target_os = "linux")]
        assert!(metrics.open_handles > 0);
        #[cfg(not(target_os = "linux"))]
        assert_eq!(metrics.open_handles, 0);
    }

    #[test]
    fn test_dir_size_empty_path() {
        assert_eq!(dir_size_bytes(""), 0);
    }

    #[test]
    fn test_dir_size_nonexistent() {
        assert_eq!(dir_size_bytes("/nonexistent/path/that/does/not/exist"), 0);
    }

    #[test]
    fn test_db_http_connections_default_zero() {
        let collector = ResourceCollector::new();
        let metrics = collector.collect();
        assert_eq!(metrics.db_connections, 0);
        assert_eq!(metrics.http_connections, 0);
    }

    #[test]
    fn test_dir_metrics_in_json() {
        let collector = ResourceCollector::new();
        let metrics = collector.collect();
        let json = metrics.to_json();
        assert!(json.get("open_handles").is_some());
        assert!(json.get("data_dir_bytes").is_some());
        assert!(json.get("log_dir_bytes").is_some());
        assert!(json.get("tmp_dir_bytes").is_some());
    }

    #[test]
    fn test_resource_target_report_pending() {
        let report = ResourceTargetReport::new("rss_bytes", 100.0, 0.0, "bytes", false);
        assert_eq!(report.status, "pending");
        assert!(!report.frozen);
    }

    #[test]
    fn test_resource_target_report_pass() {
        let report = ResourceTargetReport::new("rss_bytes", 100.0, 200.0, "bytes", true);
        assert_eq!(report.status, "pass");
    }

    #[test]
    fn test_resource_target_report_exceed() {
        let report = ResourceTargetReport::new("rss_bytes", 300.0, 200.0, "bytes", true);
        assert_eq!(report.status, "exceed");
    }

    #[test]
    fn test_resource_target_report_not_tested() {
        let report = ResourceTargetReport::new("rss_bytes", 0.0, 0.0, "bytes", true);
        assert_eq!(report.status, "not_tested");
    }
}