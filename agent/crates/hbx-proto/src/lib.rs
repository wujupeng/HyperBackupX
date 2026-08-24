use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterDeviceRequest {
    pub hostname: String,
    pub os_version: String,
    pub agent_version: String,
    pub tier: HardwareTier,
    pub supported_protocols: Vec<String>,
    pub device_fingerprint: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterDeviceResponse {
    pub agent_id: String,
    pub assigned_group: String,
    pub mtls_cert_pem: String,
    pub mtls_ca_pem: String,
    pub heartbeat_interval_secs: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeartbeatRequest {
    pub agent_id: String,
    pub timestamp: DateTime<Utc>,
    pub status: AgentStatus,
    pub resources: ResourceInfo,
    pub protocol_version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeartbeatResponse {
    pub server_time: DateTime<Utc>,
    pub pending_commands: Vec<String>,
    pub config_updated: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AgentStatus {
    Unspecified,
    Idle,
    BackingUp,
    Restoring,
    Paused,
    Error,
    Upgrading,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceInfo {
    pub total_memory_bytes: u64,
    pub available_memory_bytes: u64,
    pub cpu_cores: u32,
    pub disk_free_bytes: u64,
    pub cpu_usage_percent: f64,
    pub disk_io_mbps: f64,
    pub net_io_mbps: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum HardwareTier {
    Unspecified,
    Legacy,
    Standard,
    Modern,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatusReport {
    pub agent_id: String,
    pub timestamp: DateTime<Utc>,
    pub payload: StatusPayload,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum StatusPayload {
    JobProgress(JobProgress),
    AgentHealth(AgentHealth),
    Storage(StorageInfo),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobProgress {
    pub job_id: String,
    pub phase: JobPhase,
    pub bytes_processed: u64,
    pub bytes_total: u64,
    pub files_processed: u32,
    pub files_total: u32,
    pub current_file: String,
    pub throughput_mbps: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum JobPhase {
    Unspecified,
    Started,
    Scanning,
    Chunking,
    Encrypting,
    Uploading,
    Committing,
    Verifying,
    Completed,
    Failed,
    Paused,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentHealth {
    pub status: AgentStatus,
    pub error_message: String,
    pub consecutive_failures: u32,
    pub last_success_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageInfo {
    pub repository_id: String,
    pub total_bytes: u64,
    pub used_bytes: u64,
    pub version_count: u32,
    pub last_backup_size: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandRequest {
    pub agent_id: String,
    pub last_command_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Command {
    pub command_id: String,
    pub issued_at: DateTime<Utc>,
    pub command_type: CommandType,
    pub payload: Vec<u8>,
    pub timeout_secs: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CommandType {
    Unspecified,
    TriggerBackup,
    TriggerRestore,
    PauseJob,
    ResumeJob,
    CancelJob,
    PolicyUpdate,
    ConfigUpdate,
    UpgradeAgent,
    VerifyVersion,
    CleanupOrphans,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskResult {
    pub task_id: String,
    pub agent_id: String,
    pub job_id: String,
    pub status: TaskStatus,
    pub started_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub bytes_processed: u64,
    pub bytes_stored: u64,
    pub file_count: u32,
    pub chunk_count: u32,
    pub dedup_ratio: f64,
    pub version_id: Option<String>,
    pub error_message: Option<String>,
    pub trace_id: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TaskStatus {
    Unspecified,
    Success,
    PartialFailed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FetchPolicyRequest {
    pub agent_id: String,
    pub current_policy_version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FetchPolicyResponse {
    pub policy_id: String,
    pub policy_version: String,
    pub policy_payload: Vec<u8>,
    pub unchanged: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    pub agent_id: String,
    pub timestamp: DateTime<Utc>,
    pub level: String,
    pub message: String,
    pub trace_id: String,
    pub fields: HashMap<String, String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_register_device_roundtrip() {
        let req = RegisterDeviceRequest {
            hostname: "test-host".into(),
            os_version: "Windows 11".into(),
            agent_version: "0.1.0".into(),
            tier: HardwareTier::Modern,
            supported_protocols: vec!["v1".into()],
            device_fingerprint: "abc123".into(),
        };
        let json = serde_json::to_string(&req).unwrap();
        let de: RegisterDeviceRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(de.hostname, "test-host");
        assert_eq!(de.tier, HardwareTier::Modern);
    }

    #[test]
    fn test_heartbeat_roundtrip() {
        let req = HeartbeatRequest {
            agent_id: "agent-1".into(),
            timestamp: Utc::now(),
            status: AgentStatus::Idle,
            resources: ResourceInfo {
                total_memory_bytes: 16_000_000_000,
                available_memory_bytes: 8_000_000_000,
                cpu_cores: 8,
                disk_free_bytes: 500_000_000_000,
                cpu_usage_percent: 12.5,
                disk_io_mbps: 100.0,
                net_io_mbps: 50.0,
            },
            protocol_version: "v1".into(),
        };
        let json = serde_json::to_string(&req).unwrap();
        let de: HeartbeatRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(de.agent_id, "agent-1");
        assert_eq!(de.status, AgentStatus::Idle);
    }

    #[test]
    fn test_command_roundtrip() {
        let cmd = Command {
            command_id: "cmd-1".into(),
            issued_at: Utc::now(),
            command_type: CommandType::TriggerBackup,
            payload: b"backup now".to_vec(),
            timeout_secs: 3600,
        };
        let json = serde_json::to_string(&cmd).unwrap();
        let de: Command = serde_json::from_str(&json).unwrap();
        assert_eq!(de.command_type, CommandType::TriggerBackup);
    }

    #[test]
    fn test_task_result_roundtrip() {
        let result = TaskResult {
            task_id: "task-1".into(),
            agent_id: "agent-1".into(),
            job_id: "job-1".into(),
            status: TaskStatus::Success,
            started_at: Utc::now(),
            completed_at: Some(Utc::now()),
            bytes_processed: 1000,
            bytes_stored: 500,
            file_count: 10,
            chunk_count: 5,
            dedup_ratio: 0.5,
            version_id: Some("v1".into()),
            error_message: None,
            trace_id: "trace-1".into(),
        };
        let json = serde_json::to_string(&result).unwrap();
        let de: TaskResult = serde_json::from_str(&json).unwrap();
        assert_eq!(de.status, TaskStatus::Success);
    }

    #[test]
    fn test_status_report_roundtrip() {
        let report = StatusReport {
            agent_id: "agent-1".into(),
            timestamp: Utc::now(),
            payload: StatusPayload::JobProgress(JobProgress {
                job_id: "job-1".into(),
                phase: JobPhase::Uploading,
                bytes_processed: 500,
                bytes_total: 1000,
                files_processed: 5,
                files_total: 10,
                current_file: "/test/file.txt".into(),
                throughput_mbps: 50.0,
            }),
        };
        let json = serde_json::to_string(&report).unwrap();
        let de: StatusReport = serde_json::from_str(&json).unwrap();
        match de.payload {
            StatusPayload::JobProgress(p) => assert_eq!(p.job_id, "job-1"),
            _ => panic!("wrong payload type"),
        }
    }
}

#[cfg(test)]
mod fuzz_tests {
    use super::*;

    fn random_bytes(seed: &mut u64, n: usize) -> Vec<u8> {
        (0..n)
            .map(|_| {
                *seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
                (*seed >> 33) as u8
            })
            .collect()
    }

    #[test]
    fn fuzz_register_device_request_parser() {
        let mut seed = 42u64;
        for _ in 0..1000 {
            let data = random_bytes(&mut seed, 256);
            let _ = serde_json::from_slice::<RegisterDeviceRequest>(&data);
        }
    }

    #[test]
    fn fuzz_heartbeat_request_parser() {
        let mut seed = 123u64;
        for _ in 0..1000 {
            let data = random_bytes(&mut seed, 256);
            let _ = serde_json::from_slice::<HeartbeatRequest>(&data);
        }
    }

    #[test]
    fn fuzz_command_parser() {
        let mut seed = 999u64;
        for _ in 0..1000 {
            let data = random_bytes(&mut seed, 256);
            let _ = serde_json::from_slice::<Command>(&data);
        }
    }

    #[test]
    fn fuzz_task_result_parser() {
        let mut seed = 7777u64;
        for _ in 0..1000 {
            let data = random_bytes(&mut seed, 256);
            let _ = serde_json::from_slice::<TaskResult>(&data);
        }
    }

    #[test]
    fn fuzz_status_report_parser() {
        let mut seed = 31415u64;
        for _ in 0..1000 {
            let data = random_bytes(&mut seed, 256);
            let _ = serde_json::from_slice::<StatusReport>(&data);
        }
    }

    #[test]
    fn fuzz_malformed_json_no_panic() {
        let malformed_inputs: Vec<&str> = vec![
            "",
            "{",
            "}",
            "{{",
            "}}",
            "{}}",
            "{{}",
            "null",
            "true",
            "false",
            "42",
            "\"",
            "\\",
            "[",
            "]",
            "[,]",
            "{,}",
            "{\"key\":}",
            "{\"key\":\"value\"",
            "{\"key\":\"value\",}",
            "{'key':'value'}",
            "\x00\x01\x02\x03",
        ];

        for input in &malformed_inputs {
            let _ = serde_json::from_str::<RegisterDeviceRequest>(input);
            let _ = serde_json::from_str::<HeartbeatRequest>(input);
            let _ = serde_json::from_str::<Command>(input);
            let _ = serde_json::from_str::<TaskResult>(input);
            let _ = serde_json::from_str::<StatusReport>(input);
        }

        let high_bytes: Vec<u8> = vec![0xff, 0xfe, 0xfd, 0xfc, 0x80, 0x90, 0xa0, 0xb0];
        let _ = serde_json::from_slice::<RegisterDeviceRequest>(&high_bytes);
        let _ = serde_json::from_slice::<HeartbeatRequest>(&high_bytes);
        let _ = serde_json::from_slice::<Command>(&high_bytes);
        let _ = serde_json::from_slice::<TaskResult>(&high_bytes);
        let _ = serde_json::from_slice::<StatusReport>(&high_bytes);
    }
}
