use std::path::PathBuf;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::common::{DeviceId, OrganizationId};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Device {
    pub device_id: DeviceId,
    pub hostname: String,
    pub os_type: OsType,
    pub hardware_profile: HardwareProfile,
    pub agent_version: String,
    pub status: DeviceStatus,
    pub organization_id: OrganizationId,
    pub last_heartbeat_at: Option<DateTime<Utc>>,
    pub registered_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OsType {
    Windows7,
    Windows10,
    Windows11,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DeviceStatus {
    Online,
    Offline,
    Disabled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HardwareProfile {
    pub total_memory_mb: u64,
    pub cpu_cores: u32,
    pub disk_type: DiskType,
    pub tier: HardwareTier,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HardwareTier {
    Legacy,
    Standard,
    Modern,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DiskType {
    Hdd,
    Ssd,
    Nvme,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Journal {
    pub path: PathBuf,
    pub rotation_size: u64,
    pub current_offset: u64,
}