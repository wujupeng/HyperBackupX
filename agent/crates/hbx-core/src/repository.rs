use async_trait::async_trait;
use thiserror::Error;

use crate::domain::backup::{BackupJob, BackupVersion};
use crate::domain::common::{DeviceId, JobId, RepositoryId, VersionId};
use crate::domain::control::{AuditLog, Policy, Role, User};
use crate::domain::device::Device;
use crate::domain::repository::Repository;

#[derive(Debug, Error)]
pub enum RepositoryError {
    #[error("not found: {0}")]
    NotFound(String),
    #[error("database error: {0}")]
    Database(String),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

type Result<T> = std::result::Result<T, RepositoryError>;

#[async_trait]
pub trait BackupJobRepository: Send + Sync {
    async fn save(&self, job: &BackupJob) -> Result<()>;
    async fn find_by_id(&self, id: &JobId) -> Result<Option<BackupJob>>;
    async fn list_by_device(&self, device_id: &DeviceId) -> Result<Vec<BackupJob>>;
}

#[async_trait]
pub trait BackupVersionRepository: Send + Sync {
    async fn save(&self, version: &BackupVersion) -> Result<()>;
    async fn find_by_id(&self, id: &VersionId) -> Result<Option<BackupVersion>>;
    async fn list_by_job(&self, job_id: &JobId) -> Result<Vec<BackupVersion>>;
    async fn find_latest_success(&self, job_id: &JobId) -> Result<Option<BackupVersion>>;
}

#[async_trait]
pub trait RepositoryRegistry: Send + Sync {
    async fn register(&self, repo: &Repository) -> Result<()>;
    async fn find_by_id(&self, id: &RepositoryId) -> Result<Option<Repository>>;
    async fn list_all(&self) -> Result<Vec<Repository>>;
}

#[async_trait]
pub trait UserRepository: Send + Sync {
    async fn save(&self, user: &User) -> Result<()>;
    async fn find_by_id(&self, id: &crate::domain::common::UserId) -> Result<Option<User>>;
    async fn list_by_org(
        &self,
        org_id: &crate::domain::common::OrganizationId,
    ) -> Result<Vec<User>>;
}

#[async_trait]
pub trait RoleRepository: Send + Sync {
    async fn save(&self, role: &Role) -> Result<()>;
    async fn find_by_id(&self, id: &crate::domain::common::RoleId) -> Result<Option<Role>>;
    async fn list_all(&self) -> Result<Vec<Role>>;
}

#[async_trait]
pub trait DeviceRepository: Send + Sync {
    async fn save(&self, device: &Device) -> Result<()>;
    async fn find_by_id(&self, id: &DeviceId) -> Result<Option<Device>>;
    async fn list_by_org(
        &self,
        org_id: &crate::domain::common::OrganizationId,
    ) -> Result<Vec<Device>>;
}

#[async_trait]
pub trait PolicyRepository: Send + Sync {
    async fn save(&self, policy: &Policy) -> Result<()>;
    async fn find_by_id(&self, id: &crate::domain::common::PolicyId) -> Result<Option<Policy>>;
    async fn list_all(&self) -> Result<Vec<Policy>>;
}

#[async_trait]
pub trait AuditLogRepository: Send + Sync {
    async fn append(&self, log: &AuditLog) -> Result<()>;
    async fn list_recent(&self, limit: u64) -> Result<Vec<AuditLog>>;
}