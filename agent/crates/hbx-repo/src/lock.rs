use std::fs;
use std::path::PathBuf;
use std::time::Duration;

use hbx_core::domain::common::{LockOperation, RepoLock};
use hbx_core::pipeline::RepoError;
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

const DEFAULT_LOCK_TTL_SECS: u64 = 1800;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LockFile {
    pub lock_id: Uuid,
    pub holder: String,
    pub acquired_at: chrono::DateTime<chrono::Utc>,
    pub ttl_secs: u64,
    pub operation: String,
}

impl LockFile {
    pub fn new(operation: LockOperation, ttl: Duration) -> Self {
        let lock_id = Uuid::new_v4();
        Self {
            lock_id,
            holder: format!("{:?}", operation),
            acquired_at: chrono::Utc::now(),
            ttl_secs: ttl.as_secs(),
            operation: format!("{:?}", operation),
        }
    }

    pub fn is_expired(&self) -> bool {
        let elapsed = chrono::Utc::now().signed_duration_since(self.acquired_at);
        elapsed.num_seconds() > self.ttl_secs as i64
    }

    pub fn to_repo_lock(&self) -> RepoLock {
        RepoLock {
            lock_id: self.lock_id,
            holder: self.holder.clone(),
            acquired_at: self.acquired_at,
            ttl: Duration::from_secs(self.ttl_secs),
        }
    }
}

pub struct LockManager {
    locks_dir: PathBuf,
    _mutex: Mutex<()>,
}

impl LockManager {
    pub fn new(locks_dir: impl Into<PathBuf>) -> Self {
        Self {
            locks_dir: locks_dir.into(),
            _mutex: Mutex::new(()),
        }
    }

    pub fn ensure_dir(&self) -> Result<(), RepoError> {
        fs::create_dir_all(&self.locks_dir)?;
        Ok(())
    }

    fn lock_path(&self, lock_id: &Uuid) -> PathBuf {
        self.locks_dir.join(format!("{lock_id}.lock"))
    }

    pub fn acquire(
        &self,
        operation: LockOperation,
        timeout: Duration,
    ) -> Result<RepoLock, RepoError> {
        let _guard = self._mutex.lock();
        self.ensure_dir()?;

        self.cleanup_expired_unlocked()?;

        let lock_file = LockFile::new(operation, timeout);
        let path = self.lock_path(&lock_file.lock_id);
        let data = serde_json::to_vec_pretty(&lock_file)?;
        fs::write(&path, &data)?;

        Ok(lock_file.to_repo_lock())
    }

    pub fn release(&self, lock_id: &Uuid) -> Result<(), RepoError> {
        let _guard = self._mutex.lock();
        let path = self.lock_path(lock_id);
        if path.exists() {
            fs::remove_file(&path)?;
        }
        Ok(())
    }

    pub fn is_locked(&self) -> Result<bool, RepoError> {
        let _guard = self._mutex.lock();
        if !self.locks_dir.exists() {
            return Ok(false);
        }

        for entry in fs::read_dir(&self.locks_dir)? {
            let entry = entry?;
            let path = entry.path();
            if !path.is_file() || !path.extension().map(|e| e == "lock").unwrap_or(false) {
                continue;
            }
            let data = fs::read(&path)?;
            if let Ok(lock_file) = serde_json::from_slice::<LockFile>(&data) {
                if !lock_file.is_expired() {
                    return Ok(true);
                }
            }
        }
        Ok(false)
    }

    pub fn cleanup_expired(&self) -> Result<usize, RepoError> {
        let _guard = self._mutex.lock();
        self.cleanup_expired_unlocked()
    }

    fn cleanup_expired_unlocked(&self) -> Result<usize, RepoError> {
        if !self.locks_dir.exists() {
            return Ok(0);
        }

        let mut count = 0;
        for entry in fs::read_dir(&self.locks_dir)? {
            let entry = entry?;
            let path = entry.path();
            if !path.is_file() || !path.extension().map(|e| e == "lock").unwrap_or(false) {
                continue;
            }
            let data = fs::read(&path)?;
            if let Ok(lock_file) = serde_json::from_slice::<LockFile>(&data) {
                if lock_file.is_expired() {
                    fs::remove_file(&path)?;
                    count += 1;
                }
            }
        }
        Ok(count)
    }

    pub fn list_active_locks(&self) -> Result<Vec<LockFile>, RepoError> {
        let _guard = self._mutex.lock();
        let mut locks = Vec::new();

        if !self.locks_dir.exists() {
            return Ok(locks);
        }

        for entry in fs::read_dir(&self.locks_dir)? {
            let entry = entry?;
            let path = entry.path();
            if !path.is_file() || !path.extension().map(|e| e == "lock").unwrap_or(false) {
                continue;
            }
            let data = fs::read(&path)?;
            if let Ok(lock_file) = serde_json::from_slice::<LockFile>(&data) {
                if !lock_file.is_expired() {
                    locks.push(lock_file);
                }
            }
        }

        Ok(locks)
    }
}

pub fn default_ttl() -> Duration {
    Duration::from_secs(DEFAULT_LOCK_TTL_SECS)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;


    #[test]
    fn test_lock_acquire_release() {
        let dir = tempfile::tempdir().unwrap();
        let manager = LockManager::new(dir.path().join("locks"));

        let lock = manager.acquire(LockOperation::Backup, default_ttl()).unwrap();
        assert_eq!(lock.holder, "Backup");

        assert!(manager.is_locked().unwrap());

        manager.release(&lock.lock_id).unwrap();
        assert!(!manager.is_locked().unwrap());
    }

    #[test]
    fn test_lock_file_expired() {
        let lock_file = LockFile {
            lock_id: Uuid::new_v4(),
            holder: "Backup".to_string(),
            acquired_at: chrono::Utc::now() - chrono::Duration::seconds(3600),
            ttl_secs: 60,
            operation: "Backup".to_string(),
        };
        assert!(lock_file.is_expired());
    }

    #[test]
    fn test_lock_file_not_expired() {
        let lock_file = LockFile::new(LockOperation::Backup, Duration::from_secs(3600));
        assert!(!lock_file.is_expired());
    }

    #[test]
    fn test_cleanup_expired() {
        let dir = tempfile::tempdir().unwrap();
        let manager = LockManager::new(dir.path().join("locks"));

        let expired_lock = LockFile {
            lock_id: Uuid::new_v4(),
            holder: "Backup".to_string(),
            acquired_at: chrono::Utc::now() - chrono::Duration::seconds(3600),
            ttl_secs: 60,
            operation: "Backup".to_string(),
        };
        let path = dir.path().join("locks").join(format!("{}.lock", expired_lock.lock_id));
        fs::create_dir_all(dir.path().join("locks")).unwrap();
        fs::write(&path, serde_json::to_vec_pretty(&expired_lock).unwrap()).unwrap();

        let cleaned = manager.cleanup_expired().unwrap();
        assert_eq!(cleaned, 1);
        assert!(!path.exists());
    }

    #[test]
    fn test_multiple_locks() {
        let dir = tempfile::tempdir().unwrap();
        let manager = LockManager::new(dir.path().join("locks"));

        let lock1 = manager.acquire(LockOperation::Backup, default_ttl()).unwrap();
        let lock2 = manager.acquire(LockOperation::Verify, default_ttl()).unwrap();

        let active = manager.list_active_locks().unwrap();
        assert_eq!(active.len(), 2);

        manager.release(&lock1.lock_id).unwrap();
        manager.release(&lock2.lock_id).unwrap();

        let active = manager.list_active_locks().unwrap();
        assert_eq!(active.len(), 0);
    }

    #[test]
    fn test_concurrent_acquire() {
        use std::sync::Arc;
        let dir = tempfile::tempdir().unwrap();
        let locks_dir = dir.path().join("locks").to_path_buf();
        let managers: Vec<Arc<LockManager>> = (0..3)
            .map(|_| Arc::new(LockManager::new(locks_dir.clone())))
            .collect();

        let mut handles = Vec::new();
        for manager in managers {
            let manager = manager.clone();
            handles.push(thread::spawn(move || {
                let lock = manager.acquire(LockOperation::Backup, default_ttl()).unwrap();
                thread::sleep(Duration::from_millis(10));
                manager.release(&lock.lock_id).unwrap();
            }));
        }

        for handle in handles {
            handle.join().unwrap();
        }

        let manager = LockManager::new(&locks_dir);
        assert!(!manager.is_locked().unwrap());
    }

    #[test]
    fn test_lock_serialization() {
        let lock_file = LockFile::new(LockOperation::Restore, Duration::from_secs(600));
        let json = serde_json::to_string(&lock_file).unwrap();
        let deserialized: LockFile = serde_json::from_str(&json).unwrap();
        assert_eq!(lock_file.lock_id, deserialized.lock_id);
        assert_eq!(lock_file.holder, deserialized.holder);
        assert_eq!(lock_file.ttl_secs, deserialized.ttl_secs);
    }

    #[test]
    fn test_default_ttl() {
        assert_eq!(default_ttl(), Duration::from_secs(1800));
    }
}