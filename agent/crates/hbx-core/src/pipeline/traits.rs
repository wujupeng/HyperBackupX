use std::time::Duration;

use futures::stream::Stream;
use thiserror::Error;
use tokio::io::AsyncRead;

use crate::domain::backup::{BackupError, BackupResult, BackupSnapshot, BackupSource};
use crate::domain::chunk::{ChunkHash, ChunkId, ChunkLocation, ChunkReference};
use crate::domain::common::{
    Checkpoint, ExecutionId, FilterRule, JobId, LockOperation, RepoLock, ScanEstimate, VersionId,
    VersionSummary,
};
use crate::domain::encryption::{DerivedKey, EncryptedChunk, EncryptionProfile};
use crate::domain::repository::{FileEntry, Manifest};
use crate::domain::schedule::RetentionPolicy;
use crate::domain::verify::{VerifyMode, VerifyReport};


#[derive(Debug, Error)]
pub enum ScanError {
    #[error("source inaccessible: {0}")]
    SourceInaccessible(String),
    #[error("permission denied: {0}")]
    PermissionDenied(String),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

#[derive(Debug, Error)]
pub enum ChunkError {
    #[error("read error: {0}")]
    ReadError(String),
    #[error("invalid chunk size: {0}")]
    InvalidSize(u64),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

#[derive(Debug, Error)]
pub enum IndexError {
    #[error("index corrupted")]
    Corrupted,
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

#[derive(Debug, Error)]
pub enum CompressError {
    #[error("compression failed: {0}")]
    Failed(String),
    #[error("decompression failed: {0}")]
    DecompressFailed(String),
}

#[derive(Debug, Error)]
pub enum EncryptError {
    #[error("key unavailable")]
    KeyUnavailable,
    #[error("authentication failed")]
    AuthFailed,
    #[error("encryption failed: {0}")]
    Failed(String),
}

#[derive(Debug, Error)]
pub enum KeyError {
    #[error("key source unavailable: {0}")]
    Unavailable(String),
    #[error("invalid credential")]
    InvalidCredential,
}

#[derive(Debug, Error)]
pub enum RepoError {
    #[error("repository auth failed")]
    AuthFailed,
    #[error("repository full")]
    Full,
    #[error("lock timeout")]
    LockTimeout,
    #[error("not found: {0}")]
    NotFound(String),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("serialization error: {0}")]
    Serialize(#[from] serde_json::Error),
    #[error("failed: {0}")]
    Failed(String),
}

#[derive(Debug, Error)]
pub enum JournalError {
    #[error("journal write failed: {0}")]
    WriteFailed(String),
    #[error("journal corrupted")]
    Corrupted,
    #[error("serialization error: {0}")]
    Serialize(#[from] serde_json::Error),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

#[derive(Debug, Error)]
pub enum RetentionError {
    #[error("invalid policy: {0}")]
    InvalidPolicy(String),
}

#[derive(Debug, Error)]
pub enum VerifyError {
    #[error("verification failed: {0}")]
    Failed(String),
    #[error("repository error: {0}")]
    Repo(#[from] RepoError),
    #[error("encrypt error: {0}")]
    Encrypt(#[from] EncryptError),
    #[error("compress error: {0}")]
    Compress(#[from] CompressError),
}

#[derive(Debug, Clone, Copy)]
pub enum ChunkStrategy {
    Fixed { chunk_size: u64 },
    Cdc { min_size: u64, avg_size: u64, max_size: u64 },
}

#[derive(Debug, Clone)]
pub struct RawChunkData {
    pub offset: u64,
    pub data: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct DedupLookupResult {
    pub hash: ChunkHash,
    pub exists: bool,
    pub reference_count: u64,
    pub location: Option<ChunkLocation>,
}

#[derive(Debug, Clone)]
pub struct ZeroizingKey(pub Vec<u8>);

impl Drop for ZeroizingKey {
    fn drop(&mut self) {
        for byte in self.0.iter_mut() {
            *byte = 0;
        }
    }
}

#[derive(Debug, Clone)]
pub enum JournalEntry {
    TaskStarted {
        job_id: JobId,
        execution_id: ExecutionId,
        timestamp: u64,
    },
    FileProcessed {
        job_id: JobId,
        file_path: String,
        chunks: Vec<ChunkHash>,
    },
    ChunkWritten {
        job_id: JobId,
        hash: ChunkHash,
        location: ChunkLocation,
    },
    Checkpoint {
        job_id: JobId,
        progress: f64,
        pending_files: usize,
    },
    TaskCompleted {
        job_id: JobId,
        version_id: VersionId,
        result: BackupResult,
    },
    TaskFailed {
        job_id: JobId,
        error: BackupError,
    },
}

#[derive(Debug, Clone)]
pub struct RetentionDecision {
    pub keep: Vec<VersionId>,
    pub delete: Vec<VersionId>,
}

pub trait IScanner: Send + Sync {
    fn scan(
        &self,
        source: &BackupSource,
        filter: &FilterRule,
        baseline: Option<&BackupSnapshot>,
    ) -> Result<Box<dyn Stream<Item = FileEntry> + Send + Unpin>, ScanError>;

    fn estimate(&self, source: &BackupSource, filter: &FilterRule) -> ScanEstimate;
}

pub trait IChunker: Send + Sync {
    fn chunk(
        &self,
        reader: Box<dyn AsyncRead + Send + Unpin>,
        strategy: ChunkStrategy,
    ) -> Result<Box<dyn Stream<Item = RawChunkData> + Send + Unpin>, ChunkError>;
}

pub trait IDedupIndex: Send + Sync {
    fn batch_lookup(
        &self,
        hashes: &[ChunkHash],
    ) -> Result<Vec<DedupLookupResult>, IndexError>;

    fn register_new(
        &self,
        hash: &ChunkHash,
        location: &ChunkLocation,
    ) -> Result<(), IndexError>;

    fn add_references(
        &self,
        references: &[ChunkReference],
    ) -> Result<(), IndexError>;

    fn remove_references(
        &self,
        references: &[ChunkReference],
    ) -> Result<Vec<ChunkHash>, IndexError>;
}

pub trait ICompressor: Send + Sync {
    fn compress(&self, plain: &[u8]) -> Result<Vec<u8>, CompressError>;
    fn decompress(&self, compressed: &[u8]) -> Result<Vec<u8>, CompressError>;
}

pub trait IEncryptionProvider: Send + Sync {
    fn encrypt_chunk(
        &self,
        plain: &[u8],
        chunk_id: &ChunkId,
    ) -> Result<EncryptedChunk, EncryptError>;

    fn decrypt_chunk(
        &self,
        encrypted: &EncryptedChunk,
    ) -> Result<Vec<u8>, EncryptError>;

    fn derive_key(
        &self,
        password: &str,
        salt: &[u8],
        profile: &EncryptionProfile,
    ) -> Result<DerivedKey, EncryptError>;
}

pub trait IKeySource: Send + Sync {
    fn acquire_key(
        &self,
        profile: &EncryptionProfile,
    ) -> Result<ZeroizingKey, KeyError>;

    fn release_key(&self, key: ZeroizingKey);
}

pub trait IBackupRepository: Send + Sync {
    fn write_chunk(
        &self,
        hash: &ChunkHash,
        encrypted: &EncryptedChunk,
    ) -> Result<ChunkLocation, RepoError>;

    fn read_chunk(
        &self,
        location: &ChunkLocation,
    ) -> Result<EncryptedChunk, RepoError>;

    fn chunk_exists(&self, hash: &ChunkHash) -> Result<bool, RepoError>;

    fn find_chunk(&self, _hash: &ChunkHash) -> Result<ChunkLocation, RepoError> {
        Err(RepoError::Failed("find_chunk not implemented".into()))
    }

    fn delete_chunk(&self, location: &ChunkLocation) -> Result<(), RepoError>;

    fn write_manifest(
        &self,
        version_id: &VersionId,
        manifest: &Manifest,
    ) -> Result<(), RepoError>;

    fn read_manifest(
        &self,
        version_id: &VersionId,
    ) -> Result<Manifest, RepoError>;

    fn list_versions(&self) -> Result<Vec<VersionSummary>, RepoError>;

    fn acquire_lock(
        &self,
        operation: LockOperation,
        timeout: Duration,
    ) -> Result<RepoLock, RepoError>;
}

pub trait IJournal: Send + Sync {
    fn append(&self, entry: JournalEntry) -> Result<(), JournalError>;
    fn read_recent(&self, n: usize) -> Result<Vec<JournalEntry>, JournalError>;
    fn read_checkpoint(&self, job_id: &JobId) -> Result<Option<Checkpoint>, JournalError>;
    fn rotate(&self) -> Result<(), JournalError>;
}

pub trait IRetentionPolicyExecutor: Send + Sync {
    fn compute(
        &self,
        versions: &[VersionSummary],
        policy: &RetentionPolicy,
    ) -> Result<RetentionDecision, RetentionError>;
}

pub trait IIntegrityVerifier: Send + Sync {
    fn verify(
        &self,
        version_id: &VersionId,
        mode: VerifyMode,
        repo: &dyn IBackupRepository,
    ) -> Result<VerifyReport, VerifyError>;
}