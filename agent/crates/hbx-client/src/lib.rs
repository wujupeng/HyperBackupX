use chrono::Utc;
use hbx_proto::*;
use thiserror::Error;

pub mod tls;
pub use tls::{
    CertificateStore, CertPaths, CertMaterial, CertError, TlsConfig,
    SignCsrRequest, SignCsrResponse,
};

#[derive(Debug, Error)]
pub enum ClientError {
    #[error("HTTP error: {0}")]
    Http(String),
    #[error("JSON error: {0}")]
    Json(String),
    #[error("Not connected")]
    NotConnected,
}

pub struct HbxClient {
    server_url: String,
    agent_id: Option<String>,
    auth_token: Option<String>,
}

impl HbxClient {
    pub fn new(server_url: &str) -> Self {
        Self {
            server_url: server_url.trim_end_matches('/').to_string(),
            agent_id: None,
            auth_token: None,
        }
    }

    pub fn with_auth(mut self, token: &str) -> Self {
        self.auth_token = Some(token.to_string());
        self
    }

    pub fn agent_id(&self) -> Option<&str> {
        self.agent_id.as_deref()
    }

    pub fn set_agent_id(&mut self, agent_id: &str) {
        self.agent_id = Some(agent_id.to_string());
    }

    fn post<T: serde::Serialize, R: serde::de::DeserializeOwned>(
        &self,
        path: &str,
        body: &T,
    ) -> Result<R, ClientError> {
        let url = format!("{}{}", self.server_url, path);
        let mut req = ureq::post(&url);
        if let Some(ref token) = self.auth_token {
            req = req.set("Authorization", &format!("Bearer {token}"));
        }
        let json = serde_json::to_string(body)
            .map_err(|e| ClientError::Json(e.to_string()))?;
        let resp = req
            .set("Content-Type", "application/json")
            .send_string(&json)
            .map_err(|e| ClientError::Http(e.to_string()))?;
        let body = resp
            .into_string()
            .map_err(|e| ClientError::Http(e.to_string()))?;
        serde_json::from_str(&body).map_err(|e| ClientError::Json(e.to_string()))
    }

    pub fn register_device(
        &mut self,
        req: &RegisterDeviceRequest,
    ) -> Result<RegisterDeviceResponse, ClientError> {
        let resp: RegisterDeviceResponse = self.post("/api/v1/agent/register", req)?;
        self.agent_id = Some(resp.agent_id.clone());
        Ok(resp)
    }

    pub fn heartbeat(
        &self,
        status: AgentStatus,
        resources: ResourceInfo,
    ) -> Result<HeartbeatResponse, ClientError> {
        let agent_id = self.agent_id.as_ref().ok_or(ClientError::NotConnected)?;
        let req = HeartbeatRequest {
            agent_id: agent_id.clone(),
            timestamp: Utc::now(),
            status,
            resources,
            protocol_version: "v1".to_string(),
        };
        self.post::<_, HeartbeatResponse>("/api/v1/agent/heartbeat", &req)
    }

    pub fn report_task_result(
        &self,
        result: &TaskResult,
    ) -> Result<TaskResultAck, ClientError> {
        self.post::<_, TaskResultAck>("/api/v1/agent/task-result", result)
    }

    pub fn fetch_policy(
        &self,
        current_version: &str,
    ) -> Result<FetchPolicyResponse, ClientError> {
        let agent_id = self.agent_id.as_ref().ok_or(ClientError::NotConnected)?;
        let req = FetchPolicyRequest {
            agent_id: agent_id.clone(),
            current_policy_version: current_version.to_string(),
        };
        self.post::<_, FetchPolicyResponse>("/api/v1/agent/fetch-policy", &req)
    }

    pub fn report_status(&self, report: &StatusReport) -> Result<StatusAck, ClientError> {
        self.post::<_, StatusAck>("/api/v1/agent/status", report)
    }

    pub fn report_log(&self, entry: &LogEntry) -> Result<LogAck, ClientError> {
        self.post::<_, LogAck>("/api/v1/agent/log", entry)
    }

    /// 提交 CSR 给 Control Plane 签发，返回签发的证书和 CA 证书
    pub fn sign_csr(&self, device_id: &str, csr_pem: &str) -> Result<SignCsrResponse, ClientError> {
        let req = SignCsrRequest {
            device_id: device_id.to_string(),
            csr_pem: csr_pem.to_string(),
        };
        self.post::<_, SignCsrResponse>("/api/v1/agent/sign-csr", &req)
    }

    /// 获取 Control Plane 的 CA 证书（PEM 格式）
    pub fn get_ca_cert(&self) -> Result<String, ClientError> {
        let url = format!("{}/api/v1/agent/ca-cert", self.server_url);
        let resp = ureq::get(&url)
            .call()
            .map_err(|e| ClientError::Http(e.to_string()))?;
        resp.into_string()
            .map_err(|e| ClientError::Http(e.to_string()))
    }

    /// 完成 mTLS 引导：提交 CSR → 保存签发证书 → 返回 TLS 配置
    pub fn bootstrap_mtls(
        &self,
        device_id: &str,
        csr_pem: &str,
        key_pem: &str,
        store: &CertificateStore,
        server_name: &str,
    ) -> Result<TlsConfig, ClientError> {
        let resp = self.sign_csr(device_id, csr_pem)?;

        let material = CertMaterial {
            cert_pem: resp.cert_pem,
            key_pem: key_pem.to_string(),
            ca_pem: resp.ca_pem,
        };

        store.save(&material).map_err(|e| ClientError::Http(e.to_string()))?;

        Ok(TlsConfig::from_material(material, server_name))
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TaskResultAck {
    pub accepted: bool,
    pub message: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct StatusAck {
    pub accepted: bool,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct LogAck {
    pub accepted: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_creation() {
        let client = HbxClient::new("http://localhost:8080");
        assert_eq!(client.server_url, "http://localhost:8080");
        assert!(client.agent_id.is_none());
    }

    #[test]
    fn test_client_with_auth() {
        let client = HbxClient::new("http://localhost:8080").with_auth("test-token");
        assert_eq!(client.auth_token, Some("test-token".to_string()));
    }

    #[test]
    fn test_not_connected_error() {
        let client = HbxClient::new("http://localhost:8080");
        let result = client.heartbeat(
            AgentStatus::Idle,
            ResourceInfo {
                total_memory_bytes: 0,
                available_memory_bytes: 0,
                cpu_cores: 0,
                disk_free_bytes: 0,
                cpu_usage_percent: 0.0,
                disk_io_mbps: 0.0,
                net_io_mbps: 0.0,
            },
        );
        assert!(matches!(result, Err(ClientError::NotConnected)));
    }

    #[test]
    fn test_register_sets_agent_id() {
        let mut client = HbxClient::new("http://localhost:8080");
        client.agent_id = Some("test-agent".to_string());
        assert_eq!(client.agent_id(), Some("test-agent"));
    }
}
