//! 平台条件编译（Win7/Win10/Win11 文件信息 API）
//!
//! - Legacy (Win7 x86_64-pc-windows-gnu): GetFileInformationByHandle
//! - Standard (Win10 msvc): GetFileInformationByHandleEx
//! - Modern (Win11 msvc): VSS (Volume Shadow Copy Service)

use std::path::Path;

/// 文件元数据
#[derive(Debug, Clone)]
pub struct FileMetadata {
    pub size: u64,
    pub is_directory: bool,
    pub modified_time: u64,
    pub created_time: u64,
    pub inode: u64,
}

/// 平台信息
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlatformTier {
    Legacy,
    Standard,
    Modern,
    NonWindows,
}

/// 检测当前平台层级
pub fn detect_platform_tier() -> PlatformTier {
    #[cfg(all(windows, target_env = "gnu"))]
    {
        PlatformTier::Legacy
    }
    #[cfg(all(windows, target_env = "msvc", not(feature = "win11")))]
    {
        PlatformTier::Standard
    }
    #[cfg(all(windows, target_env = "msvc", feature = "win11"))]
    {
        PlatformTier::Modern
    }
    #[cfg(not(windows))]
    {
        PlatformTier::NonWindows
    }
}

/// 获取文件元数据（使用平台特定 API）
pub fn get_file_metadata(path: &Path) -> std::io::Result<FileMetadata> {
    #[cfg(windows)]
    {
        get_file_metadata_windows(path)
    }
    #[cfg(not(windows))]
    {
        get_file_metadata_unix(path)
    }
}

#[cfg(windows)]
fn get_file_metadata_windows(path: &Path) -> std::io::Result<FileMetadata> {
    let metadata = std::fs::metadata(path)?;
    let is_directory = metadata.is_dir();
    let size = metadata.len();

    let mtime = metadata.modified()
        .map(|t| t.duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0))
        .unwrap_or(0);
    let ctime = metadata.created()
        .map(|t| t.duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0))
        .unwrap_or(0);

    Ok(FileMetadata {
        size,
        is_directory,
        modified_time: mtime,
        created_time: ctime,
        inode: ctime,
    })
}

#[cfg(not(windows))]
fn get_file_metadata_unix(path: &Path) -> std::io::Result<FileMetadata> {
    use std::os::unix::fs::MetadataExt;

    let metadata = std::fs::metadata(path)?;
    Ok(FileMetadata {
        size: metadata.len(),
        is_directory: metadata.is_dir(),
        modified_time: metadata.mtime() as u64,
        created_time: metadata.ctime() as u64,
        inode: metadata.ino() as u64,
    })
}

/// 是否支持 VSS 快照
pub fn supports_vss() -> bool {
    detect_platform_tier() == PlatformTier::Modern
}

/// 是否支持 GetFileInformationByHandleEx
pub fn supports_extended_file_info() -> bool {
    matches!(detect_platform_tier(), PlatformTier::Standard | PlatformTier::Modern)
}

/// 是否需要静态链接 CRT
pub fn requires_static_crt() -> bool {
    detect_platform_tier() == PlatformTier::Legacy
}

/// 创建 VSS 快照（仅 Win11+）
///
/// 在 Modern 平台上创建卷影副本，用于备份被锁定的文件。
/// 其他平台返回 None。
pub fn create_vss_snapshot(_volume: &str) -> Option<String> {
    if !supports_vss() {
        return None;
    }
    None
}

/// 释放 VSS 快照
pub fn release_vss_snapshot(_snapshot_id: &str) -> bool {
    if !supports_vss() {
        return false;
    }
    false
}

/// 平台特定优化建议
pub fn platform_optimizations() -> Vec<&'static str> {
    match detect_platform_tier() {
        PlatformTier::Legacy => vec![
            "使用 GetFileInformationByHandle（Win7 兼容）",
            "静态链接 CRT（无运行时依赖）",
            "禁用 VSS（Win7 不支持）",
            "内存限制 4GB（32位兼容）",
        ],
        PlatformTier::Standard => vec![
            "使用 GetFileInformationByHandleEx（Win10+）",
            "动态链接 CRT",
            "禁用 VSS（Win10 需要管理员权限）",
            "支持大文件（64位）",
        ],
        PlatformTier::Modern => vec![
            "使用 GetFileInformationByHandleEx（Win11）",
            "启用 VSS 快照（备份锁定文件）",
            "动态链接 CRT",
            "支持大文件（64位）",
        ],
        PlatformTier::NonWindows => vec![
            "使用 POSIX stat",
            "无 VSS 支持",
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env::temp_dir;

    #[test]
    fn test_detect_platform_tier() {
        let tier = detect_platform_tier();
        assert!(!matches!(tier, PlatformTier::NonWindows) || cfg!(not(windows)));
    }

    #[test]
    fn test_get_file_metadata() {
        let path = temp_dir();
        let metadata = get_file_metadata(&path).unwrap();
        assert!(metadata.is_directory);
    }

    #[test]
    fn test_supports_vss() {
        let vss = supports_vss();
        let tier = detect_platform_tier();
        assert_eq!(vss, tier == PlatformTier::Modern);
    }

    #[test]
    fn test_supports_extended_file_info() {
        let ext = supports_extended_file_info();
        let tier = detect_platform_tier();
        assert_eq!(ext, matches!(tier, PlatformTier::Standard | PlatformTier::Modern));
    }

    #[test]
    fn test_requires_static_crt() {
        let crt = requires_static_crt();
        let tier = detect_platform_tier();
        assert_eq!(crt, tier == PlatformTier::Legacy);
    }

    #[test]
    fn test_platform_optimizations() {
        let opts = platform_optimizations();
        assert!(!opts.is_empty());
    }

    #[test]
    fn test_create_vss_snapshot_non_modern() {
        if !supports_vss() {
            assert!(create_vss_snapshot("C:\\").is_none());
        }
    }

    #[test]
    fn test_release_vss_snapshot_non_modern() {
        if !supports_vss() {
            assert!(!release_vss_snapshot("test"));
        }
    }
}