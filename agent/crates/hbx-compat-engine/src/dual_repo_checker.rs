use std::collections::HashMap;


use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;


#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileComparison {
    pub path: String,
    pub native_hash: Option<[u8; 32]>,
    pub compat_hash: Option<[u8; 32]>,
    pub native_size: Option<u64>,
    pub compat_size: Option<u64>,
    pub is_consistent: bool,
}

impl FileComparison {
    pub fn is_missing_in_native(&self) -> bool {
        self.native_hash.is_none()
    }

    pub fn is_missing_in_compat(&self) -> bool {
        self.compat_hash.is_none()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsistencyConclusion {
    pub checked_at: DateTime<Utc>,
    pub total_files_compared: u64,
    pub consistent_files: u64,
    pub inconsistent_files: u64,
    pub missing_in_native: u64,
    pub missing_in_compat: u64,
    pub comparisons: Vec<FileComparison>,
    pub is_consistent: bool,
}

impl ConsistencyConclusion {
    pub fn empty() -> Self {
        Self {
            checked_at: Utc::now(),
            total_files_compared: 0,
            consistent_files: 0,
            inconsistent_files: 0,
            missing_in_native: 0,
            missing_in_compat: 0,
            comparisons: vec![],
            is_consistent: true,
        }
    }
}

#[derive(Debug, Error)]
pub enum DualRepoError {
    #[error("backup failed: {0}")]
    BackupFailed(String),
    #[error("restore failed: {0}")]
    RestoreFailed(String),
    #[error("inconsistent: {0}")]
    Inconsistent(String),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DualRepoMode {
    NativeOnly,
    CompatibleOnly,
    DualWithConsistency,
}

pub trait IDualRepositoryConsistencyChecker {
    fn check_consistency(
        &self,
        native_dir: &std::path::Path,
        compat_dir: &std::path::Path,
    ) -> Result<ConsistencyConclusion, DualRepoError>;
}

pub struct DualRepoConsistencyChecker;

impl DualRepoConsistencyChecker {
    pub fn new() -> Self {
        Self
    }
}

impl Default for DualRepoConsistencyChecker {
    fn default() -> Self {
        Self::new()
    }
}

impl IDualRepositoryConsistencyChecker for DualRepoConsistencyChecker {
    fn check_consistency(
        &self,
        native_dir: &std::path::Path,
        compat_dir: &std::path::Path,
    ) -> Result<ConsistencyConclusion, DualRepoError> {
        let native_files = scan_dir_hashes(native_dir)?;
        let compat_files = scan_dir_hashes(compat_dir)?;

        let mut all_paths: std::collections::HashSet<String> = std::collections::HashSet::new();
        all_paths.extend(native_files.keys().cloned());
        all_paths.extend(compat_files.keys().cloned());

        let mut comparisons = Vec::new();
        let mut consistent_files: u64 = 0;
        let mut inconsistent_files: u64 = 0;
        let mut missing_in_native: u64 = 0;
        let mut missing_in_compat: u64 = 0;

        for path in &all_paths {
            let native = native_files.get(path);
            let compat = compat_files.get(path);

            let native_hash = native.map(|(h, _)| *h);
            let compat_hash = compat.map(|(h, _)| *h);
            let native_size = native.map(|(_, s)| *s);
            let compat_size = compat.map(|(_, s)| *s);

            let is_consistent = native_hash.is_some()
                && compat_hash.is_some()
                && native_hash == compat_hash
                && native_size == compat_size;

            if native_hash.is_none() {
                missing_in_native += 1;
            }
            if compat_hash.is_none() {
                missing_in_compat += 1;
            }
            if is_consistent {
                consistent_files += 1;
            } else {
                inconsistent_files += 1;
            }

            comparisons.push(FileComparison {
                path: path.clone(),
                native_hash,
                compat_hash,
                native_size,
                compat_size,
                is_consistent,
            });
        }

        let is_consistent = inconsistent_files == 0;

        Ok(ConsistencyConclusion {
            checked_at: Utc::now(),
            total_files_compared: all_paths.len() as u64,
            consistent_files,
            inconsistent_files,
            missing_in_native,
            missing_in_compat,
            comparisons,
            is_consistent,
        })
    }
}

fn scan_dir_hashes(
    dir: &std::path::Path,
) -> Result<HashMap<String, ([u8; 32], u64)>, DualRepoError> {
    let mut result = HashMap::new();
    if !dir.exists() {
        return Ok(result);
    }
    scan_dir_recursive(dir, dir, &mut result)?;
    Ok(result)
}

fn scan_dir_recursive(
    base: &std::path::Path,
    current: &std::path::Path,
    result: &mut HashMap<String, ([u8; 32], u64)>,
) -> Result<(), DualRepoError> {
    for entry in std::fs::read_dir(current)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            scan_dir_recursive(base, &path, result)?;
        } else if path.is_file() {
            let rel = path.strip_prefix(base).unwrap_or(&path);
            let rel_str = rel.to_string_lossy().to_string();

            let data = std::fs::read(&path)?;
            let size = data.len() as u64;
            let mut hasher = Sha256::new();
            hasher.update(&data);
            let hash: [u8; 32] = hasher.finalize().into();

            result.insert(rel_str, (hash, size));
        }
    }
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DualRepoInconsistentEvent {
    pub timestamp: DateTime<Utc>,
    pub conclusion: ConsistencyConclusion,
    pub message: String,
}

impl DualRepoInconsistentEvent {
    pub fn new(conclusion: ConsistencyConclusion) -> Self {
        let message = format!(
            "dual repo inconsistency detected: {} of {} files inconsistent",
            conclusion.inconsistent_files, conclusion.total_files_compared
        );
        Self {
            timestamp: Utc::now(),
            conclusion,
            message,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn write_test_file(dir: &std::path::Path, name: &str, content: &[u8]) {
        fs::write(dir.join(name), content).unwrap();
    }

    #[test]
    fn test_consistent_dirs() {
        let native_dir = tempfile::tempdir().unwrap();
        let compat_dir = tempfile::tempdir().unwrap();

        write_test_file(native_dir.path(), "file1.txt", b"hello");
        write_test_file(native_dir.path(), "file2.txt", b"world");
        write_test_file(compat_dir.path(), "file1.txt", b"hello");
        write_test_file(compat_dir.path(), "file2.txt", b"world");

        let checker = DualRepoConsistencyChecker::new();
        let conclusion = checker
            .check_consistency(native_dir.path(), compat_dir.path())
            .unwrap();

        assert!(conclusion.is_consistent);
        assert_eq!(conclusion.total_files_compared, 2);
        assert_eq!(conclusion.consistent_files, 2);
        assert_eq!(conclusion.inconsistent_files, 0);
    }

    #[test]
    fn test_inconsistent_dirs() {
        let native_dir = tempfile::tempdir().unwrap();
        let compat_dir = tempfile::tempdir().unwrap();

        write_test_file(native_dir.path(), "file1.txt", b"hello");
        write_test_file(compat_dir.path(), "file1.txt", b"HELLO");

        let checker = DualRepoConsistencyChecker::new();
        let conclusion = checker
            .check_consistency(native_dir.path(), compat_dir.path())
            .unwrap();

        assert!(!conclusion.is_consistent);
        assert_eq!(conclusion.inconsistent_files, 1);
    }

    #[test]
    fn test_missing_in_compat() {
        let native_dir = tempfile::tempdir().unwrap();
        let compat_dir = tempfile::tempdir().unwrap();

        write_test_file(native_dir.path(), "file1.txt", b"hello");
        write_test_file(native_dir.path(), "file2.txt", b"world");
        write_test_file(compat_dir.path(), "file1.txt", b"hello");

        let checker = DualRepoConsistencyChecker::new();
        let conclusion = checker
            .check_consistency(native_dir.path(), compat_dir.path())
            .unwrap();

        assert!(!conclusion.is_consistent);
        assert_eq!(conclusion.missing_in_compat, 1);
        assert_eq!(conclusion.consistent_files, 1);
    }

    #[test]
    fn test_missing_in_native() {
        let native_dir = tempfile::tempdir().unwrap();
        let compat_dir = tempfile::tempdir().unwrap();

        write_test_file(native_dir.path(), "file1.txt", b"hello");
        write_test_file(compat_dir.path(), "file1.txt", b"hello");
        write_test_file(compat_dir.path(), "file2.txt", b"world");

        let checker = DualRepoConsistencyChecker::new();
        let conclusion = checker
            .check_consistency(native_dir.path(), compat_dir.path())
            .unwrap();

        assert!(!conclusion.is_consistent);
        assert_eq!(conclusion.missing_in_native, 1);
    }

    #[test]
    fn test_empty_dirs() {
        let native_dir = tempfile::tempdir().unwrap();
        let compat_dir = tempfile::tempdir().unwrap();

        let checker = DualRepoConsistencyChecker::new();
        let conclusion = checker
            .check_consistency(native_dir.path(), compat_dir.path())
            .unwrap();

        assert!(conclusion.is_consistent);
        assert_eq!(conclusion.total_files_compared, 0);
    }

    #[test]
    fn test_nested_directories() {
        let native_dir = tempfile::tempdir().unwrap();
        let compat_dir = tempfile::tempdir().unwrap();

        fs::create_dir_all(native_dir.path().join("sub")).unwrap();
        fs::create_dir_all(compat_dir.path().join("sub")).unwrap();
        write_test_file(native_dir.path().join("sub").as_path(), "file.txt", b"nested");
        write_test_file(compat_dir.path().join("sub").as_path(), "file.txt", b"nested");

        let checker = DualRepoConsistencyChecker::new();
        let conclusion = checker
            .check_consistency(native_dir.path(), compat_dir.path())
            .unwrap();

        assert!(conclusion.is_consistent);
        assert_eq!(conclusion.total_files_compared, 1);
    }

    #[test]
    fn test_inconsistent_event() {
        let conclusion = ConsistencyConclusion {
            checked_at: Utc::now(),
            total_files_compared: 10,
            consistent_files: 8,
            inconsistent_files: 2,
            missing_in_native: 1,
            missing_in_compat: 1,
            comparisons: vec![],
            is_consistent: false,
        };
        let event = DualRepoInconsistentEvent::new(conclusion);
        assert!(event.message.contains("2 of 10 files inconsistent"));
    }

    #[test]
    fn test_dual_repo_mode() {
        assert_eq!(DualRepoMode::NativeOnly, DualRepoMode::NativeOnly);
        assert_eq!(DualRepoMode::CompatibleOnly, DualRepoMode::CompatibleOnly);
        assert_eq!(
            DualRepoMode::DualWithConsistency,
            DualRepoMode::DualWithConsistency
        );
    }
}