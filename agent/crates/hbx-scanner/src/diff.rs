use std::collections::HashMap;

use hbx_core::domain::backup::{BackupSnapshot, FileSnapshotEntry};
use hbx_core::domain::repository::FileEntry;

#[derive(Debug, Clone)]
pub struct RenamedFile {
    pub old_path: String,
    pub new_path: String,
}

#[derive(Debug, Clone, Default)]
pub struct FileTreeDiff {
    pub added: Vec<FileEntry>,
    pub modified: Vec<FileEntry>,
    pub deleted: Vec<String>,
    pub renamed: Vec<RenamedFile>,
}

impl FileTreeDiff {
    pub fn compute(current: impl Iterator<Item = FileEntry>, baseline: &BackupSnapshot) -> FileTreeDiff {
        let baseline_map: HashMap<&str, &FileSnapshotEntry> = baseline
            .files
            .iter()
            .map(|f| (f.path.as_str(), f))
            .collect();

        let mut potentially_added: Vec<FileEntry> = Vec::new();
        let mut modified = Vec::new();
        let mut current_entries: HashMap<String, FileEntry> = HashMap::new();

        for entry in current {
            let is_changed = match baseline_map.get(entry.path.as_str()) {
                Some(base) => entry.size != base.size || entry.modified_at != base.mtime,
                None => true,
            };

            if is_changed {
                if baseline_map.contains_key(entry.path.as_str()) {
                    modified.push(entry.clone());
                } else {
                    potentially_added.push(entry.clone());
                }
            }

            current_entries.insert(entry.path.clone(), entry);
        }

        let mut deleted = Vec::new();
        let mut renamed = Vec::new();
        let mut matched_new_paths: std::collections::HashSet<String> = std::collections::HashSet::new();

        for base_file in &baseline.files {
            if current_entries.contains_key(&base_file.path) {
                continue;
            }

            let rename_candidate = potentially_added.iter().find(|curr| {
                !matched_new_paths.contains(&curr.path)
                    && curr.size == base_file.size
                    && curr.modified_at == base_file.mtime
            });

            if let Some(curr) = rename_candidate {
                renamed.push(RenamedFile {
                    old_path: base_file.path.clone(),
                    new_path: curr.path.clone(),
                });
                matched_new_paths.insert(curr.path.clone());
            } else {
                deleted.push(base_file.path.clone());
            }
        }

        let added: Vec<FileEntry> = potentially_added
            .into_iter()
            .filter(|e| !matched_new_paths.contains(&e.path))
            .collect();

        FileTreeDiff {
            added,
            modified,
            deleted,
            renamed,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.added.is_empty()
            && self.modified.is_empty()
            && self.deleted.is_empty()
            && self.renamed.is_empty()
    }

    pub fn changed_files(&self) -> Vec<&FileEntry> {
        let mut result = Vec::with_capacity(self.added.len() + self.modified.len());
        result.extend(self.added.iter());
        result.extend(self.modified.iter());
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use hbx_core::domain::common::{FileAttributes, VersionId};

    fn make_file_entry(path: &str, size: u64, mtime: chrono::DateTime<Utc>) -> FileEntry {
        FileEntry {
            path: path.to_string(),
            size,
            modified_at: mtime,
            attributes: FileAttributes::default(),
            chunks: Vec::new(),
            file_hash: [0u8; 32],
        }
    }

    fn make_snapshot(files: Vec<FileSnapshotEntry>) -> BackupSnapshot {
        BackupSnapshot {
            version_id: VersionId(uuid::Uuid::new_v4()),
            timestamp: Utc::now(),
            files,
        }
    }

    fn make_snapshot_entry(path: &str, size: u64, mtime: chrono::DateTime<Utc>) -> FileSnapshotEntry {
        FileSnapshotEntry {
            path: path.to_string(),
            size,
            mtime,
            file_hash: [0u8; 32],
        }
    }

    fn fixed_time() -> chrono::DateTime<Utc> {
        chrono::DateTime::from_timestamp(1700000000, 0).unwrap()
    }

    #[test]
    fn test_empty_baseline_all_added() {
        let baseline = make_snapshot(vec![]);
        let current = vec![
            make_file_entry("/data/a.txt", 100, fixed_time()),
            make_file_entry("/data/b.txt", 200, fixed_time()),
        ];

        let diff = FileTreeDiff::compute(current.into_iter(), &baseline);

        assert_eq!(diff.added.len(), 2);
        assert_eq!(diff.modified.len(), 0);
        assert_eq!(diff.deleted.len(), 0);
        assert_eq!(diff.renamed.len(), 0);
    }

    #[test]
    fn test_no_changes_empty_diff() {
        let t = fixed_time();
        let baseline = make_snapshot(vec![
            make_snapshot_entry("/data/a.txt", 100, t),
            make_snapshot_entry("/data/b.txt", 200, t),
        ]);
        let current = vec![
            make_file_entry("/data/a.txt", 100, t),
            make_file_entry("/data/b.txt", 200, t),
        ];

        let diff = FileTreeDiff::compute(current.into_iter(), &baseline);

        assert!(diff.is_empty());
    }

    #[test]
    fn test_modified_file() {
        let t = fixed_time();
        let t2 = t + chrono::Duration::seconds(60);
        let baseline = make_snapshot(vec![
            make_snapshot_entry("/data/a.txt", 100, t),
            make_snapshot_entry("/data/b.txt", 200, t),
        ]);
        let current = vec![
            make_file_entry("/data/a.txt", 100, t),
            make_file_entry("/data/b.txt", 250, t2),
        ];

        let diff = FileTreeDiff::compute(current.into_iter(), &baseline);

        assert_eq!(diff.added.len(), 0);
        assert_eq!(diff.modified.len(), 1);
        assert_eq!(diff.modified[0].path, "/data/b.txt");
        assert_eq!(diff.deleted.len(), 0);
        assert_eq!(diff.renamed.len(), 0);
    }

    #[test]
    fn test_deleted_file() {
        let t = fixed_time();
        let baseline = make_snapshot(vec![
            make_snapshot_entry("/data/a.txt", 100, t),
            make_snapshot_entry("/data/b.txt", 200, t),
        ]);
        let current = vec![
            make_file_entry("/data/a.txt", 100, t),
        ];

        let diff = FileTreeDiff::compute(current.into_iter(), &baseline);

        assert_eq!(diff.deleted.len(), 1);
        assert_eq!(diff.deleted[0], "/data/b.txt");
        assert_eq!(diff.added.len(), 0);
        assert_eq!(diff.modified.len(), 0);
        assert_eq!(diff.renamed.len(), 0);
    }

    #[test]
    fn test_renamed_file() {
        let t = fixed_time();
        let baseline = make_snapshot(vec![
            make_snapshot_entry("/data/old.txt", 100, t),
        ]);
        let current = vec![
            make_file_entry("/data/new.txt", 100, t),
        ];

        let diff = FileTreeDiff::compute(current.into_iter(), &baseline);

        assert_eq!(diff.renamed.len(), 1);
        assert_eq!(diff.renamed[0].old_path, "/data/old.txt");
        assert_eq!(diff.renamed[0].new_path, "/data/new.txt");
        assert_eq!(diff.added.len(), 0);
        assert_eq!(diff.deleted.len(), 0);
        assert_eq!(diff.modified.len(), 0);
    }

    #[test]
    fn test_mixed_changes() {
        let t = fixed_time();
        let t2 = t + chrono::Duration::seconds(120);
        let baseline = make_snapshot(vec![
            make_snapshot_entry("/data/keep.txt", 100, t),
            make_snapshot_entry("/data/modify.txt", 200, t),
            make_snapshot_entry("/data/delete.txt", 300, t),
            make_snapshot_entry("/data/rename.txt", 400, t),
        ]);
        let current = vec![
            make_file_entry("/data/keep.txt", 100, t),
            make_file_entry("/data/modify.txt", 250, t2),
            make_file_entry("/data/renamed.txt", 400, t),
            make_file_entry("/data/new.txt", 500, t),
        ];

        let diff = FileTreeDiff::compute(current.into_iter(), &baseline);

        assert_eq!(diff.added.len(), 1);
        assert_eq!(diff.added[0].path, "/data/new.txt");
        assert_eq!(diff.modified.len(), 1);
        assert_eq!(diff.modified[0].path, "/data/modify.txt");
        assert_eq!(diff.deleted.len(), 1);
        assert_eq!(diff.deleted[0], "/data/delete.txt");
        assert_eq!(diff.renamed.len(), 1);
        assert_eq!(diff.renamed[0].old_path, "/data/rename.txt");
        assert_eq!(diff.renamed[0].new_path, "/data/renamed.txt");
    }

    #[test]
    fn test_changed_files_combines_added_and_modified() {
        let t = fixed_time();
        let t2 = t + chrono::Duration::seconds(60);
        let baseline = make_snapshot(vec![
            make_snapshot_entry("/data/a.txt", 100, t),
        ]);
        let current = vec![
            make_file_entry("/data/a.txt", 150, t2),
            make_file_entry("/data/b.txt", 200, t),
        ];

        let diff = FileTreeDiff::compute(current.into_iter(), &baseline);

        let changed = diff.changed_files();
        assert_eq!(changed.len(), 2);
    }
}