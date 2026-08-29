//! 服务端共享状态：持有 RepositoryManager、VersionOps、AuthConfig 等。

use std::path::PathBuf;
use std::sync::Arc;
use parking_lot::RwLock;
use std::collections::HashMap;

use hbx_core::domain::common::RepositoryId;
use badou_ops::{RepositoryManager, VersionOps};
use crate::auth::AuthConfig;

/// 已打开的 Repository 句柄缓存。
pub struct OpenRepo {
    chunk_store: badou_store::ChunkStore,
    manifest_store: badou_store::ManifestStore,
    snapshot_store: badou_store::SnapshotStore,
    staging: badou_store::StagingManager,
    index: badou_index::BadouIndex,
    journal: badou_journal::BadouJournal,
}

/// 服务端共享状态。
pub struct ServerState {
    data_root: PathBuf,
    repo_manager: RepositoryManager,
    version_ops: VersionOps,
    auth_config: AuthConfig,
    open_repos: RwLock<HashMap<RepositoryId, Arc<OpenRepo>>>,
}

impl ServerState {
    pub fn new(data_root: impl AsRef<std::path::Path>, auth_config: AuthConfig) -> Self {
        Self {
            data_root: data_root.as_ref().to_path_buf(),
            repo_manager: RepositoryManager::new(data_root.as_ref()),
            version_ops: VersionOps::new(),
            auth_config,
            open_repos: RwLock::new(HashMap::new()),
        }
    }

    pub fn data_root(&self) -> &std::path::Path {
        &self.data_root
    }

    pub fn repo_manager(&self) -> &RepositoryManager {
        &self.repo_manager
    }

    pub fn version_ops(&self) -> &VersionOps {
        &self.version_ops
    }

    pub fn auth_config(&self) -> &AuthConfig {
        &self.auth_config
    }

    /// 打开 Repository 并缓存句柄，返回共享引用。
    pub fn open_repo(
        &self,
        repo_id: &RepositoryId,
    ) -> Result<Arc<OpenRepo>, badou_ops::RepositoryError> {
        {
            let cache = self.open_repos.read();
            if let Some(handle) = cache.get(repo_id) {
                return Ok(handle.clone());
            }
        }

        let handle = self.repo_manager.open_repository(repo_id.clone())?;
        let index_path = handle.layout.index_dir(repo_id).join("chunk_index.json");
        let index = badou_index::BadouIndex::open(&index_path)
            .map_err(badou_ops::RepositoryError::Index)?;
        let journal_path = handle.layout.journal_dir(repo_id).join("journal.log");
        let journal = badou_journal::BadouJournal::open(&journal_path)
            .map_err(|e| badou_ops::RepositoryError::Io(std::io::Error::other(e.to_string())))?;

        let open_repo = Arc::new(OpenRepo {
            chunk_store: handle.chunk_store,
            manifest_store: handle.manifest_store,
            snapshot_store: handle.snapshot_store,
            staging: handle.staging,
            index,
            journal,
        });

        self.open_repos
            .write()
            .insert(repo_id.clone(), open_repo.clone());
        Ok(open_repo)
    }

    /// 获取已缓存的 Repository 句柄（不重新打开）。
    pub fn get_open_repo(&self, repo_id: &RepositoryId) -> Option<Arc<OpenRepo>> {
        self.open_repos.read().get(repo_id).cloned()
    }

    /// 从缓存中移除 Repository。
    pub fn close_repo(&self, repo_id: &RepositoryId) {
        self.open_repos.write().remove(repo_id);
    }
}

// 公开 OpenRepo 的字段访问器
impl OpenRepo {
    pub fn chunk_store(&self) -> &badou_store::ChunkStore {
        &self.chunk_store
    }

    pub fn manifest_store(&self) -> &badou_store::ManifestStore {
        &self.manifest_store
    }

    pub fn snapshot_store(&self) -> &badou_store::SnapshotStore {
        &self.snapshot_store
    }

    pub fn staging(&self) -> &badou_store::StagingManager {
        &self.staging
    }

    pub fn index(&self) -> &badou_index::BadouIndex {
        &self.index
    }

    pub fn journal(&self) -> &badou_journal::BadouJournal {
        &self.journal
    }
}