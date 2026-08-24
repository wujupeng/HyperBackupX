use std::time::Duration;

use hbx_core::domain::device::{DiskType, HardwareProfile, HardwareTier};
use sysinfo::System;

pub mod platform;
pub use platform::{
    FileMetadata, PlatformTier, detect_platform_tier, get_file_metadata,
    supports_vss, supports_extended_file_info, requires_static_crt,
    create_vss_snapshot, release_vss_snapshot, platform_optimizations,
};

const LEGACY_MEMORY_THRESHOLD_MB: u64 = 4096;
const STANDARD_MEMORY_THRESHOLD_MB: u64 = 8192;
const REPROBE_INTERVAL: Duration = Duration::from_secs(300);

pub struct HardwareProbe {
    last_profile: Option<HardwareProfile>,
    last_probe_time: Option<std::time::Instant>,
}

impl HardwareProbe {
    pub fn new() -> Self {
        Self {
            last_profile: None,
            last_probe_time: None,
        }
    }

    pub fn probe(&mut self) -> HardwareProfile {
        let mut sys = System::new_all();
        sys.refresh_memory();
        sys.refresh_cpu_usage();

        let total_memory_mb = sys.total_memory() / 1024;
        let cpu_cores = sys.cpus().len() as u32;
        let disk_type = detect_disk_type();
        let tier = classify_tier(total_memory_mb);

        let profile = HardwareProfile {
            total_memory_mb,
            cpu_cores,
            disk_type,
            tier,
        };

        self.last_profile = Some(profile.clone());
        self.last_probe_time = Some(std::time::Instant::now());
        profile
    }

    pub fn probe_if_stale(&mut self) -> HardwareProfile {
        let need_reprobe = match (self.last_probe_time, self.last_profile.as_ref()) {
            (Some(last_time), Some(_profile)) => last_time.elapsed() >= REPROBE_INTERVAL,
            _ => true,
        };

        if need_reprobe {
            self.probe()
        } else {
            self.last_profile.clone().unwrap()
        }
    }

    pub fn current(&self) -> Option<&HardwareProfile> {
        self.last_profile.as_ref()
    }
}

impl Default for HardwareProbe {
    fn default() -> Self {
        Self::new()
    }
}

pub fn classify_tier(total_memory_mb: u64) -> HardwareTier {
    if total_memory_mb <= LEGACY_MEMORY_THRESHOLD_MB {
        HardwareTier::Legacy
    } else if total_memory_mb <= STANDARD_MEMORY_THRESHOLD_MB {
        HardwareTier::Standard
    } else {
        HardwareTier::Modern
    }
}

pub fn chunk_size_for_tier(tier: HardwareTier) -> u64 {
    match tier {
        HardwareTier::Legacy => 1024 * 1024,
        HardwareTier::Standard => 4 * 1024 * 1024,
        HardwareTier::Modern => 8 * 1024 * 1024,
    }
}

pub fn memory_budget_mb_for_tier(tier: HardwareTier) -> u64 {
    match tier {
        HardwareTier::Legacy => 32,
        HardwareTier::Standard => 128,
        HardwareTier::Modern => 512,
    }
}

pub fn parallelism_for_tier(tier: HardwareTier, cpu_cores: u32) -> u32 {
    match tier {
        HardwareTier::Legacy => 1,
        HardwareTier::Standard => 2,
        HardwareTier::Modern => (cpu_cores / 2).max(1),
    }
}

pub fn dedup_cache_capacity_for_tier(tier: HardwareTier) -> usize {
    match tier {
        HardwareTier::Legacy => 32_000,
        HardwareTier::Standard => 128_000,
        HardwareTier::Modern => 512_000,
    }
}

fn detect_disk_type() -> DiskType {
    #[cfg(target_os = "windows")]
    {
        DiskType::Ssd
    }
    #[cfg(not(target_os = "windows"))]
    {
        DiskType::Ssd
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_classify_tier() {
        assert_eq!(classify_tier(2048), HardwareTier::Legacy);
        assert_eq!(classify_tier(4096), HardwareTier::Legacy);
        assert_eq!(classify_tier(4097), HardwareTier::Standard);
        assert_eq!(classify_tier(8192), HardwareTier::Standard);
        assert_eq!(classify_tier(8193), HardwareTier::Modern);
        assert_eq!(classify_tier(16384), HardwareTier::Modern);
    }

    #[test]
    fn test_chunk_size_for_tier() {
        assert_eq!(chunk_size_for_tier(HardwareTier::Legacy), 1 * 1024 * 1024);
        assert_eq!(chunk_size_for_tier(HardwareTier::Standard), 4 * 1024 * 1024);
        assert_eq!(chunk_size_for_tier(HardwareTier::Modern), 8 * 1024 * 1024);
    }

    #[test]
    fn test_memory_budget_for_tier() {
        assert_eq!(memory_budget_mb_for_tier(HardwareTier::Legacy), 32);
        assert_eq!(memory_budget_mb_for_tier(HardwareTier::Standard), 128);
        assert_eq!(memory_budget_mb_for_tier(HardwareTier::Modern), 512);
    }

    #[test]
    fn test_parallelism_for_tier() {
        assert_eq!(parallelism_for_tier(HardwareTier::Legacy, 8), 1);
        assert_eq!(parallelism_for_tier(HardwareTier::Standard, 8), 2);
        assert_eq!(parallelism_for_tier(HardwareTier::Modern, 8), 4);
        assert_eq!(parallelism_for_tier(HardwareTier::Modern, 2), 1);
    }

    #[test]
    fn test_probe_returns_valid_profile() {
        let mut probe = HardwareProbe::new();
        let profile = probe.probe();
        assert!(profile.total_memory_mb > 0);
        assert!(profile.cpu_cores > 0);
    }

    #[test]
    fn test_probe_if_stale_caches() {
        let mut probe = HardwareProbe::new();
        let p1 = probe.probe();
        let p2 = probe.probe_if_stale();
        assert_eq!(p1.total_memory_mb, p2.total_memory_mb);
    }
}
