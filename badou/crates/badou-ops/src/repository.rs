//! Repository 生命周期管理：Create/Open/List/Delete/Configure/Stat。

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use hbx_core::domain::common::RepositoryId;
use badou_engine::format::BadouDataLayout;
use badou_engine::domain::repository::{Repository, RepoConfig, RepoStatus};
use badou_index::BadouIndex;
use badou_store::{ChunkStore, ManifestStore, SnapshotStore, StagingManager};
use parking_lot::RwLock;
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum RepositoryError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("repository already exists: {0:?}")]
    AlreadyExists(RepositoryId),
    #[error("repository not found: {0:?}")]
    NotFound(RepositoryId),
    #[error("repository is immutable: {0:?}")]
    Immutable(RepositoryId),
    #[error("format error: {0}")]
    Format(#[from] badou_engine::format::FormatError),
    #[error("index error: {0}")]
    Index(#[from] badou_index::IndexError),
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RepoStat {
    pub repo_id: RepositoryId,
    pub name: String,
    pub status: RepoStatus,
    pub version_count: u64,
    pub total_size: u64,
    pub stored_size: u64,
    pub chunk_count: u64,
    pub immutable: bool,
}

pub struct RepositoryHandle {
    pub repo: Repository,
    pub layout: BadouDataLayout,
    pub chunk_store: ChunkStore,
    pub manifest_store: ManifestStore,
    pub snapshot_store: SnapshotStore,
    pub staging: StagingManager,
}

pub struct RepositoryManager {
    data_root: PathBuf,
    repos: RwLock<HashMap<RepositoryId, Arc<RwLock<Repository>>>>,
}

impl RepositoryManager {
    pub fn new(data_root: impl AsRef<std::path::Path>) -> Self {
        Self {
            data_root: data_root.as_ref().to_path_buf(),
            repos: RwLock::new(HashMap::new()),
        }
    }

    pub fn create_repository(
        &self,
        name: &str,
        config: RepoConfig,
    ) -> Result<Repository, RepositoryError> {
        let repo_id = RepositoryId(Uuid::new_v4());
        let layout = BadouDataLayout::new(&self.data_root);

        if layout.repo_root(&repo_id).exists() {
            return Err(RepositoryError::AlreadyExists(repo_id));
        }

        layout.init_repository(&repo_id)?;

        let index_path = layout.index_dir(&repo_id).join("chunk_index.json");
        let _index = BadouIndex::open(&index_path)?;
        let mut repo_config = config;
        repo_config.name = name.to_string();
        let repo = Repository::new(repo_id.clone(), repo_config);

        let meta_path = layout.meta_dir(&repo_id).join("repository.json");
        let json = serde_json::to_vec(&repo)
            .map_err(|e| RepositoryError::Io(std::io::Error::other(e.to_string())))?;
        std::fs::write(&meta_path, &json)?;

        self.repos.write().insert(repo_id, Arc::new(RwLock::new(repo.clone())));

        Ok(repo)
    }

    pub fn open_repository(&self, repo_id: RepositoryId) -> Result<RepositoryHandle, RepositoryError> {
        let layout = BadouDataLayout::new(&self.data_root);
        let repo_root = layout.repo_root(&repo_id);
        if !repo_root.exists() {
            return Err(RepositoryError::NotFound(repo_id));
        }

        let meta_path = layout.meta_dir(&repo_id).join("repository.json");
        let data = std::fs::read(&meta_path)?;
        let repo: Repository = serde_json::from_slice(&data)
            .map_err(|e| RepositoryError::Io(std::io::Error::other(e.to_string())))?;

        let index_path = layout.index_dir(&repo_id).join("chunk_index.json");
        let index = BadouIndex::open(&index_path)?;

        let chunk_store = ChunkStore::new(layout.clone(), index);
        let manifest_store = ManifestStore::new(layout.clone());
        let snapshot_store = SnapshotStore::new(layout.clone());
        let staging = StagingManager::new(layout.clone());

        Ok(RepositoryHandle {
            repo,
            layout,
            chunk_store,
            manifest_store,
            snapshot_store,
            staging,
        })
    }

    pub fn list_repositories(&self) -> Result<Vec<RepoStat>, RepositoryError> {
        let repos_root = self.data_root.join("repositories");
        if !repos_root.exists() {
            return Ok(Vec::new());
        }

        let mut result = Vec::new();
        for entry in std::fs::read_dir(&repos_root)? {
            let entry = entry?;
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let dir_name = entry.file_name().to_string_lossy().to_string();
            let Ok(_repo_uuid) = dir_name.parse::<Uuid>() else { continue };

            let meta_path = path.join(".badou").join("repository.json");
            if !meta_path.exists() {
                continue;
            }
            let data = std::fs::read(&meta_path)?;
            let repo: Repository = serde_json::from_slice(&data)
                .map_err(|e| RepositoryError::Io(std::io::Error::other(e.to_string())))?;

            let index_path = path.join("index").join("chunk_index.json");
            let chunk_count = if index_path.exists() {
                BadouIndex::open(&index_path).map(|i| i.chunk_count() as u64).unwrap_or(0)
            } else {
                0
            };

            result.push(RepoStat {
                repo_id: repo.repo_id.clone(),
                name: repo.name.clone(),
                status: repo.status,
                version_count: repo.version_count,
                total_size: 0,
                stored_size: 0,
                chunk_count,
                immutable: repo.is_immutable(),
            });
        }

        Ok(result)
    }

    pub fn delete_repository(&self, repo_id: RepositoryId) -> Result<(), RepositoryError> {
        let layout = BadouDataLayout::new(&self.data_root);
        let repo_root = layout.repo_root(&repo_id);
        if !repo_root.exists() {
            return Err(RepositoryError::NotFound(repo_id));
        }

        let meta_path = layout.meta_dir(&repo_id).join("repository.json");
        if meta_path.exists() {
            let data = std::fs::read(&meta_path)?;
            let repo: Repository = serde_json::from_slice(&data)
                .map_err(|e| RepositoryError::Io(std::io::Error::other(e.to_string())))?;
            if repo.is_immutable() {
                return Err(RepositoryError::Immutable(repo_id));
            }
        }

        std::fs::remove_dir_all(&repo_root)?;
        self.repos.write().remove(&repo_id);
        Ok(())
    }

    pub fn stat_repository(&self, repo_id: RepositoryId) -> Result<RepoStat, RepositoryError> {
        let layout = BadouDataLayout::new(&self.data_root);
        let repo_root = layout.repo_root(&repo_id);
        if !repo_root.exists() {
            return Err(RepositoryError::NotFound(repo_id));
        }

        let meta_path = layout.meta_dir(&repo_id).join("repository.json");
        let data = std::fs::read(&meta_path)?;
        let repo: Repository = serde_json::from_slice(&data)
            .map_err(|e| RepositoryError::Io(std::io::Error::other(e.to_string())))?;

        let index_path = layout.index_dir(&repo_id).join("chunk_index.json");
        let chunk_count = if index_path.exists() {
            BadouIndex::open(&index_path).map(|i| i.chunk_count() as u64).unwrap_or(0)
        } else {
            0
        };

        Ok(RepoStat {
            repo_id: repo.repo_id.clone(),
            name: repo.name.clone(),
            status: repo.status,
            version_count: repo.version_count,
            total_size: 0,
            stored_size: 0,
            chunk_count,
            immutable: repo.is_immutable(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_mgr() -> RepositoryManager {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().to_path_buf();
        std::mem::forget(tmp);
        RepositoryManager::new(path)
    }

    fn make_config() -> RepoConfig {
        RepoConfig {
            name: String::new(),
            immutable: false,
            immutable_until: None,
            options: std::collections::HashMap::new(),
        }
    }

    #[test]
    fn create_repository_succeeds() {
        let mgr = make_mgr();
        let repo = mgr.create_repository("test-repo", make_config()).unwrap();
        assert_eq!(repo.name, "test-repo");
        assert_eq!(repo.status, RepoStatus::Active);
    }

    #[test]
    fn open_repository_succeeds() {
        let mgr = make_mgr();
        let repo = mgr.create_repository("open-test", make_config()).unwrap();
        let handle = mgr.open_repository(repo.repo_id).unwrap();
        assert_eq!(handle.repo.name, "open-test");
    }

    #[test]
    fn open_nonexistent_fails() {
        let mgr = make_mgr();
        let result = mgr.open_repository(RepositoryId(Uuid::new_v4()));
        assert!(result.is_err());
    }

    #[test]
    fn list_repositories_returns_created() {
        let mgr = make_mgr();
        mgr.create_repository("repo-a", make_config()).unwrap();
        mgr.create_repository("repo-b", make_config()).unwrap();
        let list = mgr.list_repositories().unwrap();
        assert_eq!(list.len(), 2);
    }

    #[test]
    fn delete_repository_succeeds() {
        let mgr = make_mgr();
        let repo = mgr.create_repository("to-delete", make_config()).unwrap();
        let repo_id = repo.repo_id.clone();
        mgr.delete_repository(repo_id.clone()).unwrap();
        let result = mgr.open_repository(repo_id);
        assert!(result.is_err());
    }

    #[test]
    fn stat_repository_returns_info() {
        let mgr = make_mgr();
        let repo = mgr.create_repository("stat-test", make_config()).unwrap();
        let stat = mgr.stat_repository(repo.repo_id).unwrap();
        assert_eq!(stat.name, "stat-test");
        assert_eq!(stat.status, RepoStatus::Active);
    }
}