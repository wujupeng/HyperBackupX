use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use hbx_core::domain::common::RepositoryId;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RepoStatus {
    Active,
    Readonly,
    Deleted,
    Immutable,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoConfig {
    pub name: String,
    pub immutable: bool,
    pub immutable_until: Option<DateTime<Utc>>,
    pub options: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Repository {
    pub repo_id: RepositoryId,
    pub name: String,
    pub created_at: DateTime<Utc>,
    pub config: RepoConfig,
    pub version_count: u64,
    pub status: RepoStatus,
    pub immutable_until: Option<DateTime<Utc>>,
}

impl Repository {
    pub fn new(repo_id: RepositoryId, config: RepoConfig) -> Self {
        Self {
            repo_id,
            name: config.name.clone(),
            created_at: Utc::now(),
            config,
            version_count: 0,
            status: RepoStatus::Active,
            immutable_until: None,
        }
    }

    pub fn is_immutable(&self) -> bool {
        self.status == RepoStatus::Immutable
            || self.config.immutable
            || self.immutable_until.map(|t| t > Utc::now()).unwrap_or(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    #[test]
    fn repository_new() {
        let repo_id = RepositoryId(Uuid::new_v4());
        let config = RepoConfig {
            name: "test-repo".to_string(),
            immutable: false,
            immutable_until: None,
            options: HashMap::new(),
        };
        let repo = Repository::new(repo_id, config);
        assert_eq!(repo.name, "test-repo");
        assert_eq!(repo.status, RepoStatus::Active);
        assert_eq!(repo.version_count, 0);
        assert!(!repo.is_immutable());
    }

    #[test]
    fn repository_immutable_flag() {
        let repo_id = RepositoryId(Uuid::new_v4());
        let config = RepoConfig {
            name: "locked".to_string(),
            immutable: true,
            immutable_until: None,
            options: HashMap::new(),
        };
        let repo = Repository::new(repo_id, config);
        assert!(repo.is_immutable());
    }
}