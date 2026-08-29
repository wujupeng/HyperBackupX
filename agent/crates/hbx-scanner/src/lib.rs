﻿pub mod diff;

pub use diff::{FileTreeDiff, RenamedFile};

use futures::stream::Stream;
use hbx_core::domain::backup::{BackupSnapshot, BackupSource};
use hbx_core::domain::common::{FileAttributes, FilterRule, ScanEstimate};
use hbx_core::domain::repository::FileEntry;
use hbx_core::pipeline::{IScanner, ScanError};
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use walkdir::WalkDir;

pub struct LocalScanner {
    #[allow(dead_code)]
    scan_threads: usize,
}

impl LocalScanner {
    pub fn new() -> Self {
        Self { scan_threads: 4 }
    }

    pub fn with_threads(scan_threads: usize) -> Self {
        Self {
            scan_threads: scan_threads.max(1),
        }
    }

    pub fn scan_with_diff(
        &self,
        source: &BackupSource,
        filter: &FilterRule,
        baseline: Option<&BackupSnapshot>,
    ) -> Result<
        (
            Box<dyn Stream<Item = FileEntry> + Send + Unpin>,
            Option<FileTreeDiff>,
        ),
        ScanError,
    > {
        let mut all_files: Vec<FileEntry> = Vec::new();

        for base_path in &source.paths {
            if !base_path.exists() {
                tracing::warn!("source path does not exist: {:?}", base_path);
                continue;
            }

            for entry in WalkDir::new(base_path)
                .follow_links(false)
                .into_iter()
                .filter_map(|e| e.ok())
            {
                if !entry.file_type().is_file() {
                    continue;
                }

                let path = entry.path();
                let path_str = path.to_string_lossy().to_string();

                if !passes_filters(&path_str, &source.include_rules, &source.exclude_rules, filter) {
                    continue;
                }

                let metadata = match entry.metadata() {
                    Ok(m) => m,
                    Err(e) => {
                        tracing::warn!("metadata error for {:?}: {}", path, e);
                        continue;
                    }
                };

                let modified_at = metadata.modified().map(|t| {
                    let duration = t
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default();
                    chrono::DateTime::from_timestamp(duration.as_secs() as i64, 0)
                        .unwrap_or_default()
                }).unwrap_or_default();

                all_files.push(FileEntry {
                    path: path_str,
                    size: metadata.len(),
                    modified_at,
                    attributes: FileAttributes::default(),
                    chunks: Vec::new(),
                    file_hash: [0u8; 32],
                });
            }
        }

        let (diff_result, files_to_send) = if let Some(ref baseline) = baseline {
            let diff = FileTreeDiff::compute(all_files.into_iter(), baseline);
            let files: Vec<FileEntry> = diff.added.iter().chain(diff.modified.iter()).cloned().collect();
            (Some(diff), files)
        } else {
            (None, all_files)
        };

        let (tx, rx) = mpsc::channel::<FileEntry>(64);

        tokio::spawn(async move {
            for file_entry in files_to_send {
                if tx.send(file_entry).await.is_err() {
                    return;
                }
            }
        });

        Ok((Box::new(ReceiverStream::new(rx)), diff_result))
    }
}

impl Default for LocalScanner {
    fn default() -> Self {
        Self::new()
    }
}

impl IScanner for LocalScanner {
    fn scan(
        &self,
        source: &BackupSource,
        filter: &FilterRule,
        baseline: Option<&BackupSnapshot>,
    ) -> Result<Box<dyn Stream<Item = FileEntry> + Send + Unpin>, ScanError> {
        let (stream, _diff) = self.scan_with_diff(source, filter, baseline)?;
        Ok(stream)
    }

    fn estimate(&self, source: &BackupSource, filter: &FilterRule) -> ScanEstimate {
        let mut total_files = 0u64;
        let mut total_size = 0u64;

        for base_path in &source.paths {
            if !base_path.exists() {
                continue;
            }

            for entry in WalkDir::new(base_path)
                .follow_links(false)
                .into_iter()
                .filter_map(|e| e.ok())
            {
                if !entry.file_type().is_file() {
                    continue;
                }

                let path_str = entry.path().to_string_lossy().to_string();
                if !passes_filters(&path_str, &source.include_rules, &source.exclude_rules, filter) {
                    continue;
                }

                total_files += 1;
                if let Ok(metadata) = entry.metadata() {
                    total_size += metadata.len();
                }
            }
        }

        ScanEstimate {
            total_files,
            total_size,
        }
    }
}

fn passes_filters(
    path: &str,
    include_rules: &[FilterRule],
    exclude_rules: &[FilterRule],
    _global_filter: &FilterRule,
) -> bool {
    for rule in exclude_rules {
        if matches_rule(path, rule) {
            return false;
        }
    }

    if include_rules.is_empty() {
        return true;
    }

    include_rules.iter().any(|rule| matches_rule(path, rule))
}

fn matches_rule(path: &str, rule: &FilterRule) -> bool {
    match rule {
        FilterRule::Glob(pattern) => glob_match(pattern, path),
        FilterRule::Regex(pattern) => {
            regex::Regex::new(pattern)
                .map(|re| re.is_match(path))
                .unwrap_or(false)
        }
        FilterRule::PathPrefix(prefix) => path.starts_with(prefix),
    }
}

fn glob_match(pattern: &str, text: &str) -> bool {
    let pat: Vec<char> = pattern.chars().collect();
    let txt: Vec<char> = text.chars().collect();
    glob_match_helper(&pat, 0, &txt, 0)
}

fn glob_match_helper(pat: &[char], pi: usize, txt: &[char], ti: usize) -> bool {
    if pi == pat.len() {
        return ti == txt.len();
    }

    if pat[pi] == '*' {
        if pi + 1 == pat.len() {
            return true;
        }
        for next in ti..=txt.len() {
            if glob_match_helper(pat, pi + 1, txt, next) {
                return true;
            }
        }
        return false;
    }

    if ti < txt.len() && (pat[pi] == '?' || pat[pi] == txt[ti]) {
        return glob_match_helper(pat, pi + 1, txt, ti + 1);
    }

    false
}


#[cfg(test)]
mod tests {
    use super::*;
    use futures::StreamExt;
    use hbx_core::domain::backup::BackupSource;
    use hbx_core::domain::common::FilterRule;
    use hbx_core::pipeline::IScanner;
    use std::fs;


    #[tokio::test]
    async fn test_scan_basic() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();

        fs::write(dir.join("a.txt"), "hello").unwrap();
        fs::write(dir.join("b.txt"), "world").unwrap();
        fs::create_dir(dir.join("sub")).unwrap();
        fs::write(dir.join("sub").join("c.txt"), "world").unwrap();

        let source = BackupSource {
            paths: vec![dir.to_path_buf()],
            include_rules: vec![],
            exclude_rules: vec![],
        };

        let scanner = LocalScanner::new();
        let filter = FilterRule::Glob("*".to_string());
        let stream = scanner.scan(&source, &filter, None).unwrap();

        let entries: Vec<FileEntry> = stream.collect().await;
        assert_eq!(entries.len(), 3);

        let paths: Vec<&str> = entries.iter().map(|e| e.path.as_str()).collect();
        assert!(paths.iter().any(|p| p.ends_with("a.txt")));
        assert!(paths.iter().any(|p| p.ends_with("b.txt")));
        assert!(paths.iter().any(|p| p.ends_with("c.txt")));
    }

    #[tokio::test]
    async fn test_scan_with_exclude() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();

        fs::write(dir.join("keep.txt"), "data").unwrap();
        fs::write(dir.join("skip.tmp"), "temp").unwrap();

        let source = BackupSource {
            paths: vec![dir.to_path_buf()],
            include_rules: vec![],
            exclude_rules: vec![FilterRule::Glob("*.tmp".to_string())],
        };

        let scanner = LocalScanner::new();
        let filter = FilterRule::Glob("*".to_string());
        let stream = scanner.scan(&source, &filter, None).unwrap();

        let entries: Vec<FileEntry> = stream.collect().await;
        assert_eq!(entries.len(), 1);
        assert!(entries[0].path.ends_with("keep.txt"));
    }

    #[tokio::test]
    async fn test_scan_empty_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let source = BackupSource {
            paths: vec![tmp.path().to_path_buf()],
            include_rules: vec![],
            exclude_rules: vec![],
        };

        let scanner = LocalScanner::new();
        let filter = FilterRule::Glob("*".to_string());
        let stream = scanner.scan(&source, &filter, None).unwrap();

        let entries: Vec<FileEntry> = stream.collect().await;
        assert!(entries.is_empty());
    }

    #[test]
    fn test_estimate() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();

        fs::write(dir.join("a.txt"), "hello world").unwrap();
        fs::write(dir.join("b.txt"), "foo bar baz").unwrap();

        let source = BackupSource {
            paths: vec![dir.to_path_buf()],
            include_rules: vec![],
            exclude_rules: vec![],
        };

        let scanner = LocalScanner::new();
        let filter = FilterRule::Glob("*".to_string());
        let estimate = scanner.estimate(&source, &filter);

        assert_eq!(estimate.total_files, 2);
        assert!(estimate.total_size > 0);
    }

    #[test]
    fn test_glob_match() {
        assert!(glob_match("*.txt", "hello.txt"));
        assert!(!glob_match("*.txt", "hello.rs"));
        assert!(glob_match("a*b", "axxxb"));
        assert!(glob_match("?", "x"));
        assert!(!glob_match("?", "xy"));
        assert!(glob_match("*", "anything"));
    }
}
