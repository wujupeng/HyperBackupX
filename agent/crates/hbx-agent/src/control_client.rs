use anyhow::{Context, Result};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::time::Duration;

const PROTOCOL_VERSION: &str = "1";

#[derive(Debug, Serialize)]
pub struct RegisterRequest {
    pub hostname: String,
    pub os_version: String,
    pub agent_version: String,
    pub tier: String,
    pub supported_protocols: Vec<String>,
    pub device_fingerprint: String,
}

#[derive(Debug, Deserialize)]
pub struct RegisterResponse {
    pub agent_id: String,
    pub assigned_group: String,
    #[serde(default)]
    pub agent_token: String,
    #[serde(default)]
    pub mtls_cert_pem: String,
    #[serde(default)]
    pub mtls_ca_pem: String,
    pub heartbeat_interval_secs: u32,
}

#[derive(Debug, Serialize)]
pub struct HeartbeatRequest {
    pub agent_id: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub status: String,
    pub resources: serde_json::Value,
    pub protocol_version: String,
}

#[derive(Debug, Deserialize)]
pub struct HeartbeatResponse {
    pub server_time: chrono::DateTime<chrono::Utc>,
    #[serde(default)]
    pub pending_commands: Vec<String>,
    #[serde(default)]
    pub config_updated: bool,
}

#[derive(Debug, Serialize)]
pub struct TaskResultRequest {
    pub task_id: String,
    pub agent_id: String,
    pub job_id: String,
    pub status: String,
    pub started_at: chrono::DateTime<chrono::Utc>,
    pub completed_at: Option<chrono::DateTime<chrono::Utc>>,
    pub bytes_processed: u64,
    pub bytes_stored: u64,
    pub file_count: u32,
    pub chunk_count: u32,
    pub dedup_ratio: f64,
    pub version_id: Option<String>,
    pub error_message: Option<String>,
    pub trace_id: String,
}

pub struct ControlClient {
    base_url: String,
    agent_id: Option<String>,
    agent_token: Option<String>,
    heartbeat_interval: u32,
}

impl ControlClient {
    pub fn new(base_url: &str) -> Self {
        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            agent_id: None,
            agent_token: None,
            heartbeat_interval: 30,
        }
    }

    pub fn agent_id(&self) -> Option<&str> {
        self.agent_id.as_deref()
    }

    pub fn agent_token(&self) -> Option<&str> {
        self.agent_token.as_deref()
    }

    pub fn heartbeat_interval(&self) -> u32 {
        self.heartbeat_interval
    }

    pub fn register(&mut self, req: &RegisterRequest) -> Result<RegisterResponse> {
        let url = format!("{}/api/v1/agent/register", self.base_url);
        let body = serde_json::to_string(req).context("serialize register request")?;

        let resp = ureq::post(&url)
            .set("Content-Type", "application/json")
            .send_string(&body)
            .context("register request failed")?;

        let resp_text = resp.into_string().context("read register response")?;
        let resp_data: RegisterResponse =
            serde_json::from_str(&resp_text).context("parse register response")?;

        self.agent_id = Some(resp_data.agent_id.clone());
        self.agent_token = if resp_data.agent_token.is_empty() { None } else { Some(resp_data.agent_token.clone()) };
        self.heartbeat_interval = resp_data.heartbeat_interval_secs;

        Ok(resp_data)
    }

    pub fn heartbeat(&self, status: &str, resources: serde_json::Value) -> Result<HeartbeatResponse> {
        let agent_id = self
            .agent_id
            .as_ref()
            .context("agent not registered")?;

        let req = HeartbeatRequest {
            agent_id: agent_id.clone(),
            timestamp: Utc::now(),
            status: status.to_string(),
            resources,
            protocol_version: PROTOCOL_VERSION.to_string(),
        };

        let url = format!("{}/api/v1/agent/heartbeat", self.base_url);
        let body = serde_json::to_string(&req).context("serialize heartbeat")?;

        let request = ureq::post(&url)
            .set("Content-Type", "application/json");

        let request = if let Some(token) = &self.agent_token {
            request.set("Authorization", &format!("Bearer {}", token))
        } else {
            request
        };

        let resp = request
            .send_string(&body)
            .context("heartbeat request failed")?;

        let resp_text = resp.into_string().context("read heartbeat response")?;
        let resp_data: HeartbeatResponse =
            serde_json::from_str(&resp_text).context("parse heartbeat response")?;

        Ok(resp_data)
    }

    pub fn report_task_result(&self, req: &TaskResultRequest) -> Result<()> {
        let url = format!("{}/api/v1/agent/task-result", self.base_url);
        let body = serde_json::to_string(req).context("serialize task result")?;

        let request = ureq::post(&url)
            .set("Content-Type", "application/json");

        let request = if let Some(token) = &self.agent_token {
            request.set("Authorization", &format!("Bearer {}", token))
        } else {
            request
        };

        let resp = request
            .send_string(&body)
            .context("task-result request failed")?;

        let _ = resp.into_string();
        Ok(())
    }

    pub fn register_with_retry(&mut self, req: &RegisterRequest, max_retries: u32) -> Result<RegisterResponse> {
        let mut delay = Duration::from_secs(1);
        let mut last_err = None;

        for attempt in 0..max_retries {
            match self.register(req) {
                Ok(resp) => return Ok(resp),
                Err(e) => {
                    last_err = Some(e);
                    if attempt < max_retries - 1 {
                        tracing::warn!(
                            "register attempt {} failed, retrying in {:?}",
                            attempt + 1,
                            delay
                        );
                        std::thread::sleep(delay);
                        delay = std::cmp::min(delay * 2, Duration::from_secs(60));
                    }
                }
            }
        }

        Err(last_err.unwrap_or_else(|| anyhow::anyhow!("register failed after retries")))
    }
}