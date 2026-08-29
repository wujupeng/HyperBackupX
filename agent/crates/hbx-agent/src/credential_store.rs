use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write;
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Clone)]
pub struct AgentCredentials {
    pub agent_id: String,
    pub agent_token: String,
    pub mtls_cert_pem: Option<String>,
    pub expires_at: u64,
}

#[derive(Debug)]
pub struct CredentialStore {
    path: PathBuf,
}

impl CredentialStore {
    pub fn new() -> Self {
        let path = Self::default_path();
        Self { path }
    }

    fn default_path() -> PathBuf {
        if cfg!(target_os = "windows") {
            PathBuf::from(std::env::var("PROGRAMDATA").unwrap_or_else(|_| "C:\\ProgramData".to_string()))
                .join("hbx-agent")
                .join("credentials.json")
        } else {
            let data_dir = std::env::var("HBX_AGENT_DATA_DIR")
                .unwrap_or_else(|_| {
                    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
                    format!("{}/.hbx", home)
                });
            PathBuf::from(data_dir).join("credentials.json")
        }
    }

    pub fn save(&self, cred: &AgentCredentials) -> Result<()> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).context("create credential dir")?;
        }

        let json = serde_json::to_string_pretty(cred).context("serialize credentials")?;
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&self.path)
            .context("open credential file")?;

        file.write_all(json.as_bytes()).context("write credentials")?;
        drop(file);

        self.restrict_permissions()?;
        Ok(())
    }

    pub fn load(&self) -> Result<Option<AgentCredentials>> {
        if !self.path.exists() {
            return Ok(None);
        }

        let content = fs::read_to_string(&self.path).context("read credential file")?;
        let cred: AgentCredentials = serde_json::from_str(&content).context("parse credentials")?;
        Ok(Some(cred))
    }

    fn restrict_permissions(&self) -> Result<()> {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&self.path, fs::Permissions::from_mode(0o600))
                .context("set file permissions 0600")?;
        }
        #[cfg(windows)]
        {
            // On Windows, the file is created with default ACL.
            // For production, use icacls to restrict to SYSTEM + agent user only.
            // For now, the file is in ProgramData which has restricted access by default.
        }
        Ok(())
    }

    pub fn is_expired(&self, cred: &AgentCredentials) -> bool {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        cred.expires_at <= now
    }

    pub fn needs_refresh(&self, cred: &AgentCredentials) -> bool {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        // Refresh if less than 7 days until expiry
        cred.expires_at <= now + 7 * 24 * 3600
    }
}