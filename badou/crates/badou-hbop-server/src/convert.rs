//! Proto ↔ 领域类型转换辅助。

use badou_proto::{
    RepositoryConfig as PbRepoConfig, RepositoryInfo as PbRepoInfo,
    RepoStatus as PbRepoStatus, SnapshotMeta as PbSnapshotMeta,
    ManifestData as PbManifestData, ChunkRef as PbChunkRef,
    ChunkData as PbChunkData, ChunkInfo as PbChunkInfo,
    VersionInfo as PbVersionInfo, VersionStatus as PbVersionStatus,
    SnapshotStatus as PbSnapshotStatus, ChunkStatus as PbChunkStatus,
    VerifyReport as PbVerifyReport, RecoveryChunk as PbRecoveryChunk,
    GcReport as PbGcReport,
};
use badou_engine::domain::repository::{RepoConfig, RepoStatus};
use badou_engine::domain::snapshot::{Snapshot, SnapshotStatus};
use badou_engine::domain::version::{Version, VersionStatus};
use badou_engine::domain::manifest::Manifest;
use hbx_core::domain::chunk::ChunkHash;
use hbx_core::domain::common::{RepositoryId, VersionId};
use uuid::Uuid;
use chrono::{DateTime, Utc};

/// prost_types::Timestamp → chrono DateTime
pub fn ts_to_chrono(ts: &prost_types::Timestamp) -> DateTime<Utc> {
    DateTime::from_timestamp(ts.seconds, ts.nanos as u32)
        .unwrap_or_else(Utc::now)
}

/// chrono DateTime → prost_types::Timestamp
pub fn chrono_to_ts(dt: &DateTime<Utc>) -> prost_types::Timestamp {
    prost_types::Timestamp {
        seconds: dt.timestamp(),
        nanos: dt.timestamp_subsec_nanos() as i32,
    }
}

/// hex string → ChunkHash
pub fn parse_hash(hex_str: &str) -> Result<ChunkHash, String> {
    let bytes = hex::decode(hex_str).map_err(|e| format!("invalid hex: {}", e))?;
    if bytes.len() != 32 {
        return Err(format!("expected 32 bytes, got {}", bytes.len()));
    }
    let mut arr = [0u8; 32];
    arr.copy_from_slice(&bytes);
    Ok(ChunkHash(arr))
}

/// string → RepositoryId
pub fn parse_repo_id(s: &str) -> Result<RepositoryId, String> {
    let uuid = s.parse::<Uuid>().map_err(|e| format!("invalid repo_id: {}", e))?;
    Ok(RepositoryId(uuid))
}

/// string → VersionId
pub fn parse_version_id(s: &str) -> Result<VersionId, String> {
    let uuid = s.parse::<Uuid>().map_err(|e| format!("invalid version_id: {}", e))?;
    Ok(VersionId(uuid))
}

/// PbRepoConfig → RepoConfig
pub fn pb_to_repo_config(pb: &PbRepoConfig) -> RepoConfig {
    RepoConfig {
        name: pb.name.clone(),
        immutable: pb.immutable.unwrap_or(false),
        immutable_until: pb.immutable_until.as_ref().map(ts_to_chrono),
        options: pb.options.clone(),
    }
}

/// RepoStatus → PbRepoStatus
pub fn repo_status_to_pb(status: RepoStatus) -> PbRepoStatus {
    match status {
        RepoStatus::Active => PbRepoStatus::RepoActive,
        RepoStatus::Readonly => PbRepoStatus::RepoReadonly,
        RepoStatus::Deleted => PbRepoStatus::RepoDeleted,
        RepoStatus::Immutable => PbRepoStatus::RepoImmutable,
    }
}

/// VersionStatus → PbVersionStatus
pub fn version_status_to_pb(status: VersionStatus) -> PbVersionStatus {
    match status {
        VersionStatus::Created => PbVersionStatus::Created,
        VersionStatus::Writing => PbVersionStatus::Writing,
        VersionStatus::Verifying => PbVersionStatus::Verifying,
        VersionStatus::Committing => PbVersionStatus::Committing,
        VersionStatus::Sealed => PbVersionStatus::Sealed,
        VersionStatus::Expired => PbVersionStatus::Expired,
        VersionStatus::Deleted => PbVersionStatus::Deleted,
        VersionStatus::GcPending => PbVersionStatus::GcPending,
        VersionStatus::Purged => PbVersionStatus::Purged,
    }
}

/// SnapshotStatus → PbSnapshotStatus
pub fn snapshot_status_to_pb(status: SnapshotStatus) -> PbSnapshotStatus {
    match status {
        SnapshotStatus::Created => PbSnapshotStatus::SnapshotCreated,
        SnapshotStatus::Writing => PbSnapshotStatus::SnapshotWriting,
        SnapshotStatus::Sealed => PbSnapshotStatus::SnapshotSealed,
        SnapshotStatus::Corrupt => PbSnapshotStatus::SnapshotCorrupt,
        SnapshotStatus::Deleted => PbSnapshotStatus::SnapshotDeleted,
    }
}

/// Repository → PbRepoInfo
pub fn repo_to_pb_info(repo: &badou_engine::domain::repository::Repository) -> PbRepoInfo {
    PbRepoInfo {
        repo_id: repo.repo_id.0.to_string(),
        name: repo.name.clone(),
        status: repo_status_to_pb(repo.status) as i32,
        immutable_until: repo.config.immutable_until.as_ref().map(chrono_to_ts),
        version_count: repo.version_count,
        total_size: 0,
        stored_size: 0,
        created_at: Some(chrono_to_ts(&repo.created_at)),
    }
}

/// Version → PbVersionInfo
pub fn version_to_pb_info(v: &Version) -> PbVersionInfo {
    PbVersionInfo {
        version_id: v.version_id.0.to_string(),
        repo_id: v.repo_id.0.to_string(),
        snapshot_id: v.snapshot_id.to_string(),
        parent_version_id: v.parent_version_id.as_ref().map(|p| p.0.to_string()).unwrap_or_default(),
        sequence: v.sequence,
        status: version_status_to_pb(v.status) as i32,
        created_at: Some(chrono_to_ts(&v.created_at)),
        sealed_at: v.sealed_at.as_ref().map(chrono_to_ts),
        immutable_until: v.immutable_until.as_ref().map(chrono_to_ts),
    }
}

/// Snapshot → PbSnapshotMeta
pub fn snapshot_to_pb_meta(s: &Snapshot, repo_id: &str) -> PbSnapshotMeta {
    PbSnapshotMeta {
        snapshot_id: s.snapshot_id.to_string(),
        version_id: s.version_id.0.to_string(),
        repo_id: repo_id.to_string(),
        status: snapshot_status_to_pb(s.status) as i32,
        source_machine: s.source_machine.hostname.clone(),
        backup_policy: serde_json::to_vec(&s.backup_policy).unwrap_or_default(),
        file_tree_root: s.file_tree.root.clone(),
        encryption_info: serde_json::to_vec(&s.encryption_info).unwrap_or_default(),
        compression_info: serde_json::to_vec(&s.compression_info).unwrap_or_default(),
        total_size: s.total_size,
        stored_size: s.stored_size,
        file_count: s.file_count,
        chunk_count: s.chunk_count,
        created_at: Some(chrono_to_ts(&s.created_at)),
    }
}

/// Manifest → PbManifestData
pub fn manifest_to_pb_data(m: &Manifest) -> PbManifestData {
    PbManifestData {
        manifest_id: m.manifest_id.to_string(),
        snapshot_id: m.snapshot_id.to_string(),
        file_tree: m.file_tree.clone(),
        chunk_refs: m.chunk_refs.iter().map(|r| PbChunkRef {
            chunk_hash: hex::encode(r.chunk_hash.0),
            offset: r.offset,
            size: r.size,
        }).collect(),
        created_at: Some(chrono_to_ts(&m.created_at)),
    }
}

/// PbChunkData → (ChunkHash, Vec<u8>)
pub fn pb_to_chunk_data(pb: &PbChunkData) -> Result<(ChunkHash, Vec<u8>), String> {
    let hash = parse_hash(&pb.chunk_hash)?;
    Ok((hash, pb.data.clone()))
}

/// ChunkInfo proto builder
pub fn make_chunk_info(hash_hex: &str, size: u64, stored_size: u64, ref_count: u32) -> PbChunkInfo {
    PbChunkInfo {
        chunk_hash: hash_hex.to_string(),
        size,
        stored_size,
        ref_count,
        status: PbChunkStatus::ChunkActive as i32,
        created_at: Some(chrono_to_ts(&Utc::now())),
    }
}

/// badou_verify::VerifyReport → PbVerifyReport
pub fn verify_report_to_pb(report: &badou_verify::VerifyReport, target_id: &str) -> PbVerifyReport {
    let (passed, level) = match &report.target {
        badou_verify::VerifyTarget::Chunk { .. } => (report.status == badou_verify::VerifyStatus::Pass, 3),
        badou_verify::VerifyTarget::Manifest { .. } => (report.status == badou_verify::VerifyStatus::Pass, 2),
        badou_verify::VerifyTarget::Snapshot { .. } => (report.status == badou_verify::VerifyStatus::Pass, 2),
        badou_verify::VerifyTarget::Version { .. } => (report.status == badou_verify::VerifyStatus::Pass, 2),
        badou_verify::VerifyTarget::Repository { .. } => (report.status == badou_verify::VerifyStatus::Pass, 1),
    };
    let failed_items = match &report.status {
        badou_verify::VerifyStatus::Mismatch { expected, actual } => {
            vec![format!("expected: {}, actual: {}", expected, actual)]
        }
        badou_verify::VerifyStatus::Missing { detail } => vec![detail.clone()],
        _ => vec![],
    };
    PbVerifyReport {
        target_id: target_id.to_string(),
        level,
        passed,
        total_checked: 1,
        total_failed: if passed { 0 } else { 1 },
        failed_items,
        checked_at: Some(chrono_to_ts(&report.checked_at)),
    }
}

/// RecoveryChunk proto builder
pub fn make_recovery_chunk(hash_hex: &str, data: Vec<u8>, file_path: &str, offset: u64) -> PbRecoveryChunk {
    let size = data.len() as u64;
    PbRecoveryChunk {
        chunk_hash: hash_hex.to_string(),
        data,
        size,
        file_path: file_path.to_string(),
        offset,
    }
}

/// GcReport → PbGcReport
pub fn gc_report_to_pb(report: &badou_gc::GcReport, repo_id: &str) -> PbGcReport {
    PbGcReport {
        repo_id: repo_id.to_string(),
        chunks_scanned: (report.purged_chunks.len() + report.skipped_chunks.len()) as u64,
        chunks_collected: report.purged_count() as u64,
        bytes_freed: report.freed_bytes,
        started_at: Some(chrono_to_ts(&report.started_at)),
        completed_at: Some(chrono_to_ts(&report.finished_at)),
    }
}