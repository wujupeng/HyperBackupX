use std::path::PathBuf;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub data_root: PathBuf,
    pub bind_addr: String,
    pub metrics_addr: String,
    #[serde(default)]
    pub management_addr: Option<String>,
    pub jwt_secret: String,
    #[serde(default)]
    pub tls: Option<TlsPaths>,
    pub cluster: ClusterConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TlsPaths {
    pub server_cert: PathBuf,
    pub server_key: PathBuf,
    pub client_ca_cert: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(tag = "mode", rename_all = "lowercase")]
pub enum ClusterConfig {
    #[default]
    Single,
    Raft {
        node_id: String,
        #[serde(default)]
        peers: Vec<String>,
    },
}

impl ServerConfig {
    pub fn load(path: &std::path::Path) -> anyhow::Result<Self> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| anyhow::anyhow!("读取配置文件失败 {}: {}", path.display(), e))?;
        let config: ServerConfig = serde_json::from_str(&content)
            .map_err(|e| anyhow::anyhow!("解析配置文件失败: {}", e))?;
        config.validate()?;
        Ok(config)
    }

    pub fn validate(&self) -> anyhow::Result<()> {
        if self.jwt_secret.is_empty() {
            anyhow::bail!("jwt_secret 不能为空");
        }
        self.bind_addr
            .parse::<std::net::SocketAddr>()
            .map_err(|e| anyhow::anyhow!("bind_addr 无效: {}", e))?;
        self.metrics_addr
            .parse::<std::net::SocketAddr>()
            .map_err(|e| anyhow::anyhow!("metrics_addr 无效: {}", e))?;
        if let Some(ref mgmt) = self.management_addr {
            mgmt.parse::<std::net::SocketAddr>()
                .map_err(|e| anyhow::anyhow!("management_addr 无效: {}", e))?;
        }
        Ok(())
    }

    #[allow(dead_code)]
    pub fn default_for(data_root: PathBuf) -> Self {
        Self {
            data_root,
            bind_addr: "0.0.0.0:9090".to_string(),
            metrics_addr: "0.0.0.0:9091".to_string(),
            management_addr: Some("0.0.0.0:9092".to_string()),
            jwt_secret: "change-me-please".to_string(),
            tls: None,
            cluster: ClusterConfig::Single,
        }
    }

    #[allow(dead_code)]
    pub fn to_json(&self) -> anyhow::Result<String> {
        serde_json::to_string_pretty(self)
            .map_err(|e| anyhow::anyhow!("序列化配置失败: {}", e))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_serialize_deserialize() {
        let config = ServerConfig::default_for(PathBuf::from("/data/badou"));
        let json = config.to_json().unwrap();
        let parsed: ServerConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.bind_addr, config.bind_addr);
        assert_eq!(parsed.jwt_secret, config.jwt_secret);
    }

    #[test]
    fn config_raft_mode() {
        let json = r#"{
            "data_root": "/data/badou",
            "bind_addr": "0.0.0.0:9090",
            "metrics_addr": "0.0.0.0:9091",
            "jwt_secret": "secret",
            "cluster": {
                "mode": "raft",
                "node_id": "node-1",
                "peers": ["node-2:9090", "node-3:9090"]
            }
        }"#;
        let config: ServerConfig = serde_json::from_str(json).unwrap();
        assert!(matches!(config.cluster, ClusterConfig::Raft { .. }));
    }

    #[test]
    fn config_validate_empty_secret_fails() {
        let config = ServerConfig {
            data_root: PathBuf::from("/tmp"),
            bind_addr: "0.0.0.0:9090".to_string(),
            metrics_addr: "0.0.0.0:9091".to_string(),
            management_addr: None,
            jwt_secret: String::new(),
            tls: None,
            cluster: ClusterConfig::Single,
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn config_validate_invalid_addr_fails() {
        let config = ServerConfig {
            data_root: PathBuf::from("/tmp"),
            bind_addr: "invalid".to_string(),
            metrics_addr: "0.0.0.0:9091".to_string(),
            management_addr: None,
            jwt_secret: "secret".to_string(),
            tls: None,
            cluster: ClusterConfig::Single,
        };
        assert!(config.validate().is_err());
    }
}