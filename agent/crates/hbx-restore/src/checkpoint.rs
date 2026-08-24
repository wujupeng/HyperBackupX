use std::collections::HashSet;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RestoreCheckpoint {
    pub restore_id: String,
    pub restored_files: HashSet<String>,
    pub total_files: usize,
    pub completed: bool,
}

impl RestoreCheckpoint {
    pub fn new(restore_id: &str, total_files: usize) -> Self {
        Self {
            restore_id: restore_id.to_string(),
            restored_files: HashSet::new(),
            total_files,
            completed: false,
        }
    }

    pub fn mark_restored(&mut self, file_path: &str) {
        self.restored_files.insert(file_path.to_string());
    }

    pub fn is_restored(&self, file_path: &str) -> bool {
        self.restored_files.contains(file_path)
    }

    pub fn mark_completed(&mut self) {
        self.completed = true;
    }

    pub fn progress(&self) -> f64 {
        if self.total_files == 0 {
            return 1.0;
        }
        self.restored_files.len() as f64 / self.total_files as f64
    }

    pub fn save(&self, path: &Path) -> Result<(), std::io::Error> {
        let data = serde_json::to_string(self)
            .map_err(std::io::Error::other)?;
        std::fs::write(path, data)
    }

    pub fn load(path: &Path) -> Result<Self, std::io::Error> {
        let data = std::fs::read_to_string(path)?;
        serde_json::from_str(&data)
            .map_err(std::io::Error::other)
    }

    pub fn load_or_create(
        path: &Path,
        restore_id: &str,
        total_files: usize,
    ) -> Result<Self, std::io::Error> {
        if path.exists() {
            Self::load(path)
        } else {
            Ok(Self::new(restore_id, total_files))
        }
    }

    pub fn checkpoint_path(base: &Path, restore_id: &str) -> PathBuf {
        base.join(format!("{}.restore-checkpoint", restore_id))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_checkpoint_new() {
        let cp = RestoreCheckpoint::new("test-restore", 10);
        assert_eq!(cp.restored_files.len(), 0);
        assert_eq!(cp.total_files, 10);
        assert!(!cp.completed);
        assert_eq!(cp.progress(), 0.0);
    }

    #[test]
    fn test_mark_restored() {
        let mut cp = RestoreCheckpoint::new("test", 3);
        cp.mark_restored("a.txt");
        cp.mark_restored("b.txt");

        assert!(cp.is_restored("a.txt"));
        assert!(cp.is_restored("b.txt"));
        assert!(!cp.is_restored("c.txt"));
        assert_eq!(cp.progress(), 2.0 / 3.0);
    }

    #[test]
    fn test_mark_completed() {
        let mut cp = RestoreCheckpoint::new("test", 1);
        cp.mark_restored("a.txt");
        cp.mark_completed();
        assert!(cp.completed);
    }

    #[test]
    fn test_save_load_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("checkpoint.json");

        let mut cp = RestoreCheckpoint::new("test-restore", 5);
        cp.mark_restored("a.txt");
        cp.mark_restored("b.txt");
        cp.mark_completed();

        cp.save(&path).unwrap();
        let loaded = RestoreCheckpoint::load(&path).unwrap();

        assert_eq!(loaded.restore_id, "test-restore");
        assert_eq!(loaded.total_files, 5);
        assert!(loaded.is_restored("a.txt"));
        assert!(loaded.is_restored("b.txt"));
        assert!(loaded.completed);
    }

    #[test]
    fn test_load_or_create_new() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("new-checkpoint.json");

        let cp = RestoreCheckpoint::load_or_create(&path, "test", 10).unwrap();
        assert_eq!(cp.restore_id, "test");
        assert_eq!(cp.total_files, 10);
        assert_eq!(cp.restored_files.len(), 0);
    }

    #[test]
    fn test_load_or_create_existing() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("existing-checkpoint.json");

        let mut cp = RestoreCheckpoint::new("test", 5);
        cp.mark_restored("a.txt");
        cp.save(&path).unwrap();

        let loaded = RestoreCheckpoint::load_or_create(&path, "test", 5).unwrap();
        assert!(loaded.is_restored("a.txt"));
    }

    #[test]
    fn test_progress_zero_files() {
        let cp = RestoreCheckpoint::new("test", 0);
        assert_eq!(cp.progress(), 1.0);
    }

    #[test]
    fn test_progress_full() {
        let mut cp = RestoreCheckpoint::new("test", 2);
        cp.mark_restored("a.txt");
        cp.mark_restored("b.txt");
        assert_eq!(cp.progress(), 1.0);
    }

    #[test]
    fn test_checkpoint_path() {
        let path = RestoreCheckpoint::checkpoint_path(
            Path::new("/tmp"),
            "abc-123",
        );
        assert_eq!(path, PathBuf::from("/tmp/abc-123.restore-checkpoint"));
    }

    #[test]
    fn test_duplicate_mark_restored() {
        let mut cp = RestoreCheckpoint::new("test", 2);
        cp.mark_restored("a.txt");
        cp.mark_restored("a.txt");
        assert_eq!(cp.restored_files.len(), 1);
    }
}