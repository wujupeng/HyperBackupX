use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::common::{DeviceId, OrganizationId, PolicyId, RoleId, UserId};

use super::schedule::{RetentionPolicy, Schedule};
use super::encryption::EncryptionProfile;
use super::common::RateLimit;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub user_id: UserId,
    pub username: String,
    pub display_name: String,
    pub email: String,
    pub roles: Vec<RoleId>,
    pub organization_id: OrganizationId,
    pub status: UserStatus,
    pub auth_source: AuthSource,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum UserStatus {
    Active,
    Disabled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuthSource {
    Local,
    Ad,
    Ldap,
    Oidc,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Role {
    pub role_id: RoleId,
    pub name: String,
    pub permissions: Vec<Permission>,
    pub is_builtin: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Permission {
    pub resource: String,
    pub actions: Vec<Action>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Action {
    Read,
    Create,
    Update,
    Delete,
    Execute,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Organization {
    pub organization_id: OrganizationId,
    pub name: String,
    pub parent_id: Option<OrganizationId>,
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Policy {
    pub policy_id: PolicyId,
    pub name: String,
    pub template: PolicyTemplate,
    pub version: u32,
    pub scope: PolicyScope,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyTemplate {
    pub schedule: Schedule,
    pub retention: RetentionPolicy,
    pub encryption: EncryptionProfile,
    pub rate_limit: RateLimit,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PolicyScope {
    Device(DeviceId),
    Group(OrganizationId),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditLog {
    pub log_id: Uuid,
    pub actor_id: String,
    pub actor_type: ActorType,
    pub action: AuditAction,
    pub target_type: String,
    pub target_id: String,
    pub result: AuditResult,
    pub timestamp: DateTime<Utc>,
    pub trace_id: Option<String>,
    pub detail: serde_json::Value,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ActorType {
    User,
    Agent,
    System,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuditAction {
    Backup,
    Restore,
    DeleteVersion,
    PolicyChange,
    PermissionChange,
    Login,
    Logout,
    DeviceRegister,
    DeviceDisable,
    Upgrade,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuditResult {
    Success,
    Failed,
    Denied,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Alert {
    pub alert_id: Uuid,
    pub rule_id: String,
    pub severity: AlertSeverity,
    pub device_id: Option<DeviceId>,
    pub message: String,
    pub triggered_at: DateTime<Utc>,
    pub suppressed: bool,
    pub acknowledged: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AlertSeverity {
    Info,
    Warning,
    Error,
    Critical,
}