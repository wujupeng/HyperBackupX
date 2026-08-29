pub mod checkpoint;
pub mod restore_point;

pub use checkpoint::RestoreCheckpoint;
pub use restore_point::{filter_versions_before, RestorePointResolver};

use std::path::{Path, PathBuf};
use std::sync::Arc;

use hbx_core::domain::chunk::{ChunkHash, ChunkLocation};
use hbx_core::domain::common::VersionId;
use hbx_core::domain::restore::{
    FileSelection, RestoreJob, RestoreMode, RestoreStatus,
};
use hbx_core::domain::repository::{FileEntry, Manifest};
use hbx_core::pipeline::{
    IBackupRepository, ICompressor, IEncryptionProvider, RepoError,
};
use parking_lot::Mutex;
use sha2::{Digest, Sha256};

#[derive(Debug, thiserror::Error)]
pub enum RestoreError {
    #[error("repo error: {0}")]
    Repo(#[from] RepoError),
    #[error("compress error: {0}")]
    Compress(#[from] hbx_core::pipeline::CompressError),
    #[error("encrypt error: {0}")]
    Encrypt(#[from] hbx_core::pipeline::EncryptError),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("restore failed: {0}")]
    Failed(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RestoreState {
    Pending,
    Resolving,
    Planning,
    Downloading,
    Decrypting,
    Reassembling,
    Verifying,
    Success,
    PartialFailed,
    Failed,
}

pub struct RestoreTracker {
    state: Mutex<RestoreState>,
    progress: Mutex<f64>,
}

impl RestoreTracker {
    pub fn new() -> Self {
        Self {
            state: Mutex::new(RestoreState::Pending),
            progress: Mutex::new(0.0),
        }
    }

    pub fn set_state(&self, state: RestoreState) {
        *self.state.lock() = state;
        tracing::info!(state = ?state, "restore state transition");
    }

    pub fn set_progress(&self, progress: f64) {
        *self.progress.lock() = progress.clamp(0.0, 1.0);
    }

    pub fn state(&self) -> RestoreState {
        *self.state.lock()
    }

    pub fn progress(&self) -> f64 {
        *self.progress.lock()
    }

    pub fn status(&self) -> RestoreStatus {
        match self.state() {
            RestoreState::Pending => RestoreStatus::Pending,
            RestoreState::Resolving
            | RestoreState::Planning
            | RestoreState::Downloading
            | RestoreState::Decrypting
            | RestoreState::Reassembling
            | RestoreState::Verifying => RestoreStatus::Running,
            RestoreState::Success => RestoreStatus::Success,
            RestoreState::PartialFailed => RestoreStatus::PartialFailed,
            RestoreState::Failed => RestoreStatus::Failed,
        }
    }
}

impl Default for RestoreTracker {
    fn default() -> Self {
        Self::new()
    }
}

pub struct RestoreResult {
    pub version_id: VersionId,
    pub files_restored: u64,
    pub files_failed: u64,
    pub bytes_restored: u64,
    pub all_verified: bool,
    pub failed_files: Vec<PathBuf>,
}

pub struct RestoreEngine {
    compressor: Arc<dyn ICompressor>,
    encryption: Arc<dyn IEncryptionProvider>,
}

impl RestoreEngine {
    pub fn new(
        compressor: Arc<dyn ICompressor>,
        encryption: Arc<dyn IEncryptionProvider>,
    ) -> Self {
        Self { compressor, encryption }
    }

    pub async fn run_restore(
        &self,
        job: &RestoreJob,
        repo: &dyn IBackupRepository,
        tracker: &RestoreTracker,
    ) -> Result<RestoreResult, RestoreError> {
        tracker.set_state(RestoreState::Resolving);
        let manifest = repo.read_manifest(&job.source_version_id)?;

        tracker.set_state(RestoreState::Planning);
        let selected_files = plan_files(&manifest, &job.file_selection);
        let total_files = selected_files.len().max(1);

        let mut files_restored: u64 = 0;
        let mut files_failed: u64 = 0;
        let mut bytes_restored: u64 = 0;
        let mut failed_files: Vec<PathBuf> = Vec::new();
        let mut all_verified = true;

        for (i, file_entry) in selected_files.iter().enumerate() {
            tracker.set_state(RestoreState::Downloading);
            let target_path = compute_target_path(
                &job.target_location,
                &file_entry.path,
                job.restore_mode,
            );

            if let Some(parent) = target_path.parent() {
                tokio::fs::create_dir_all(parent).await?;
            }

            let should_write = match job.restore_mode {
                RestoreMode::Overwrite | RestoreMode::NewLocation => true,
                RestoreMode::Skip => !target_path.exists(),
                RestoreMode::Rename => true,
            };

            if !should_write {
                tracing::info!(path = %target_path.display(), "skipping existing file");
                files_restored += 1;
                continue;
            }

            tracker.set_state(RestoreState::Decrypting);
            let mut file_data = Vec::new();
            let mut restore_failed = false;

            for chunk_hash in &file_entry.chunks {
                let hash_key = hex::encode(chunk_hash.0);
                let location = if let Some(loc) = manifest.chunk_locations.get(&hash_key) {
                    loc.clone()
                } else {
                    match repo.find_chunk(chunk_hash) {
                        Ok(loc) => loc,
                        Err(_) => chunk_location_from_hash(chunk_hash),
                    }
                };
                let encrypted = repo.read_chunk(&location)?;
                let compressed = self.encryption.decrypt_chunk(&encrypted)?;
                let plain = self.compressor.decompress(&compressed)?;
                file_data.extend_from_slice(&plain);
            }

            tracker.set_state(RestoreState::Reassembling);
            tokio::fs::write(&target_path, &file_data).await?;
            bytes_restored += file_data.len() as u64;

            tracker.set_state(RestoreState::Verifying);
            let computed_hash: [u8; 32] = {
                let mut hasher = Sha256::new();
                hasher.update(&file_data);
                hasher.finalize().into()
            };

            if computed_hash != file_entry.file_hash {
                tracing::warn!(
                    path = %file_entry.path,
                    "file hash mismatch after restore"
                );
                all_verified = false;
                restore_failed = true;
            }

            if restore_failed {
                files_failed += 1;
                failed_files.push(PathBuf::from(&file_entry.path));
            } else {
                files_restored += 1;
            }

            tracker.set_progress((i + 1) as f64 / total_files as f64);
        }

        if files_failed > 0 && files_restored > 0 {
            tracker.set_state(RestoreState::PartialFailed);
        } else if files_failed > 0 {
            tracker.set_state(RestoreState::Failed);
        } else {
            tracker.set_state(RestoreState::Success);
        }

        Ok(RestoreResult {
            version_id: job.source_version_id.clone(),
            files_restored,
            files_failed,
            bytes_restored,
            all_verified,
            failed_files,
        })
    }
}

fn plan_files<'a>(manifest: &'a Manifest, selection: &FileSelection) -> Vec<&'a FileEntry> {
    match selection {
        FileSelection::All => manifest.files.iter().collect(),
        FileSelection::FileList(paths) => {
            let path_set: std::collections::HashSet<&str> =
                paths.iter().map(|p| p.to_str().unwrap_or("")).collect();
            manifest
                .files
                .iter()
                .filter(|f| path_set.contains(f.path.as_str()))
                .collect()
        }
        FileSelection::Glob(pattern) => {
            manifest
                .files
                .iter()
                .filter(|f| glob_match(pattern, &f.path))
                .collect()
        }
        FileSelection::Search(query) => {
            manifest
                .files
                .iter()
                .filter(|f| f.path.contains(query.as_str()))
                .collect()
        }
        FileSelection::DateRange { from, to } => {
            manifest
                .files
                .iter()
                .filter(|f| f.modified_at >= *from && f.modified_at <= *to)
                .collect()
        }
    }
}

fn compute_target_path(
    target_base: &Path,
    source_path: &str,
    mode: RestoreMode,
) -> PathBuf {
    let source = Path::new(source_path);
    let file_name = source.file_name().unwrap_or_default();

    match mode {
        RestoreMode::Overwrite | RestoreMode::NewLocation | RestoreMode::Skip => {
            target_base.join(file_name)
        }
        RestoreMode::Rename => {
            let mut name = file_name.to_os_string();
            name.push(".restored");
            target_base.join(name)
        }
    }
}

fn chunk_location_from_hash(hash: &ChunkHash) -> ChunkLocation {
    ChunkLocation {
        bucket: format!("{:02x}", hash.0[0]),
        path: hex::encode(hash.0) + ".chunk",
    }
}

fn glob_match(pattern: &str, text: &str) -> bool {
    let pat: Vec<char> = pattern.chars().collect();
    let txt: Vec<char> = text.chars().collect();
    glob_helper(&pat, 0, &txt, 0)
}

fn glob_helper(pat: &[char], pi: usize, txt: &[char], ti: usize) -> bool {
    if pi == pat.len() {
        return ti == txt.len();
    }
    if pat[pi] == '*' {
        if pi + 1 == pat.len() {
            return true;
        }
        for next in ti..=txt.len() {
            if glob_helper(pat, pi + 1, txt, next) {
                return true;
            }
        }
        return false;
    }
    if ti < txt.len() && (pat[pi] == '?' || pat[pi] == txt[ti]) {
        return glob_helper(pat, pi + 1, txt, ti + 1);
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use hbx_chunker::FixedChunker;
    use hbx_compress::ZstdCompressor;
    use hbx_core::domain::backup::{
        BackupDestination, BackupJob, BackupSource, JobStatus,
    };
    use hbx_core::domain::common::{
        CompressionAlgorithm, CompressionProfile, EncryptionProfileRef, JobId,
        RepositoryId, RetentionPolicyRef, ScheduleRef,
    };
    use hbx_core::domain::repository::BackendType;
    use hbx_core::pipeline::ChunkStrategy;
    use hbx_dedup::LocalDedupIndex;
    use hbx_engine::{BackupEngine, NoOpEncryptionProvider};
    use hbx_repo::{LocalRepository, RepositoryInitializer};
    use hbx_scanner::LocalScanner;
    use sha2::{Digest, Sha256};
    use std::fs;
    use uuid::Uuid;

    fn make_backup_job(source_path: PathBuf) -> BackupJob {
        BackupJob {
            job_id: JobId(Uuid::new_v4()),
            name: "test-restore-backup".to_string(),
            source: BackupSource {
                paths: vec![source_path],
                include_rules: vec![],
                exclude_rules: vec![],
            },
            destination: BackupDestination {
                repository_id: RepositoryId(Uuid::new_v4()),
                logical_path: "/".to_string(),
            },
            schedule: ScheduleRef(Uuid::new_v4()),
            retention_policy: RetentionPolicyRef(Uuid::new_v4()),
            encryption_profile: EncryptionProfileRef(Uuid::new_v4()),
            compression_profile: CompressionProfile {
                algorithm: CompressionAlgorithm::Zstd,
                level: 3,
            },
            status: JobStatus::Active,
            created_at: chrono::Utc::now(),
        }
    }

    fn make_restore_job(version_id: VersionId, target: PathBuf) -> RestoreJob {
        RestoreJob {
            restore_id: hbx_core::domain::common::RestoreId(Uuid::new_v4()),
            source_version_id: version_id,
            file_selection: FileSelection::All,
            restore_mode: RestoreMode::Overwrite,
            target_location: target,
            status: RestoreStatus::Pending,
            started_at: None,
            completed_at: None,
            failed_files: vec![],
        }
    }

    struct TestSetup {
        _src_dir: tempfile::TempDir,
        _repo_dir: tempfile::TempDir,
        repo: LocalRepository,
        version_id: VersionId,
        source_files: Vec<(String, Vec<u8>)>,
    }

    async fn setup_backup() -> TestSetup {
        let src_dir = tempfile::tempdir().unwrap();
        let repo_dir = tempfile::tempdir().unwrap();

        let files = vec![
            ("a.txt".to_string(), b"hello world".to_vec()),
            ("b.txt".to_string(), b"foo bar baz qux".to_vec()),
            ("sub/c.txt".to_string(), b"nested content here".to_vec()),
        ];

        let mut source_files = Vec::new();
        for (name, content) in &files {
            let path = src_dir.path().join(name);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).unwrap();
            }
            fs::write(&path, content).unwrap();
            source_files.push((path.to_string_lossy().to_string(), content.clone()));
        }

        RepositoryInitializer::new(repo_dir.path())
            .init(RepositoryId(Uuid::new_v4()), BackendType::Local)
            .unwrap();
        let repo = LocalRepository::open(repo_dir.path()).unwrap();

        let engine = BackupEngine::builder()
            .scanner(LocalScanner::new())
            .chunker(FixedChunker::new())
            .dedup(LocalDedupIndex::new())
            .compressor(ZstdCompressor::default())
            .encryption(NoOpEncryptionProvider)
            .repo(LocalRepository::open(repo_dir.path()).unwrap())
            .memory_limit(256 * 1024 * 1024)
            .chunk_strategy(ChunkStrategy::Fixed { chunk_size: 1024 })
            .build()
            .unwrap();

        let job = make_backup_job(src_dir.path().to_path_buf());
        let tracker = engine.execution_tracker(&job.job_id);
        let result = engine.run_backup(&job, &tracker).await.unwrap();
        let version_id = result.version_id.unwrap();

        TestSetup {
            _src_dir: src_dir,
            _repo_dir: repo_dir,
            repo,
            version_id,
            source_files,
        }
    }

    fn make_restore_engine() -> RestoreEngine {
        RestoreEngine::new(
            Arc::new(ZstdCompressor::default()),
            Arc::new(NoOpEncryptionProvider),
        )
    }

    #[tokio::test]
    async fn test_restore_all_files() {
        let setup = setup_backup().await;
        let target_dir = tempfile::tempdir().unwrap();

        let job = make_restore_job(setup.version_id, target_dir.path().to_path_buf());
        let tracker = RestoreTracker::new();
        let engine = make_restore_engine();

        let result = engine.run_restore(&job, &setup.repo, &tracker).await.unwrap();

        assert_eq!(result.files_restored, 3);
        assert_eq!(result.files_failed, 0);
        assert!(result.all_verified);
        assert_eq!(tracker.state(), RestoreState::Success);
    }

    #[tokio::test]
    async fn test_restore_sha256_consistency() {
        let setup = setup_backup().await;
        let target_dir = tempfile::tempdir().unwrap();

        let job = make_restore_job(setup.version_id, target_dir.path().to_path_buf());
        let tracker = RestoreTracker::new();
        let engine = make_restore_engine();

        let result = engine.run_restore(&job, &setup.repo, &tracker).await.unwrap();
        assert!(result.all_verified);

        for (source_path, original_content) in &setup.source_files {
            let source = Path::new(source_path);
            let restored_path = compute_target_path(
                target_dir.path(),
                source_path,
                RestoreMode::Overwrite,
            );
            if !restored_path.exists() {
                continue;
            }
            let restored_content = fs::read(&restored_path).unwrap();

            let original_hash: [u8; 32] = {
                let mut h = Sha256::new();
                h.update(original_content);
                h.finalize().into()
            };
            let restored_hash: [u8; 32] = {
                let mut h = Sha256::new();
                h.update(&restored_content);
                h.finalize().into()
            };
            assert_eq!(
                original_hash, restored_hash,
                "SHA-256 mismatch for {}",
                source.display()
            );
        }
    }

    #[tokio::test]
    async fn test_restore_empty_version() {
        let src_dir = tempfile::tempdir().unwrap();
        let repo_dir = tempfile::tempdir().unwrap();

        RepositoryInitializer::new(repo_dir.path())
            .init(RepositoryId(Uuid::new_v4()), BackendType::Local)
            .unwrap();
        let repo = LocalRepository::open(repo_dir.path()).unwrap();

        let engine = BackupEngine::builder()
            .scanner(LocalScanner::new())
            .chunker(FixedChunker::new())
            .dedup(LocalDedupIndex::new())
            .compressor(ZstdCompressor::default())
            .encryption(NoOpEncryptionProvider)
            .repo(LocalRepository::open(repo_dir.path()).unwrap())
            .memory_limit(256 * 1024 * 1024)
            .chunk_strategy(ChunkStrategy::Fixed { chunk_size: 1024 })
            .build()
            .unwrap();

        let job = make_backup_job(src_dir.path().to_path_buf());
        let tracker = engine.execution_tracker(&job.job_id);
        let result = engine.run_backup(&job, &tracker).await.unwrap();
        let version_id = result.version_id.unwrap();

        let target_dir = tempfile::tempdir().unwrap();
        let restore_job = make_restore_job(version_id, target_dir.path().to_path_buf());
        let restore_tracker = RestoreTracker::new();
        let restore_engine = make_restore_engine();

        let result = restore_engine
            .run_restore(&restore_job, &repo, &restore_tracker)
            .await
            .unwrap();

        assert_eq!(result.files_restored, 0);
        assert_eq!(restore_tracker.state(), RestoreState::Success);
    }

    #[tokio::test]
    async fn test_restore_state_machine() {
        let setup = setup_backup().await;
        let target_dir = tempfile::tempdir().unwrap();

        let job = make_restore_job(setup.version_id, target_dir.path().to_path_buf());
        let tracker = RestoreTracker::new();
        let engine = make_restore_engine();

        assert_eq!(tracker.state(), RestoreState::Pending);

        engine.run_restore(&job, &setup.repo, &tracker).await.unwrap();

        assert_eq!(tracker.state(), RestoreState::Success);
        assert!(tracker.progress() >= 1.0);
    }

    #[tokio::test]
    async fn test_restore_file_selection_glob() {
        let setup = setup_backup().await;
        let target_dir = tempfile::tempdir().unwrap();

        let mut job = make_restore_job(setup.version_id, target_dir.path().to_path_buf());
        job.file_selection = FileSelection::Glob("*.txt".to_string());
        let tracker = RestoreTracker::new();
        let engine = make_restore_engine();

        let result = engine.run_restore(&job, &setup.repo, &tracker).await.unwrap();

        assert!(result.files_restored > 0);
        assert_eq!(result.files_failed, 0);
    }

    #[tokio::test]
    async fn test_restore_mode_overwrite() {
        let setup = setup_backup().await;
        let target_dir = tempfile::tempdir().unwrap();

        let target_file = target_dir.path().join("a.txt");
        fs::write(&target_file, b"old content").unwrap();

        let job = make_restore_job(setup.version_id, target_dir.path().to_path_buf());
        let tracker = RestoreTracker::new();
        let engine = make_restore_engine();

        let result = engine.run_restore(&job, &setup.repo, &tracker).await.unwrap();
        assert_eq!(result.files_restored, 3);

        let content = fs::read_to_string(&target_file).unwrap();
        assert_eq!(content, "hello world");
    }

    #[tokio::test]
    async fn test_restore_mode_skip() {
        let setup = setup_backup().await;
        let target_dir = tempfile::tempdir().unwrap();

        let target_file = target_dir.path().join("a.txt");
        fs::write(&target_file, b"existing content").unwrap();

        let mut job = make_restore_job(setup.version_id, target_dir.path().to_path_buf());
        job.restore_mode = RestoreMode::Skip;
        let tracker = RestoreTracker::new();
        let engine = make_restore_engine();

        let result = engine.run_restore(&job, &setup.repo, &tracker).await.unwrap();
        assert_eq!(result.files_restored, 3);

        let content = fs::read_to_string(&target_file).unwrap();
        assert_eq!(content, "existing content");
    }

    #[tokio::test]
    async fn test_restore_mode_rename() {
        let setup = setup_backup().await;
        let target_dir = tempfile::tempdir().unwrap();

        let mut job = make_restore_job(setup.version_id, target_dir.path().to_path_buf());
        job.restore_mode = RestoreMode::Rename;
        let tracker = RestoreTracker::new();
        let engine = make_restore_engine();

        let result = engine.run_restore(&job, &setup.repo, &tracker).await.unwrap();
        assert_eq!(result.files_restored, 3);

        let renamed_file = target_dir.path().join("a.txt.restored");
        assert!(renamed_file.exists());
        let content = fs::read_to_string(&renamed_file).unwrap();
        assert_eq!(content, "hello world");
    }

    #[tokio::test]
    async fn test_restore_mode_new_location() {
        let setup = setup_backup().await;
        let target_dir = tempfile::tempdir().unwrap();

        let mut job = make_restore_job(setup.version_id, target_dir.path().to_path_buf());
        job.restore_mode = RestoreMode::NewLocation;
        let tracker = RestoreTracker::new();
        let engine = make_restore_engine();

        let result = engine.run_restore(&job, &setup.repo, &tracker).await.unwrap();
        assert_eq!(result.files_restored, 3);
        assert!(result.all_verified);

        let restored_file = target_dir.path().join("a.txt");
        assert!(restored_file.exists());
    }

    #[tokio::test]
    async fn test_restore_checkpoint_breakpoint() {
        let setup = setup_backup().await;
        let target_dir = tempfile::tempdir().unwrap();
        let cp_dir = tempfile::tempdir().unwrap();

        let cp_path = RestoreCheckpoint::checkpoint_path(cp_dir.path(), "test-restore-1");

        let job = make_restore_job(setup.version_id, target_dir.path().to_path_buf());

        let manifest = setup.repo.read_manifest(&job.source_version_id).unwrap();
        let selected_files = manifest.files.iter().collect::<Vec<_>>();

        let mut cp = RestoreCheckpoint::new("test-restore-1", selected_files.len());
        cp.mark_restored(&selected_files[0].path);
        cp.save(&cp_path).unwrap();

        let mut files_restored = 0;
        for file_entry in &selected_files {
            if cp.is_restored(&file_entry.path) {
                continue;
            }
            files_restored += 1;
            cp.mark_restored(&file_entry.path);
        }

        assert!(files_restored < selected_files.len());
        assert_eq!(cp.restored_files.len(), selected_files.len());
        cp.mark_completed();
        assert!(cp.completed);
    }

    #[tokio::test]
    async fn test_restore_file_selection_file_list() {
        let setup = setup_backup().await;
        let target_dir = tempfile::tempdir().unwrap();

        let manifest = setup
            .repo
            .read_manifest(&setup.version_id)
            .unwrap();
        let first_file_path = manifest.files[0].path.clone();

        let mut job = make_restore_job(setup.version_id, target_dir.path().to_path_buf());
        job.file_selection = FileSelection::FileList(vec![first_file_path.into()]);
        let tracker = RestoreTracker::new();
        let engine = make_restore_engine();

        let result = engine.run_restore(&job, &setup.repo, &tracker).await.unwrap();
        assert!(result.files_restored >= 1);
    }

    #[tokio::test]
    async fn test_restore_file_selection_search() {
        let setup = setup_backup().await;
        let target_dir = tempfile::tempdir().unwrap();

        let mut job = make_restore_job(setup.version_id, target_dir.path().to_path_buf());
        job.file_selection = FileSelection::Search("b".to_string());
        let tracker = RestoreTracker::new();
        let engine = make_restore_engine();

        let result = engine.run_restore(&job, &setup.repo, &tracker).await.unwrap();
        assert!(result.files_restored >= 1);
    }

    #[tokio::test]
    async fn test_restore_does_not_modify_source() {
        let setup = setup_backup().await;
        let target_dir = tempfile::tempdir().unwrap();

        let original_files: Vec<(String, Vec<u8>)> = setup
            .source_files
            .iter()
            .map(|(p, _c)| (p.clone(), fs::read(p).unwrap_or_default()))
            .collect();

        let job = make_restore_job(setup.version_id, target_dir.path().to_path_buf());
        let tracker = RestoreTracker::new();
        let engine = make_restore_engine();

        engine.run_restore(&job, &setup.repo, &tracker).await.unwrap();

        for (path, original_content) in &original_files {
            let current_content = fs::read(path).unwrap_or_default();
            assert_eq!(
                current_content, *original_content,
                "source file {} was modified during restore",
                path
            );
        }
    }
}
