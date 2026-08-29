//! 七种核心对象领域模型与聚合根。

pub mod repository;
pub mod version;
pub mod snapshot;
pub mod manifest;
pub mod chunk;
pub mod index;
pub mod journal;

pub use repository::{Repository, RepoStatus, RepoConfig};
pub use version::{Version, VersionStatus};
pub use snapshot::{Snapshot, SnapshotStatus};
pub use manifest::Manifest;
pub use chunk::{BadouChunk, ChunkStatus};
pub use index::Index;
pub use journal::{Journal, JournalOp};

pub use snapshot::{
    SourceMachine, BackupPolicy, EncryptionInfo, CompressionInfo,
    VerifyInfo, FileTree, ChunkMapping, KeyRef,
};