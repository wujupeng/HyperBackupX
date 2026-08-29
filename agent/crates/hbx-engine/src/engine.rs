use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use futures::stream::StreamExt;
use hbx_core::domain::backup::{
    BackupExecution, BackupJob, BackupResult, BackupSnapshot, BackupType, ExecutionState,
};
use hbx_core::domain::chunk::{ChunkHash, ChunkId, ChunkLocation, ChunkReference};
use hbx_core::domain::common::{ExecutionId, FilterRule, VersionId};
use hbx_core::domain::repository::{FileEntry, Manifest, ManifestHashes};
use hbx_core::pipeline::{
    ChunkStrategy, IBackupRepository, IChunker, ICompressor, IDedupIndex, IEncryptionProvider,
    IJournal, IScanner, JournalEntry,
};
use parking_lot::Mutex;
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::concurrent::{BackupLockGuard, StagingTracker, is_storage_full};
use crate::memory::MemoryBudget;

#[derive(Debug, thiserror::Error)]
pub enum EngineError {
    #[error("scan error: {0}")]
    Scan(#[from] hbx_core::pipeline::ScanError),
    #[error("chunk error: {0}")]
    Chunk(#[from] hbx_core::pipeline::ChunkError),
    #[error("compress error: {0}")]
    Compress(#[from] hbx_core::pipeline::CompressError),
    #[error("encrypt error: {0}")]
    Encrypt(#[from] hbx_core::pipeline::EncryptError),
    #[error("repo error: {0}")]
    Repo(#[from] hbx_core::pipeline::RepoError),
    #[error("index error: {0}")]
    Index(#[from] hbx_core::pipeline::IndexError),
    #[error("journal error: {0}")]
    Journal(#[from] hbx_core::pipeline::JournalError),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("serialization error: {0}")]
    Serialize(#[from] serde_json::Error),
    #[error("backup failed: {0}")]
    Failed(String),
}

pub struct ExecutionTracker {
    inner: Mutex<BackupExecution>,
}

impl ExecutionTracker {
    fn new(job_id: &hbx_core::domain::common::JobId) -> Self {
        let execution = BackupExecution {
            execution_id: ExecutionId(Uuid::new_v4()),
            job_id: job_id.clone(),
            started_at: chrono::Utc::now(),
            completed_at: None,
            state: ExecutionState::Pending,
            progress: 0.0,
            checkpoint: None,
        };
        Self {
            inner: Mutex::new(execution),
        }
    }

    pub fn set_state(&self, state: ExecutionState) {
        let mut guard = self.inner.lock();
        guard.state = state;
        if state == ExecutionState::Success || state == ExecutionState::Failed {
            guard.completed_at = Some(chrono::Utc::now());
            guard.progress = 1.0;
        }
        tracing::info!(state = ?state, "execution state transition");
    }

    pub fn set_progress(&self, progress: f64) {
        let mut guard = self.inner.lock();
        guard.progress = progress.clamp(0.0, 1.0);
    }

    pub fn snapshot(&self) -> BackupExecution {
        self.inner.lock().clone()
    }
}

pub struct BackupEngine {
    scanner: Arc<dyn IScanner>,
    chunker: Arc<dyn IChunker>,
    dedup: Arc<dyn IDedupIndex>,
    compressor: Arc<dyn ICompressor>,
    encryption: Arc<dyn IEncryptionProvider>,
    repo: Arc<dyn IBackupRepository>,
    journal: Option<Arc<dyn IJournal>>,
    memory_budget: Arc<MemoryBudget>,
    chunk_strategy: ChunkStrategy,
}

impl BackupEngine {
    pub fn builder() -> BackupEngineBuilder {
        BackupEngineBuilder::default()
    }

    pub fn execution_tracker(&self, job_id: &hbx_core::domain::common::JobId) -> ExecutionTracker {
        ExecutionTracker::new(job_id)
    }

    pub fn memory_used(&self) -> u64 {
        self.memory_budget.used()
    }

    pub fn memory_limit(&self) -> u64 {
        self.memory_budget.limit()
    }

    pub async fn run_backup(
        &self,
        job: &BackupJob,
        tracker: &ExecutionTracker,
    ) -> Result<BackupResult, EngineError> {
        let start = Instant::now();
        let version_id = VersionId(Uuid::new_v4());

        if let Some(ref journal) = self.journal {
            let _ = journal.append(JournalEntry::TaskStarted {
                job_id: job.job_id.clone(),
                execution_id: tracker.snapshot().execution_id,
                timestamp: chrono::Utc::now().timestamp() as u64,
            });
        }

        tracker.set_state(ExecutionState::Scanning);
        let filter = FilterRule::Glob("*".to_string());
        let file_stream = self.scanner.scan(&job.source, &filter, None)?;

        let estimate = self.scanner.estimate(&job.source, &filter);
        let total_files = estimate.total_files.max(1);

        tracker.set_state(ExecutionState::Chunking);

        let mut files: Vec<FileEntry> = Vec::new();
        let mut chunk_refs: Vec<ChunkReference> = Vec::new();
        let mut chunk_locations: std::collections::HashMap<String, ChunkLocation> = std::collections::HashMap::new();
        let mut data_processed: u64 = 0;
        let mut data_stored: u64 = 0;
        let mut chunk_count: u64 = 0;
        let mut file_count: u64 = 0;
        let mut skipped_files: Vec<PathBuf> = Vec::new();

        tokio::pin!(file_stream);
        while let Some(file_entry) = file_stream.next().await {
            let path = PathBuf::from(&file_entry.path);
            let file = match tokio::fs::File::open(&path).await {
                Ok(f) => f,
                Err(e) => {
                    tracing::warn!(path = %file_entry.path, error = %e, "failed to open file, skipping");
                    skipped_files.push(path);
                    continue;
                }
            };

            let reader: Box<dyn tokio::io::AsyncRead + Send + Unpin> = Box::new(file);
            let chunk_stream = self.chunker.chunk(reader, self.chunk_strategy)?;

            let mut file_hasher = Sha256::new();
            let mut file_chunks: Vec<ChunkHash> = Vec::new();

            tracker.set_state(ExecutionState::Chunking);
            tokio::pin!(chunk_stream);
            while let Some(chunk) = chunk_stream.next().await {
                file_hasher.update(&chunk.data);
                data_processed += chunk.data.len() as u64;

                let hash = compute_chunk_hash(&chunk.data);

                let lookup = self.dedup.batch_lookup(std::slice::from_ref(&hash))?;
                let is_new = !lookup[0].exists;

                if is_new {
                    if self.repo.chunk_exists(&hash)? {
                        let location = self.repo.find_chunk(&hash)?;
                        self.dedup.register_new(&hash, &location)?;
                        chunk_locations.insert(hex::encode(hash.0), location);
                        tracing::debug!(hash = ?hash, "chunk already exists in repo, idempotent skip");
                    } else {
                        tracker.set_state(ExecutionState::Encrypting);
                        let compressed = self.compressor.compress(&chunk.data)?;
                        let chunk_id = ChunkId(Uuid::new_v4());
                        let encrypted = self.encryption.encrypt_chunk(&compressed, &chunk_id)?;

                        tracker.set_state(ExecutionState::Uploading);
                        let _guard = self.memory_budget.acquire(encrypted.ciphertext.len() as u64).await;
                        let location = self.repo.write_chunk(&hash, &encrypted)?;
                        data_stored += encrypted.ciphertext.len() as u64;

                        self.dedup.register_new(&hash, &location)?;
                        chunk_locations.insert(hex::encode(hash.0), location);
                    }
                } else if let Some(ref loc) = lookup[0].location {
                    chunk_locations.insert(hex::encode(hash.0), loc.clone());
                }

                chunk_count += 1;
                file_chunks.push(hash.clone());

                let chunk_ref = ChunkReference {
                    hash,
                    version_id: version_id.clone(),
                    file_path: file_entry.path.clone(),
                    offset: chunk.offset,
                };
                chunk_refs.push(chunk_ref);
            }

            let file_hash: [u8; 32] = file_hasher.finalize().into();

            let completed_entry = FileEntry {
                path: file_entry.path,
                size: file_entry.size,
                modified_at: file_entry.modified_at,
                attributes: file_entry.attributes,
                chunks: file_chunks,
                file_hash,
            };
            files.push(completed_entry);
            file_count += 1;

            tracker.set_progress(file_count as f64 / total_files as f64);

            if let Some(ref journal) = self.journal {
                let _ = journal.append(JournalEntry::FileProcessed {
                    job_id: job.job_id.clone(),
                    file_path: files.last().unwrap().path.clone(),
                    chunks: files.last().unwrap().chunks.clone(),
                });
            }
        }

        tracker.set_state(ExecutionState::Committing);
        self.dedup.add_references(&chunk_refs)?;

        let manifest = self.build_manifest(
            &version_id,
            None,
            1,
            BackupType::Full,
            files,
            chunk_refs,
            chunk_locations,
        )?;

        self.repo.write_manifest(&version_id, &manifest)?;

        if let Some(ref journal) = self.journal {
            let result = BackupResult {
                version_id: Some(version_id.clone()),
                data_processed,
                data_stored,
                dedup_ratio: compute_dedup_ratio(data_processed, data_stored),
                chunk_count,
                file_count,
                duration: start.elapsed(),
                skipped_files: skipped_files.clone(),
            };
            let _ = journal.append(JournalEntry::TaskCompleted {
                job_id: job.job_id.clone(),
                version_id: version_id.clone(),
                result,
            });
        }

        tracker.set_state(ExecutionState::Success);

        let duration = start.elapsed();
        let dedup_ratio = compute_dedup_ratio(data_processed, data_stored);

        Ok(BackupResult {
            version_id: Some(version_id),
            data_processed,
            data_stored,
            dedup_ratio,
            chunk_count,
            file_count,
            duration,
            skipped_files,
        })
    }

    pub async fn run_incremental_backup(
        &self,
        job: &BackupJob,
        baseline_version_id: &VersionId,
        tracker: &ExecutionTracker,
    ) -> Result<BackupResult, EngineError> {
        let start = Instant::now();
        let version_id = VersionId(Uuid::new_v4());

        tracker.set_state(ExecutionState::Scanning);
        let prev_manifest = self.repo.read_manifest(baseline_version_id)?;

        let baseline_snapshot = BackupSnapshot {
            version_id: baseline_version_id.clone(),
            timestamp: prev_manifest.timestamp,
            files: prev_manifest
                .files
                .iter()
                .map(|f| (f.path.clone(), f.size, f.file_hash))
                .collect(),
        };

        let filter = FilterRule::Glob("*".to_string());
        let file_stream = self.scanner.scan(&job.source, &filter, Some(&baseline_snapshot))?;

        let estimate = self.scanner.estimate(&job.source, &filter);
        let total_files = estimate.total_files.max(1);

        tracker.set_state(ExecutionState::Chunking);

        let mut changed_files: Vec<FileEntry> = Vec::new();
        let mut changed_chunk_refs: Vec<ChunkReference> = Vec::new();
        let mut chunk_locations: std::collections::HashMap<String, ChunkLocation> = std::collections::HashMap::new();
        let mut changed_paths: std::collections::HashSet<String> = std::collections::HashSet::new();
        let mut data_processed: u64 = 0;
        let mut data_stored: u64 = 0;
        let mut chunk_count: u64 = 0;
        let mut skipped_files: Vec<PathBuf> = Vec::new();

        tokio::pin!(file_stream);
        while let Some(file_entry) = file_stream.next().await {
            changed_paths.insert(file_entry.path.clone());

            let path = PathBuf::from(&file_entry.path);
            let file = match tokio::fs::File::open(&path).await {
                Ok(f) => f,
                Err(e) => {
                    tracing::warn!(path = %file_entry.path, error = %e, "failed to open file, skipping");
                    skipped_files.push(path);
                    continue;
                }
            };

            let reader: Box<dyn tokio::io::AsyncRead + Send + Unpin> = Box::new(file);
            let chunk_stream = self.chunker.chunk(reader, self.chunk_strategy)?;

            let mut file_hasher = Sha256::new();
            let mut file_chunks: Vec<ChunkHash> = Vec::new();

            tokio::pin!(chunk_stream);
            while let Some(chunk) = chunk_stream.next().await {
                file_hasher.update(&chunk.data);
                data_processed += chunk.data.len() as u64;

                let hash = compute_chunk_hash(&chunk.data);
                let lookup = self.dedup.batch_lookup(std::slice::from_ref(&hash))?;
                let is_new = !lookup[0].exists;

                if is_new {
                    if self.repo.chunk_exists(&hash)? {
                        let location = self.repo.find_chunk(&hash)?;
                        self.dedup.register_new(&hash, &location)?;
                        chunk_locations.insert(hex::encode(hash.0), location);
                        tracing::debug!(hash = ?hash, "chunk already exists in repo, idempotent skip");
                    } else {
                        tracker.set_state(ExecutionState::Encrypting);
                        let compressed = self.compressor.compress(&chunk.data)?;
                        let chunk_id = ChunkId(Uuid::new_v4());
                        let encrypted = self.encryption.encrypt_chunk(&compressed, &chunk_id)?;

                        tracker.set_state(ExecutionState::Uploading);
                        let _guard = self.memory_budget.acquire(encrypted.ciphertext.len() as u64).await;
                        let location = self.repo.write_chunk(&hash, &encrypted)?;
                        data_stored += encrypted.ciphertext.len() as u64;

                        self.dedup.register_new(&hash, &location)?;
                        chunk_locations.insert(hex::encode(hash.0), location);
                    }
                } else if let Some(ref loc) = lookup[0].location {
                    chunk_locations.insert(hex::encode(hash.0), loc.clone());
                }

                chunk_count += 1;
                file_chunks.push(hash.clone());

                let chunk_ref = ChunkReference {
                    hash,
                    version_id: version_id.clone(),
                    file_path: file_entry.path.clone(),
                    offset: chunk.offset,
                };
                changed_chunk_refs.push(chunk_ref);
            }

            let file_hash: [u8; 32] = file_hasher.finalize().into();

            let completed_entry = FileEntry {
                path: file_entry.path,
                size: file_entry.size,
                modified_at: file_entry.modified_at,
                attributes: file_entry.attributes,
                chunks: file_chunks,
                file_hash,
            };
            changed_files.push(completed_entry);

            tracker.set_progress(changed_files.len() as f64 / total_files as f64);
        }

        let mut all_files: Vec<FileEntry> = Vec::new();
        let mut all_chunk_refs: Vec<ChunkReference> = Vec::new();
        let mut unchanged_count: u64 = 0;

        for prev_file in &prev_manifest.files {
            if changed_paths.contains(&prev_file.path) {
                continue;
            }
            all_files.push(prev_file.clone());
            for prev_cr in &prev_manifest.chunk_refs {
                if prev_cr.file_path == prev_file.path {
                    all_chunk_refs.push(ChunkReference {
                        hash: prev_cr.hash.clone(),
                        version_id: version_id.clone(),
                        file_path: prev_cr.file_path.clone(),
                        offset: prev_cr.offset,
                    });
                }
            }
            unchanged_count += 1;
        }

        let changed_count = changed_files.len() as u64;
        all_files.extend(changed_files);
        all_chunk_refs.extend(changed_chunk_refs);

        for (k, v) in &prev_manifest.chunk_locations {
            chunk_locations.entry(k.clone()).or_insert(v.clone());
        }

        tracker.set_state(ExecutionState::Committing);
        self.dedup.add_references(&all_chunk_refs)?;

        let manifest = self.build_manifest(
            &version_id,
            Some(baseline_version_id.clone()),
            prev_manifest.version_number + 1,
            BackupType::Incremental,
            all_files,
            all_chunk_refs,
            chunk_locations,
        )?;

        self.repo.write_manifest(&version_id, &manifest)?;

        tracker.set_state(ExecutionState::Success);

        let duration = start.elapsed();
        let file_count = changed_count + unchanged_count;
        let dedup_ratio = compute_dedup_ratio(data_processed, data_stored);

        tracing::info!(
            changed = changed_count,
            unchanged = unchanged_count,
            data_stored,
            "incremental backup completed"
        );

        Ok(BackupResult {
            version_id: Some(version_id),
            data_processed,
            data_stored,
            dedup_ratio,
            chunk_count,
            file_count,
            duration,
            skipped_files,
        })
    }

    pub fn read_processed_files_from_journal(
        &self,
        job_id: &hbx_core::domain::common::JobId,
    ) -> std::collections::HashSet<String> {
        let mut processed = std::collections::HashSet::new();
        if let Some(ref journal) = self.journal {
            if let Ok(entries) = journal.read_recent(10000) {
                for entry in entries {
                    if let JournalEntry::FileProcessed {
                        job_id: entry_job_id,
                        file_path,
                        ..
                    } = &entry
                    {
                        if entry_job_id == job_id {
                            processed.insert(file_path.clone());
                        }
                    }
                }
            }
        }
        processed
    }

    pub async fn run_backup_resumable(
        &self,
        job: &BackupJob,
        tracker: &ExecutionTracker,
    ) -> Result<BackupResult, EngineError> {
        let processed_files = self.read_processed_files_from_journal(&job.job_id);
        if !processed_files.is_empty() {
            tracing::info!(
                skipped = processed_files.len(),
                "resuming backup from checkpoint, skipping already processed files"
            );
        }

        let start = Instant::now();
        let version_id = VersionId(Uuid::new_v4());

        if let Some(ref journal) = self.journal {
            let _ = journal.append(JournalEntry::TaskStarted {
                job_id: job.job_id.clone(),
                execution_id: tracker.snapshot().execution_id,
                timestamp: chrono::Utc::now().timestamp() as u64,
            });
        }

        tracker.set_state(ExecutionState::Scanning);
        let filter = FilterRule::Glob("*".to_string());
        let file_stream = self.scanner.scan(&job.source, &filter, None)?;

        let estimate = self.scanner.estimate(&job.source, &filter);
        let total_files = estimate.total_files.max(1);

        tracker.set_state(ExecutionState::Chunking);

        let mut files: Vec<FileEntry> = Vec::new();
        let mut chunk_refs: Vec<ChunkReference> = Vec::new();
        let mut chunk_locations: std::collections::HashMap<String, ChunkLocation> = std::collections::HashMap::new();
        let mut data_processed: u64 = 0;
        let mut data_stored: u64 = 0;
        let mut chunk_count: u64 = 0;
        let mut file_count: u64 = 0;
        let mut skipped_files: Vec<PathBuf> = Vec::new();
        let mut resumed_count: u64 = 0;

        tokio::pin!(file_stream);
        while let Some(file_entry) = file_stream.next().await {
            if processed_files.contains(&file_entry.path) {
                resumed_count += 1;
                tracing::debug!(path = %file_entry.path, "skipping already processed file (resume)");
                continue;
            }

            let path = PathBuf::from(&file_entry.path);
            let file = match tokio::fs::File::open(&path).await {
                Ok(f) => f,
                Err(e) => {
                    tracing::warn!(path = %file_entry.path, error = %e, "failed to open file, skipping");
                    skipped_files.push(path);
                    continue;
                }
            };

            let reader: Box<dyn tokio::io::AsyncRead + Send + Unpin> = Box::new(file);
            let chunk_stream = self.chunker.chunk(reader, self.chunk_strategy)?;

            let mut file_hasher = Sha256::new();
            let mut file_chunks: Vec<ChunkHash> = Vec::new();

            tracker.set_state(ExecutionState::Chunking);
            tokio::pin!(chunk_stream);
            while let Some(chunk) = chunk_stream.next().await {
                file_hasher.update(&chunk.data);
                data_processed += chunk.data.len() as u64;

                let hash = compute_chunk_hash(&chunk.data);

                let lookup = self.dedup.batch_lookup(std::slice::from_ref(&hash))?;
                let is_new = !lookup[0].exists;

                if is_new {
                    if self.repo.chunk_exists(&hash)? {
                        let location = self.repo.find_chunk(&hash)?;
                        self.dedup.register_new(&hash, &location)?;
                        chunk_locations.insert(hex::encode(hash.0), location);
                    } else {
                        tracker.set_state(ExecutionState::Encrypting);
                        let compressed = self.compressor.compress(&chunk.data)?;
                        let chunk_id = ChunkId(Uuid::new_v4());
                        let encrypted = self.encryption.encrypt_chunk(&compressed, &chunk_id)?;

                        tracker.set_state(ExecutionState::Uploading);
                        let _guard = self.memory_budget.acquire(encrypted.ciphertext.len() as u64).await;
                        let location = self.repo.write_chunk(&hash, &encrypted)?;
                        data_stored += encrypted.ciphertext.len() as u64;

                        self.dedup.register_new(&hash, &location)?;
                        chunk_locations.insert(hex::encode(hash.0), location);
                    }
                } else if let Some(ref loc) = lookup[0].location {
                    chunk_locations.insert(hex::encode(hash.0), loc.clone());
                }

                chunk_count += 1;
                file_chunks.push(hash.clone());

                let chunk_ref = ChunkReference {
                    hash,
                    version_id: version_id.clone(),
                    file_path: file_entry.path.clone(),
                    offset: chunk.offset,
                };
                chunk_refs.push(chunk_ref);
            }

            let file_hash: [u8; 32] = file_hasher.finalize().into();

            let completed_entry = FileEntry {
                path: file_entry.path,
                size: file_entry.size,
                modified_at: file_entry.modified_at,
                attributes: file_entry.attributes,
                chunks: file_chunks,
                file_hash,
            };
            files.push(completed_entry);
            file_count += 1;

            let progress = (file_count + resumed_count) as f64 / total_files as f64;
            tracker.set_progress(progress);

            if let Some(ref journal) = self.journal {
                let _ = journal.append(JournalEntry::FileProcessed {
                    job_id: job.job_id.clone(),
                    file_path: files.last().unwrap().path.clone(),
                    chunks: files.last().unwrap().chunks.clone(),
                });
                let _ = journal.append(JournalEntry::Checkpoint {
                    job_id: job.job_id.clone(),
                    progress,
                    pending_files: (total_files - file_count - resumed_count) as usize,
                });
            }
        }

        tracker.set_state(ExecutionState::Committing);
        self.dedup.add_references(&chunk_refs)?;

        let manifest = self.build_manifest(
            &version_id,
            None,
            1,
            BackupType::Full,
            files,
            chunk_refs,
            chunk_locations,
        )?;

        self.repo.write_manifest(&version_id, &manifest)?;

        if let Some(ref journal) = self.journal {
            let result = BackupResult {
                version_id: Some(version_id.clone()),
                data_processed,
                data_stored,
                dedup_ratio: compute_dedup_ratio(data_processed, data_stored),
                chunk_count,
                file_count,
                duration: start.elapsed(),
                skipped_files: skipped_files.clone(),
            };
            let _ = journal.append(JournalEntry::TaskCompleted {
                job_id: job.job_id.clone(),
                version_id: version_id.clone(),
                result,
            });
        }

        tracker.set_state(ExecutionState::Success);

        let duration = start.elapsed();
        let dedup_ratio = compute_dedup_ratio(data_processed, data_stored);

        tracing::info!(
            new_files = file_count,
            resumed_files = resumed_count,
            data_stored,
            "resumable backup completed"
        );

        Ok(BackupResult {
            version_id: Some(version_id),
            data_processed,
            data_stored,
            dedup_ratio,
            chunk_count,
            file_count,
            duration,
            skipped_files,
        })
    }

    pub async fn run_backup_concurrent(
        &self,
        job: &BackupJob,
        tracker: &ExecutionTracker,
    ) -> Result<BackupResult, EngineError> {
        let _lock = BackupLockGuard::acquire(self.repo.clone(), Duration::from_secs(1800))
            .map_err(|e| EngineError::Failed(format!("failed to acquire backup lock: {e}")))?;

        let start = Instant::now();
        let version_id = VersionId(Uuid::new_v4());
        let mut staging = StagingTracker::new();

        if let Some(ref journal) = self.journal {
            let _ = journal.append(JournalEntry::TaskStarted {
                job_id: job.job_id.clone(),
                execution_id: tracker.snapshot().execution_id,
                timestamp: chrono::Utc::now().timestamp() as u64,
            });
        }

        tracker.set_state(ExecutionState::Scanning);
        let filter = FilterRule::Glob("*".to_string());
        let file_stream = self.scanner.scan(&job.source, &filter, None)?;

        let estimate = self.scanner.estimate(&job.source, &filter);
        let total_files = estimate.total_files.max(1);

        tracker.set_state(ExecutionState::Chunking);

        let mut files: Vec<FileEntry> = Vec::new();
        let mut chunk_refs: Vec<ChunkReference> = Vec::new();
        let mut chunk_locations: std::collections::HashMap<String, ChunkLocation> = std::collections::HashMap::new();
        let mut data_processed: u64 = 0;
        let mut data_stored: u64 = 0;
        let mut chunk_count: u64 = 0;
        let mut file_count: u64 = 0;
        let mut skipped_files: Vec<PathBuf> = Vec::new();

        tokio::pin!(file_stream);
        while let Some(file_entry) = file_stream.next().await {
            let path = PathBuf::from(&file_entry.path);
            let file = match tokio::fs::File::open(&path).await {
                Ok(f) => f,
                Err(e) => {
                    tracing::warn!(path = %file_entry.path, error = %e, "failed to open file, skipping");
                    skipped_files.push(path);
                    continue;
                }
            };

            let reader: Box<dyn tokio::io::AsyncRead + Send + Unpin> = Box::new(file);
            let chunk_stream = self.chunker.chunk(reader, self.chunk_strategy)?;

            let mut file_hasher = Sha256::new();
            let mut file_chunks: Vec<ChunkHash> = Vec::new();

            tracker.set_state(ExecutionState::Chunking);
            tokio::pin!(chunk_stream);
            while let Some(chunk) = chunk_stream.next().await {
                file_hasher.update(&chunk.data);
                data_processed += chunk.data.len() as u64;

                let hash = compute_chunk_hash(&chunk.data);
                let lookup = self.dedup.batch_lookup(std::slice::from_ref(&hash))?;
                let is_new = !lookup[0].exists;

                if is_new {
                    if self.repo.chunk_exists(&hash)? {
                        let location = self.repo.find_chunk(&hash)?;
                        self.dedup.register_new(&hash, &location)?;
                        chunk_locations.insert(hex::encode(hash.0), location);
                    } else {
                        tracker.set_state(ExecutionState::Encrypting);
                        let compressed = self.compressor.compress(&chunk.data)?;
                        let chunk_id = ChunkId(Uuid::new_v4());
                        let encrypted = self.encryption.encrypt_chunk(&compressed, &chunk_id)?;

                        tracker.set_state(ExecutionState::Uploading);
                        let _guard = self.memory_budget.acquire(encrypted.ciphertext.len() as u64).await;
                        let location = match self.repo.write_chunk(&hash, &encrypted) {
                            Ok(loc) => loc,
                            Err(e) => {
                                if is_storage_full(&e) {
                                    tracing::error!(error = %e, "storage full, rolling back staging chunks");
                                    let rollback = staging.rollback(self.repo.as_ref());
                                    tracing::info!(
                                        deleted = rollback.chunks_deleted,
                                        failed = rollback.chunks_failed,
                                        "staging rollback completed"
                                    );
                                }
                                tracker.set_state(ExecutionState::Failed);
                                return Err(EngineError::Repo(e));
                            }
                        };
                        data_stored += encrypted.ciphertext.len() as u64;
                        staging.track(hash.clone(), location.clone());

                        self.dedup.register_new(&hash, &location)?;
                        chunk_locations.insert(hex::encode(hash.0), location);
                    }
                } else if let Some(ref loc) = lookup[0].location {
                    chunk_locations.insert(hex::encode(hash.0), loc.clone());
                }

                chunk_count += 1;
                file_chunks.push(hash.clone());

                let chunk_ref = ChunkReference {
                    hash,
                    version_id: version_id.clone(),
                    file_path: file_entry.path.clone(),
                    offset: chunk.offset,
                };
                chunk_refs.push(chunk_ref);
            }

            let file_hash: [u8; 32] = file_hasher.finalize().into();

            let completed_entry = FileEntry {
                path: file_entry.path,
                size: file_entry.size,
                modified_at: file_entry.modified_at,
                attributes: file_entry.attributes,
                chunks: file_chunks,
                file_hash,
            };
            files.push(completed_entry);
            file_count += 1;

            tracker.set_progress(file_count as f64 / total_files as f64);

            if let Some(ref journal) = self.journal {
                let _ = journal.append(JournalEntry::FileProcessed {
                    job_id: job.job_id.clone(),
                    file_path: files.last().unwrap().path.clone(),
                    chunks: files.last().unwrap().chunks.clone(),
                });
            }
        }

        tracker.set_state(ExecutionState::Committing);
        self.dedup.add_references(&chunk_refs)?;

        let manifest = self.build_manifest(
            &version_id,
            None,
            1,
            BackupType::Full,
            files,
            chunk_refs,
            chunk_locations,
        )?;

        self.repo.write_manifest(&version_id, &manifest)?;

        if let Some(ref journal) = self.journal {
            let result = BackupResult {
                version_id: Some(version_id.clone()),
                data_processed,
                data_stored,
                dedup_ratio: compute_dedup_ratio(data_processed, data_stored),
                chunk_count,
                file_count,
                duration: start.elapsed(),
                skipped_files: skipped_files.clone(),
            };
            let _ = journal.append(JournalEntry::TaskCompleted {
                job_id: job.job_id.clone(),
                version_id: version_id.clone(),
                result,
            });
        }

        tracker.set_state(ExecutionState::Success);

        let duration = start.elapsed();
        let dedup_ratio = compute_dedup_ratio(data_processed, data_stored);

        tracing::info!(
            staging_chunks = staging.len(),
            "concurrent backup completed, lock will be released on drop"
        );

        Ok(BackupResult {
            version_id: Some(version_id),
            data_processed,
            data_stored,
            dedup_ratio,
            chunk_count,
            file_count,
            duration,
            skipped_files,
        })
    }

    pub fn cleanup_orphan_chunks(
        &self,
        chunk_locations: &[ChunkLocation],
    ) -> Result<u32, EngineError> {
        let mut deleted = 0;
        for location in chunk_locations {
            match self.repo.delete_chunk(location) {
                Ok(()) => deleted += 1,
                Err(e) => {
                    tracing::warn!(location = ?location, error = %e, "failed to delete orphan chunk");
                }
            }
        }
        tracing::info!(deleted, total = chunk_locations.len(), "orphan chunk cleanup completed");
        Ok(deleted)
    }

    fn build_manifest(
        &self,
        version_id: &VersionId,
        parent_version_id: Option<VersionId>,
        version_number: u64,
        backup_type: BackupType,
        files: Vec<FileEntry>,
        chunk_refs: Vec<ChunkReference>,
        chunk_locations: std::collections::HashMap<String, hbx_core::domain::chunk::ChunkLocation>,
    ) -> Result<Manifest, EngineError> {
        let mut file_index_hasher = Sha256::new();
        for file in &files {
            file_index_hasher.update(file.file_hash);
        }
        let file_index_hash: [u8; 32] = file_index_hasher.finalize().into();

        let mut chunk_index_hasher = Sha256::new();
        for cr in &chunk_refs {
            chunk_index_hasher.update(cr.hash.0);
        }
        let chunk_index_hash: [u8; 32] = chunk_index_hasher.finalize().into();

        let placeholder_hashes = ManifestHashes {
            manifest_hash: [0u8; 32],
            file_index_hash,
            chunk_index_hash,
            repo_hash: [0u8; 32],
        };

        let temp_manifest = Manifest {
            version_id: version_id.clone(),
            timestamp: chrono::Utc::now(),
            parent_version_id: parent_version_id.clone(),
            version_number,
            backup_type,
            files: files.clone(),
            chunk_refs: chunk_refs.clone(),
            hashes: placeholder_hashes.clone(),
            chunk_locations: chunk_locations.clone(),
        };
        let manifest_bytes = serde_json::to_vec(&temp_manifest)?;
        let manifest_hash: [u8; 32] = {
            let mut h = Sha256::new();
            h.update(&manifest_bytes);
            h.finalize().into()
        };

        let mut repo_hasher = Sha256::new();
        repo_hasher.update(manifest_hash);
        repo_hasher.update(file_index_hash);
        repo_hasher.update(chunk_index_hash);
        let repo_hash: [u8; 32] = repo_hasher.finalize().into();

        Ok(Manifest {
            version_id: version_id.clone(),
            timestamp: temp_manifest.timestamp,
            parent_version_id,
            version_number,
            backup_type,
            files,
            chunk_refs,
            hashes: ManifestHashes {
                manifest_hash,
                file_index_hash,
                chunk_index_hash,
                repo_hash,
            },
            chunk_locations,
        })
    }
}

fn compute_chunk_hash(data: &[u8]) -> ChunkHash {
    let hash = blake3::hash(data);
    ChunkHash(*hash.as_bytes())
}

fn compute_dedup_ratio(processed: u64, stored: u64) -> f64 {
    if processed == 0 {
        return 0.0;
    }
    1.0 - (stored as f64 / processed as f64)
}

#[derive(Default)]
pub struct BackupEngineBuilder {
    scanner: Option<Arc<dyn IScanner>>,
    chunker: Option<Arc<dyn IChunker>>,
    dedup: Option<Arc<dyn IDedupIndex>>,
    compressor: Option<Arc<dyn ICompressor>>,
    encryption: Option<Arc<dyn IEncryptionProvider>>,
    repo: Option<Arc<dyn IBackupRepository>>,
    journal: Option<Arc<dyn IJournal>>,
    memory_limit: Option<u64>,
    chunk_strategy: Option<ChunkStrategy>,
}

impl BackupEngineBuilder {
    pub fn scanner(mut self, scanner: impl IScanner + 'static) -> Self {
        self.scanner = Some(Arc::new(scanner));
        self
    }

    pub fn chunker(mut self, chunker: impl IChunker + 'static) -> Self {
        self.chunker = Some(Arc::new(chunker));
        self
    }

    pub fn dedup(mut self, dedup: impl IDedupIndex + 'static) -> Self {
        self.dedup = Some(Arc::new(dedup));
        self
    }

    pub fn compressor(mut self, compressor: impl ICompressor + 'static) -> Self {
        self.compressor = Some(Arc::new(compressor));
        self
    }

    pub fn encryption(mut self, encryption: impl IEncryptionProvider + 'static) -> Self {
        self.encryption = Some(Arc::new(encryption));
        self
    }

    pub fn repo(mut self, repo: impl IBackupRepository + 'static) -> Self {
        self.repo = Some(Arc::new(repo));
        self
    }

    pub fn journal(mut self, journal: impl IJournal + 'static) -> Self {
        self.journal = Some(Arc::new(journal));
        self
    }

    pub fn memory_limit(mut self, limit: u64) -> Self {
        self.memory_limit = Some(limit);
        self
    }

    pub fn chunk_strategy(mut self, strategy: ChunkStrategy) -> Self {
        self.chunk_strategy = Some(strategy);
        self
    }

    pub fn build(self) -> Result<BackupEngine, EngineError> {
        Ok(BackupEngine {
            scanner: self.scanner.ok_or_else(|| EngineError::Failed("scanner not set".into()))?,
            chunker: self.chunker.ok_or_else(|| EngineError::Failed("chunker not set".into()))?,
            dedup: self.dedup.ok_or_else(|| EngineError::Failed("dedup not set".into()))?,
            compressor: self.compressor.ok_or_else(|| EngineError::Failed("compressor not set".into()))?,
            encryption: self.encryption.ok_or_else(|| EngineError::Failed("encryption not set".into()))?,
            repo: self.repo.ok_or_else(|| EngineError::Failed("repo not set".into()))?,
            journal: self.journal,
            memory_budget: MemoryBudget::new(self.memory_limit.unwrap_or(4 * 1024 * 1024 * 1024)),
            chunk_strategy: self.chunk_strategy.unwrap_or(ChunkStrategy::Fixed {
                chunk_size: 4 * 1024 * 1024,
            }),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hbx_chunker::FixedChunker;
    use hbx_compress::ZstdCompressor;
    use hbx_core::domain::backup::{BackupDestination, BackupSource, JobStatus};
    use hbx_core::domain::common::{
        CompressionAlgorithm, CompressionProfile, EncryptionProfileRef, JobId,
        RetentionPolicyRef, ScheduleRef,
    };
    use hbx_core::domain::repository::BackendType;
    use hbx_dedup::LocalDedupIndex;
    use hbx_repo::{LocalRepository, RepositoryInitializer};
    use hbx_scanner::LocalScanner;
    use std::fs;
    use uuid::Uuid;

    use crate::crypto::NoOpEncryptionProvider;

    fn make_job(source_path: PathBuf) -> BackupJob {
        BackupJob {
            job_id: JobId(Uuid::new_v4()),
            name: "test-backup".to_string(),
            source: BackupSource {
                paths: vec![source_path],
                include_rules: vec![],
                exclude_rules: vec![],
            },
            destination: BackupDestination {
                repository_id: hbx_core::domain::common::RepositoryId(Uuid::new_v4()),
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

    fn setup_engine(repo_path: &std::path::Path) -> BackupEngine {
        RepositoryInitializer::new(repo_path)
            .init(
                hbx_core::domain::common::RepositoryId(Uuid::new_v4()),
                BackendType::Local,
            )
            .unwrap();
        let repo = LocalRepository::open(repo_path).unwrap();

        BackupEngine::builder()
            .scanner(LocalScanner::new())
            .chunker(FixedChunker::new())
            .dedup(LocalDedupIndex::new())
            .compressor(ZstdCompressor::default())
            .encryption(NoOpEncryptionProvider)
            .repo(repo)
            .memory_limit(256 * 1024 * 1024)
            .chunk_strategy(ChunkStrategy::Fixed { chunk_size: 1024 })
            .build()
            .unwrap()
    }

    #[tokio::test]
    async fn test_backup_small_files() {
        let src_dir = tempfile::tempdir().unwrap();
        let repo_dir = tempfile::tempdir().unwrap();

        fs::write(src_dir.path().join("a.txt"), b"hello world").unwrap();
        fs::write(src_dir.path().join("b.txt"), b"foo bar baz").unwrap();
        fs::create_dir(src_dir.path().join("sub")).unwrap();
        fs::write(src_dir.path().join("sub").join("c.txt"), b"nested content").unwrap();

        let job = make_job(src_dir.path().to_path_buf());
        let engine = setup_engine(repo_dir.path());
        let tracker = engine.execution_tracker(&job.job_id);

        let result = engine.run_backup(&job, &tracker).await.unwrap();

        assert_eq!(result.file_count, 3);
        assert!(result.chunk_count > 0);
        assert!(result.data_processed > 0);
        assert!(result.version_id.is_some());

        let exec = tracker.snapshot();
        assert_eq!(exec.state, ExecutionState::Success);
        assert_eq!(exec.progress, 1.0);
        assert!(exec.completed_at.is_some());
    }

    #[tokio::test]
    async fn test_backup_empty_dir() {
        let src_dir = tempfile::tempdir().unwrap();
        let repo_dir = tempfile::tempdir().unwrap();

        let job = make_job(src_dir.path().to_path_buf());
        let engine = setup_engine(repo_dir.path());
        let tracker = engine.execution_tracker(&job.job_id);

        let result = engine.run_backup(&job, &tracker).await.unwrap();

        assert_eq!(result.file_count, 0);
        assert_eq!(result.chunk_count, 0);
        assert_eq!(result.data_processed, 0);

        let exec = tracker.snapshot();
        assert_eq!(exec.state, ExecutionState::Success);
    }

    #[tokio::test]
    async fn test_backup_dedup() {
        let src_dir = tempfile::tempdir().unwrap();
        let repo_dir = tempfile::tempdir().unwrap();

        let content = vec![0xabu8; 8192];
        fs::write(src_dir.path().join("file1.bin"), &content).unwrap();
        fs::write(src_dir.path().join("file2.bin"), &content).unwrap();

        let job = make_job(src_dir.path().to_path_buf());
        let engine = setup_engine(repo_dir.path());
        let tracker = engine.execution_tracker(&job.job_id);

        let result = engine.run_backup(&job, &tracker).await.unwrap();

        assert_eq!(result.file_count, 2);
        assert!(result.dedup_ratio > 0.0, "dedup ratio should be positive: {}", result.dedup_ratio);
        assert!(result.data_stored < result.data_processed, "stored should be less than processed");
    }

    #[tokio::test]
    async fn test_state_machine_transitions() {
        let src_dir = tempfile::tempdir().unwrap();
        let repo_dir = tempfile::tempdir().unwrap();

        fs::write(src_dir.path().join("test.txt"), b"test content").unwrap();

        let job = make_job(src_dir.path().to_path_buf());
        let engine = setup_engine(repo_dir.path());
        let tracker = engine.execution_tracker(&job.job_id);

        assert_eq!(tracker.snapshot().state, ExecutionState::Pending);

        engine.run_backup(&job, &tracker).await.unwrap();

        let exec = tracker.snapshot();
        assert_eq!(exec.state, ExecutionState::Success);
        assert!(exec.progress >= 1.0);
    }

    #[tokio::test]
    async fn test_memory_budget_respected() {
        let src_dir = tempfile::tempdir().unwrap();
        let repo_dir = tempfile::tempdir().unwrap();

        let content = vec![0xcdu8; 100_000];
        fs::write(src_dir.path().join("large.bin"), &content).unwrap();

        let job = make_job(src_dir.path().to_path_buf());
        let engine = setup_engine(repo_dir.path());
        let tracker = engine.execution_tracker(&job.job_id);

        engine.run_backup(&job, &tracker).await.unwrap();

        assert!(engine.memory_used() <= engine.memory_limit());
    }
}