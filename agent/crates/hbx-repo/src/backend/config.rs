use serde::{Deserialize, Serialize};

use hbx_core::domain::repository::BackendType;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackendConfig {
    pub backend_type: BackendType,
    pub connection: ConnectionConfig,
    pub credentials_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "params")]
pub enum ConnectionConfig {
    Local { root_path: String },
    S3(S3Config),
    WebDav(WebDavConfig),
    Sftp(SftpConfig),
    Ftp(FtpConfig),
    Smb(SmbConfig),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct S3Config {
    pub endpoint: String,
    pub region: String,
    pub bucket: String,
    pub use_tls: bool,
    pub path_style: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebDavConfig {
    pub endpoint: String,
    pub base_path: String,
    pub use_tls: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SftpConfig {
    pub host: String,
    pub port: u16,
    pub base_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FtpConfig {
    pub host: String,
    pub port: u16,
    pub base_path: String,
    pub use_tls: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SmbConfig {
    pub host: String,
    pub share: String,
    pub base_path: String,
}

impl BackendConfig {
    pub fn local(root_path: impl Into<String>) -> Self {
        Self {
            backend_type: BackendType::Local,
            connection: ConnectionConfig::Local {
                root_path: root_path.into(),
            },
            credentials_id: String::new(),
        }
    }

    pub fn s3(endpoint: impl Into<String>, region: impl Into<String>, bucket: impl Into<String>) -> Self {
        Self {
            backend_type: BackendType::S3,
            connection: ConnectionConfig::S3(S3Config {
                endpoint: endpoint.into(),
                region: region.into(),
                bucket: bucket.into(),
                use_tls: true,
                path_style: false,
            }),
            credentials_id: String::new(),
        }
    }

    pub fn webdav(endpoint: impl Into<String>, base_path: impl Into<String>) -> Self {
        Self {
            backend_type: BackendType::Webdav,
            connection: ConnectionConfig::WebDav(WebDavConfig {
                endpoint: endpoint.into(),
                base_path: base_path.into(),
                use_tls: true,
            }),
            credentials_id: String::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_local_config() {
        let config = BackendConfig::local("/tmp/repo");
        assert_eq!(config.backend_type, BackendType::Local);
    }

    #[test]
    fn test_s3_config() {
        let config = BackendConfig::s3("s3.amazonaws.com", "us-east-1", "mybucket");
        assert_eq!(config.backend_type, BackendType::S3);
    }

    #[test]
    fn test_webdav_config() {
        let config = BackendConfig::webdav("https://dav.example.com", "/backup");
        assert_eq!(config.backend_type, BackendType::Webdav);
    }

    #[test]
    fn test_config_serialization() {
        let config = BackendConfig::s3("s3.amazonaws.com", "us-east-1", "mybucket");
        let json = serde_json::to_string(&config).unwrap();
        let deserialized: BackendConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.backend_type, BackendType::S3);
    }
}