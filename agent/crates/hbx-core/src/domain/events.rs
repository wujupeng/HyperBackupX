use serde::{Deserialize, Serialize};

use super::common::{DeviceId, JobId, PolicyId, VersionId};
use super::chunk::ChunkHash;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DomainEvent {
    BackupStarted(EvtBackupStarted),
    BackupCompleted(EvtBackupCompleted),
    BackupFailed(EvtBackupFailed),
    VersionCreated(EvtVersionCreated),
    VersionDeleted(EvtVersionDeleted),
    ChunkRegistered(EvtChunkRegistered),
    ChunkOrphaned(EvtChunkOrphaned),
    RestoreStarted(EvtRestoreStarted),
    RestoreCompleted(EvtRestoreCompleted),
    VerifyFailed(EvtVerifyFailed),
    DeviceRegistered(EvtDeviceRegistered),
    DeviceOffline(EvtDeviceOffline),
    PolicyUpdated(EvtPolicyUpdated),
    AlertTriggered(EvtAlertTriggered),
    AuditRecorded(EvtAuditRecorded),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvtBackupStarted {
    pub job_id: JobId,
    pub device_id: DeviceId,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvtBackupCompleted {
    pub job_id: JobId,
    pub version_id: VersionId,
    pub duration_secs: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvtBackupFailed {
    pub job_id: JobId,
    pub error_code: String,
    pub error_message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvtVersionCreated {
    pub version_id: VersionId,
    pub job_id: JobId,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvtVersionDeleted {
    pub version_id: VersionId,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvtChunkRegistered {
    pub hash: ChunkHash,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvtChunkOrphaned {
    pub hash: ChunkHash,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvtRestoreStarted {
    pub restore_id: super::common::RestoreId,
    pub version_id: VersionId,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvtRestoreCompleted {
    pub restore_id: super::common::RestoreId,
    pub success: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvtVerifyFailed {
    pub version_id: VersionId,
    pub failure_detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvtDeviceRegistered {
    pub device_id: DeviceId,
    pub hostname: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvtDeviceOffline {
    pub device_id: DeviceId,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvtPolicyUpdated {
    pub policy_id: PolicyId,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvtAlertTriggered {
    pub alert_id: uuid::Uuid,
    pub severity: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvtAuditRecorded {
    pub log_id: uuid::Uuid,
}