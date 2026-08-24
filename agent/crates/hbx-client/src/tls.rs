//! mTLS 证书存储与配置
//!
//! Agent 端证书管理：存储/加载 PEM 编码的证书和私钥，
//! 通过 Control Plane 签发 CSR 获取客户端证书。

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// 证书存储路径配置
#[derive(Debug, Clone)]
pub struct CertPaths {
    pub cert_dir: PathBuf,
    pub cert_file: String,
    pub key_file: String,
    pub ca_file: String,
}

impl Default for CertPaths {
    fn default() -> Self {
        let cert_dir = dirs_config_dir().unwrap_or_else(|| PathBuf::from("./certs"));
        Self {
            cert_dir,
            cert_file: "agent.crt".to_string(),
            key_file: "agent.key".to_string(),
            ca_file: "ca.crt".to_string(),
        }
    }
}

impl CertPaths {
    pub fn cert_path(&self) -> PathBuf {
        self.cert_dir.join(&self.cert_file)
    }
    pub fn key_path(&self) -> PathBuf {
        self.cert_dir.join(&self.key_file)
    }
    pub fn ca_path(&self) -> PathBuf {
        self.cert_dir.join(&self.ca_file)
    }
}

fn dirs_config_dir() -> Option<PathBuf> {
    std::env::var("HBX_CERT_DIR")
        .ok()
        .map(PathBuf::from)
        .or_else(|| {
            #[cfg(windows)]
            {
                std::env::var("PROGRAMDATA")
                    .ok()
                    .map(|p| PathBuf::from(p).join("HyperBackupX").join("certs"))
            }
            #[cfg(not(windows))]
            {
                Some(PathBuf::from("/etc/hbx/certs"))
            }
        })
}

/// 证书存储
pub struct CertificateStore {
    paths: CertPaths,
}

/// 证书存储错误
#[derive(Debug, thiserror::Error)]
pub enum CertError {
    #[error("IO error: {0}")]
    Io(#[from] io::Error),
    #[error("Certificate not found at {0}")]
    NotFound(String),
    #[error("Invalid PEM: {0}")]
    InvalidPem(String),
    #[error("Certificate expired")]
    Expired,
    #[error("CA certificate missing")]
    CaMissing,
}

/// PEM 编码的证书材料
#[derive(Debug, Clone)]
pub struct CertMaterial {
    pub cert_pem: String,
    pub key_pem: String,
    pub ca_pem: String,
}

impl Default for CertificateStore {
    fn default() -> Self {
        Self::new(CertPaths::default())
    }
}

impl CertificateStore {
    /// 创建证书存储
    pub fn new(paths: CertPaths) -> Self {
        Self { paths }
    }

    /// 保存证书材料
    pub fn save(&self, material: &CertMaterial) -> Result<(), CertError> {
        fs::create_dir_all(&self.paths.cert_dir)?;

        let key_path = self.paths.key_path();
        fs::write(&key_path, &material.key_pem)?;
        secure_file(&key_path)?;

        fs::write(self.paths.cert_path(), &material.cert_pem)?;
        fs::write(self.paths.ca_path(), &material.ca_pem)?;
        Ok(())
    }

    /// 加载证书材料
    pub fn load(&self) -> Result<CertMaterial, CertError> {
        let cert_pem = fs::read_to_string(self.paths.cert_path())
            .map_err(|e| {
                if e.kind() == io::ErrorKind::NotFound {
                    CertError::NotFound(self.paths.cert_path().to_string_lossy().to_string())
                } else {
                    CertError::Io(e)
                }
            })?;

        let key_pem = fs::read_to_string(self.paths.key_path())
            .map_err(|e| {
                if e.kind() == io::ErrorKind::NotFound {
                    CertError::NotFound(self.paths.key_path().to_string_lossy().to_string())
                } else {
                    CertError::Io(e)
                }
            })?;

        let ca_pem = fs::read_to_string(self.paths.ca_path())
            .map_err(|e| {
                if e.kind() == io::ErrorKind::NotFound {
                    CertError::NotFound(self.paths.ca_path().to_string_lossy().to_string())
                } else {
                    CertError::Io(e)
                }
            })?;

        validate_pem(&cert_pem, "CERTIFICATE")?;
        validate_pem(&key_pem, "PRIVATE KEY")?;
        validate_pem(&ca_pem, "CERTIFICATE")?;

        Ok(CertMaterial {
            cert_pem,
            key_pem,
            ca_pem,
        })
    }

    /// 检查证书是否已存在
    pub fn exists(&self) -> bool {
        self.paths.cert_path().exists()
            && self.paths.key_path().exists()
            && self.paths.ca_path().exists()
    }

    /// 删除证书
    pub fn remove(&self) -> io::Result<()> {
        let _ = fs::remove_file(self.paths.cert_path());
        let _ = fs::remove_file(self.paths.key_path());
        let _ = fs::remove_file(self.paths.ca_path());
        Ok(())
    }

    /// 获取 CA 证书路径
    pub fn ca_path(&self) -> PathBuf {
        self.paths.ca_path()
    }

    /// 获取 Agent 证书路径
    pub fn cert_path(&self) -> PathBuf {
        self.paths.cert_path()
    }

    /// 获取 Agent 私钥路径
    pub fn key_path(&self) -> PathBuf {
        self.paths.key_path()
    }
}

/// 验证 PEM 格式
fn validate_pem(pem: &str, expected_type: &str) -> Result<(), CertError> {
    let trimmed = pem.trim();
    if !trimmed.starts_with("-----BEGIN ") {
        return Err(CertError::InvalidPem("missing BEGIN header".to_string()));
    }
    if !trimmed.contains(expected_type) {
        return Err(CertError::InvalidPem(format!(
            "expected PEM type '{expected_type}'"
        )));
    }
    if !trimmed.contains("-----END ") {
        return Err(CertError::InvalidPem("missing END header".to_string()));
    }
    Ok(())
}

/// 设置文件权限（仅所有者可读写）
fn secure_file(path: &Path) -> Result<(), CertError> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(path)?.permissions();
        perms.set_mode(0o600);
        fs::set_permissions(path, perms)?;
    }
    #[cfg(windows)]
    {
        let _ = path;
    }
    Ok(())
}

/// mTLS 配置
#[derive(Debug, Clone)]
pub struct TlsConfig {
    pub cert_pem: String,
    pub key_pem: String,
    pub ca_pem: String,
    pub server_name: String,
}

impl TlsConfig {
    /// 从证书材料创建 TLS 配置
    pub fn from_material(material: CertMaterial, server_name: &str) -> Self {
        Self {
            cert_pem: material.cert_pem,
            key_pem: material.key_pem,
            ca_pem: material.ca_pem,
            server_name: server_name.to_string(),
        }
    }

    /// 从证书存储创建 TLS 配置
    pub fn from_store(store: &CertificateStore, server_name: &str) -> Result<Self, CertError> {
        let material = store.load()?;
        Ok(Self::from_material(material, server_name))
    }

    /// 是否已配置完整的 mTLS
    pub fn is_complete(&self) -> bool {
        !self.cert_pem.is_empty() && !self.key_pem.is_empty() && !self.ca_pem.is_empty()
    }
}

/// CSR 签发请求/响应
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SignCsrRequest {
    pub device_id: String,
    pub csr_pem: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SignCsrResponse {
    pub cert_pem: String,
    pub ca_pem: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_cert_paths_default() {
        let paths = CertPaths::default();
        assert!(paths.cert_path().to_string_lossy().contains("agent.crt"));
        assert!(paths.key_path().to_string_lossy().contains("agent.key"));
        assert!(paths.ca_path().to_string_lossy().contains("ca.crt"));
    }

    #[test]
    fn test_certificate_store_save_load() {
        let dir = tempdir().unwrap();
        let paths = CertPaths {
            cert_dir: dir.path().to_path_buf(),
            cert_file: "agent.crt".to_string(),
            key_file: "agent.key".to_string(),
            ca_file: "ca.crt".to_string(),
        };
        let store = CertificateStore::new(paths);

        let material = CertMaterial {
            cert_pem: "-----BEGIN CERTIFICATE-----\nMIIB\n-----END CERTIFICATE-----\n".to_string(),
            key_pem: "-----BEGIN EC PRIVATE KEY-----\nMHc\n-----END EC PRIVATE KEY-----\n".to_string(),
            ca_pem: "-----BEGIN CERTIFICATE-----\nMIIB\n-----END CERTIFICATE-----\n".to_string(),
        };

        store.save(&material).unwrap();
        assert!(store.exists());

        let loaded = store.load().unwrap();
        assert_eq!(loaded.cert_pem, material.cert_pem);
        assert_eq!(loaded.key_pem, material.key_pem);
        assert_eq!(loaded.ca_pem, material.ca_pem);
    }

    #[test]
    fn test_certificate_store_not_found() {
        let dir = tempdir().unwrap();
        let paths = CertPaths {
            cert_dir: dir.path().to_path_buf(),
            cert_file: "agent.crt".to_string(),
            key_file: "agent.key".to_string(),
            ca_file: "ca.crt".to_string(),
        };
        let store = CertificateStore::new(paths);

        assert!(!store.exists());
        let result = store.load();
        assert!(matches!(result, Err(CertError::NotFound(_))));
    }

    #[test]
    fn test_certificate_store_remove() {
        let dir = tempdir().unwrap();
        let paths = CertPaths {
            cert_dir: dir.path().to_path_buf(),
            cert_file: "agent.crt".to_string(),
            key_file: "agent.key".to_string(),
            ca_file: "ca.crt".to_string(),
        };
        let store = CertificateStore::new(paths);

        let material = CertMaterial {
            cert_pem: "-----BEGIN CERTIFICATE-----\nMIIB\n-----END CERTIFICATE-----\n".to_string(),
            key_pem: "-----BEGIN EC PRIVATE KEY-----\nMHc\n-----END EC PRIVATE KEY-----\n".to_string(),
            ca_pem: "-----BEGIN CERTIFICATE-----\nMIIB\n-----END CERTIFICATE-----\n".to_string(),
        };

        store.save(&material).unwrap();
        assert!(store.exists());
        store.remove().unwrap();
        assert!(!store.exists());
    }

    #[test]
    fn test_validate_pem_valid() {
        let pem = "-----BEGIN CERTIFICATE-----\nMIIB\n-----END CERTIFICATE-----\n";
        assert!(validate_pem(pem, "CERTIFICATE").is_ok());
    }

    #[test]
    fn test_validate_pem_missing_header() {
        let pem = "MIIB";
        assert!(validate_pem(pem, "CERTIFICATE").is_err());
    }

    #[test]
    fn test_validate_pem_wrong_type() {
        let pem = "-----BEGIN CERTIFICATE-----\nMIIB\n-----END CERTIFICATE-----\n";
        assert!(validate_pem(pem, "PRIVATE KEY").is_err());
    }

    #[test]
    fn test_tls_config_from_material() {
        let material = CertMaterial {
            cert_pem: "-----BEGIN CERTIFICATE-----\nMIIB\n-----END CERTIFICATE-----\n".to_string(),
            key_pem: "-----BEGIN EC PRIVATE KEY-----\nMHc\n-----END EC PRIVATE KEY-----\n".to_string(),
            ca_pem: "-----BEGIN CERTIFICATE-----\nMIIB\n-----END CERTIFICATE-----\n".to_string(),
        };
        let config = TlsConfig::from_material(material, "control.hbx.local");
        assert!(config.is_complete());
        assert_eq!(config.server_name, "control.hbx.local");
    }

    #[test]
    fn test_tls_config_incomplete() {
        let config = TlsConfig {
            cert_pem: "".to_string(),
            key_pem: "".to_string(),
            ca_pem: "".to_string(),
            server_name: "test".to_string(),
        };
        assert!(!config.is_complete());
    }

    #[test]
    fn test_sign_csr_roundtrip() {
        let req = SignCsrRequest {
            device_id: "device-001".to_string(),
            csr_pem: "-----BEGIN CERTIFICATE REQUEST-----\nMIIB\n-----END CERTIFICATE REQUEST-----\n".to_string(),
        };
        let json = serde_json::to_string(&req).unwrap();
        let decoded: SignCsrRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.device_id, "device-001");
    }
}