//! 单节点运行模式：无需 Raft，直接启动 HBOP Server。
//!
//! 映射 design.md §2.4.1 单节点部署、spec.md §5.8 规则 2。

use std::path::PathBuf;
use thiserror::Error;

/// 单节点启动配置。
#[derive(Debug, Clone)]
pub struct SingleNodeConfig {
    /// 数据根目录。
    pub data_root: PathBuf,
    /// gRPC 监听地址。
    pub bind_addr: String,
    /// JWT 密钥。
    pub jwt_secret: Vec<u8>,
    /// 是否启用 TLS。
    pub tls: Option<TlsConfig>,
}

/// TLS 配置。
#[derive(Debug, Clone)]
pub struct TlsConfig {
    pub server_cert_pem: Vec<u8>,
    pub server_key_pem: Vec<u8>,
    pub client_ca_cert_pem: Vec<u8>,
}

/// 单节点模式。
pub struct SingleNodeMode {
    config: SingleNodeConfig,
}

#[derive(Debug, Error)]
pub enum SingleNodeError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("invalid bind address: {0}")]
    InvalidBindAddr(String),
    #[error("data root does not exist: {0:?}")]
    DataRootNotFound(PathBuf),
}

impl SingleNodeMode {
    pub fn new(config: SingleNodeConfig) -> Self {
        Self { config }
    }

    pub fn config(&self) -> &SingleNodeConfig {
        &self.config
    }

    /// 验证配置。
    pub fn validate(&self) -> Result<(), SingleNodeError> {
        if !self.config.data_root.exists() {
            std::fs::create_dir_all(&self.config.data_root)?;
        }
        self.config.bind_addr.parse::<std::net::SocketAddr>()
            .map_err(|e| SingleNodeError::InvalidBindAddr(e.to_string()))?;
        Ok(())
    }

    /// 返回数据根目录。
    pub fn data_root(&self) -> &std::path::Path {
        &self.config.data_root
    }

    /// 返回监听地址。
    pub fn bind_addr(&self) -> &str {
        &self.config.bind_addr
    }

    /// 返回 JWT 密钥。
    pub fn jwt_secret(&self) -> &[u8] {
        &self.config.jwt_secret
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_node_config_validate() {
        let tmp = tempfile::tempdir().unwrap();
        let config = SingleNodeConfig {
            data_root: tmp.path().to_path_buf(),
            bind_addr: "127.0.0.1:8080".to_string(),
            jwt_secret: b"secret".to_vec(),
            tls: None,
        };
        let mode = SingleNodeMode::new(config);
        assert!(mode.validate().is_ok());
    }

    #[test]
    fn single_node_invalid_addr_fails() {
        let tmp = tempfile::tempdir().unwrap();
        let config = SingleNodeConfig {
            data_root: tmp.path().to_path_buf(),
            bind_addr: "invalid-addr".to_string(),
            jwt_secret: b"secret".to_vec(),
            tls: None,
        };
        let mode = SingleNodeMode::new(config);
        assert!(mode.validate().is_err());
    }

    #[test]
    fn single_node_creates_data_root() {
        let tmp = tempfile::tempdir().unwrap();
        let data_root = tmp.path().join("nested").join("data");
        let config = SingleNodeConfig {
            data_root: data_root.clone(),
            bind_addr: "127.0.0.1:8080".to_string(),
            jwt_secret: b"secret".to_vec(),
            tls: None,
        };
        let mode = SingleNodeMode::new(config);
        assert!(mode.validate().is_ok());
        assert!(data_root.exists());
    }
}