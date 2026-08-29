use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

pub struct ApiClient {
    base_url: String,
    token: Option<String>,
}

impl ApiClient {
    pub fn new(base_url: &str, token: Option<String>) -> Self {
        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            token,
        }
    }

    fn request(&self, method: &str, path: &str, body: Option<&str>) -> Result<String> {
        let url = format!("{}{}", self.base_url, path);
        let mut req = match method {
            "GET" => ureq::get(&url),
            "POST" => ureq::post(&url),
            "PUT" => ureq::put(&url),
            "DELETE" => ureq::delete(&url),
            _ => return Err(anyhow::anyhow!("unsupported method: {}", method)),
        };

        if let Some(ref token) = self.token {
            req = req.set("Authorization", &format!("Bearer {}", token));
        }

        let resp = if let Some(json_body) = body {
            req.set("Content-Type", "application/json").send_string(json_body)
        } else {
            req.call()
        };

        let resp = resp.context(format!("{} {} failed", method, url))?;
        let text = resp.into_string().context("read response body")?;
        Ok(text)
    }

    pub fn get(&self, path: &str) -> Result<String> {
        self.request("GET", path, None)
    }

    pub fn post(&self, path: &str, body: &str) -> Result<String> {
        self.request("POST", path, Some(body))
    }

    pub fn delete(&self, path: &str) -> Result<String> {
        self.request("DELETE", path, None)
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CompatRepo {
    pub repo_id: String,
    pub name: String,
    pub root_path: String,
    pub storage_backend: String,
    pub status: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CompatJob {
    pub job_id: String,
    pub name: String,
    pub repo_id: String,
    pub backup_type: String,
    pub dual_repo_mode: String,
    pub status: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CompatExecution {
    pub execution_id: String,
    pub job_id: String,
    pub state: String,
    pub progress: f64,
    pub files_processed: i64,
    pub bytes_processed: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ImportResult {
    pub import_id: String,
    pub status: String,
    pub resulting_job_id: Option<String>,
    pub field_mappings: Vec<serde_json::Value>,
    pub unsupported_items: Vec<serde_json::Value>,
    pub idempotent: bool,
}