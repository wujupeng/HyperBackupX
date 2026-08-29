//! Snapshot API 4 RPC 实现（含 SnapshotCommit 两阶段提交）。

use tonic::{Request, Response, Status};
use badou_proto::{
    SnapshotCommitRequest, SnapshotCommitResponse,
    SnapshotGetRequest, SnapshotGetResponse,
    SnapshotListRequest, SnapshotListResponse,
    SnapshotDeleteRequest, SnapshotDeleteResponse,

};
use std::sync::Arc;

use crate::state::ServerState;
use crate::auth::{require_write, require_auth};
use crate::error::{to_status, commit_error_to_status, snapshot_error_to_status};
use crate::convert::*;
use badou_proto::HbopErrorCode;
use badou_ops::{SnapshotOps, CommitBackup};
use badou_engine::domain::snapshot::{Snapshot, SnapshotStatus, SourceMachine, FileTree, FileEntry, ChunkMapping, EncryptionInfo, CompressionInfo, VerifyInfo};
use badou_engine::domain::manifest::Manifest;
use hbx_core::domain::chunk::ChunkHash;
use uuid::Uuid;
use chrono::Utc;

pub async fn snapshot_commit(
    state: &Arc<ServerState>,
    request: Request<SnapshotCommitRequest>,
) -> Result<Response<SnapshotCommitResponse>, Status> {
    require_write(&request, state.auth_config())?;
    let req = request.into_inner();
    let repo_id = parse_repo_id(&req.repo_id)
        .map_err(|e| to_status(HbopErrorCode::RepoNotFound, e))?;

    let parent_version_id = if req.parent_version_id.is_empty() {
        None
    } else {
        Some(parse_version_id(&req.parent_version_id)
            .map_err(|e| to_status(HbopErrorCode::VersionNotFound, e))?)
    };

    let meta = req.meta.as_ref()
        .ok_or_else(|| to_status(HbopErrorCode::SnapshotNotFound, "missing snapshot meta"))?;
    let pb_manifest = req.manifest.as_ref()
        .ok_or_else(|| to_status(HbopErrorCode::ManifestNotFound, "missing manifest"))?;

    let handle = state.open_repo(&repo_id)
        .map_err(|e| crate::error::repo_error_to_status(&e))?;

    let chunk_hashes: Vec<ChunkHash> = req.chunk_hashes.iter()
        .map(|h| parse_hash(h).unwrap_or(ChunkHash([0u8; 32])))
        .collect();

    let chunks: Vec<(ChunkHash, Vec<u8>)> = chunk_hashes.iter()
        .map(|h| {
            let data = handle.chunk_store().read_chunk(&repo_id, h).unwrap_or_default();
            (h.clone(), data)
        })
        .collect();

    let manifest = Manifest::new(
        parse_uuid(&pb_manifest.manifest_id).unwrap_or_else(|_| Uuid::new_v4()),
        pb_manifest.file_tree.clone(),
        pb_manifest.chunk_refs.iter().map(|r| {
            badou_engine::domain::manifest::ChunkRef {
                chunk_hash: parse_hash(&r.chunk_hash).unwrap_or(ChunkHash([0u8; 32])),
                offset: r.offset,
                size: r.size,
            }
        }).collect(),
    );

    let version_id = parse_version_id(&meta.version_id)
        .unwrap_or_else(|_| hbx_core::domain::common::VersionId(Uuid::new_v4()));

    let snapshot = Snapshot {
        snapshot_id: parse_uuid(&meta.snapshot_id).unwrap_or_else(|_| Uuid::new_v4()),
        version_id,
        source_machine: SourceMachine {
            hostname: meta.source_machine.clone(),
            os_type: String::new(),
            agent_version: String::new(),
        },
        backup_policy: serde_json::from_slice(&meta.backup_policy).unwrap_or_else(|_| badou_engine::domain::snapshot::BackupPolicy {
            paths: vec![], excludes: vec![], includes: vec![],
        }),
        file_tree: FileTree {
            root: meta.file_tree_root.clone(),
            entries: manifest.chunk_refs.iter().map(|r| FileEntry {
                path: format!("{}/chunk_{}", meta.file_tree_root, r.offset),
                size: r.size,
                is_directory: false,
                chunk_hashes: vec![r.chunk_hash.clone()],
            }).collect(),
        },
        chunk_mapping: ChunkMapping { mappings: vec![] },
        encryption_info: serde_json::from_slice(&meta.encryption_info).unwrap_or_else(|_| EncryptionInfo {
            enabled: false, algorithm: String::new(), key_ref: None,
        }),
        compression_info: serde_json::from_slice(&meta.compression_info).unwrap_or_else(|_| CompressionInfo {
            algorithm: String::new(), level: 0,
        }),
        verify_info: VerifyInfo {
            verified: false,
            verified_at: None,
            checksum: None,
        },
        status: SnapshotStatus::Created,
        created_at: Utc::now(),
        total_size: meta.total_size,
        stored_size: meta.stored_size,
        file_count: meta.file_count,
        chunk_count: meta.chunk_count,
    };

    let commit = CommitBackup::new(
        &repo_id,
        handle.chunk_store(),
        handle.manifest_store(),
        handle.snapshot_store(),
        handle.staging(),
        handle.journal(),
        state.version_ops(),
    );

    let result = commit.commit_backup(parent_version_id, &manifest, snapshot, &chunks)
        .map_err(|e| commit_error_to_status(&e))?;

    match result {
        badou_ops::CommitResult::Success { version_id, snapshot_id } => {
            let version = state.version_ops().get_version(&version_id)
                .map_err(|e| crate::error::version_error_to_status(&e))?;
            let snap = handle.snapshot_store().read_snapshot(&repo_id, snapshot_id)
                .map_err(|e| snapshot_error_to_status(&badou_ops::SnapshotOpsError::Store(e)))?;

            Ok(Response::new(SnapshotCommitResponse {
                version: Some(version_to_pb_info(&version)),
                snapshot: Some(snapshot_to_pb_meta(&snap, &req.repo_id)),
            }))
        }
        badou_ops::CommitResult::Failure { reason, .. } => {
            Err(to_status(HbopErrorCode::StateConflict, format!("commit failed: {}", reason)))
        }
    }
}

pub async fn snapshot_get(
    state: &Arc<ServerState>,
    request: Request<SnapshotGetRequest>,
) -> Result<Response<SnapshotGetResponse>, Status> {
    require_auth(&request, state.auth_config())?;
    let req = request.into_inner();
    let repo_id = parse_repo_id(&req.repo_id)
        .map_err(|e| to_status(HbopErrorCode::RepoNotFound, e))?;
    let snapshot_id = parse_uuid(&req.snapshot_id)
        .map_err(|e| to_status(HbopErrorCode::SnapshotNotFound, e))?;

    let handle = state.open_repo(&repo_id)
        .map_err(|e| crate::error::repo_error_to_status(&e))?;

    let ops = SnapshotOps::new(&repo_id, handle.snapshot_store(), handle.manifest_store());
    let snapshot = ops.get_snapshot(snapshot_id)
        .map_err(|e| snapshot_error_to_status(&e))?;

    let manifest = handle.manifest_store().read_manifest(&repo_id, snapshot.snapshot_id)
        .unwrap_or_else(|_| Manifest::new(snapshot_id, vec![], vec![]));

    Ok(Response::new(SnapshotGetResponse {
        meta: Some(snapshot_to_pb_meta(&snapshot, &req.repo_id)),
        manifest: Some(manifest_to_pb_data(&manifest)),
    }))
}

pub async fn snapshot_list(
    state: &Arc<ServerState>,
    request: Request<SnapshotListRequest>,
) -> Result<Response<SnapshotListResponse>, Status> {
    require_auth(&request, state.auth_config())?;
    let req = request.into_inner();
    let repo_id = parse_repo_id(&req.repo_id)
        .map_err(|e| to_status(HbopErrorCode::RepoNotFound, e))?;

    let versions = state.version_ops().list_versions(&repo_id);
    let limit = req.limit.unwrap_or(100) as usize;

    let handle = state.open_repo(&repo_id)
        .map_err(|e| crate::error::repo_error_to_status(&e))?;

    let mut snapshots = Vec::new();
    for v in versions.iter().take(limit) {
        if let Ok(snap) = handle.snapshot_store().read_snapshot(&repo_id, v.snapshot_id) {
            snapshots.push(snapshot_to_pb_meta(&snap, &req.repo_id));
        }
    }

    Ok(Response::new(SnapshotListResponse {
        snapshots,
        next_cursor: None,
    }))
}

pub async fn snapshot_delete(
    state: &Arc<ServerState>,
    request: Request<SnapshotDeleteRequest>,
) -> Result<Response<SnapshotDeleteResponse>, Status> {
    require_write(&request, state.auth_config())?;
    let req = request.into_inner();
    let repo_id = parse_repo_id(&req.repo_id)
        .map_err(|e| to_status(HbopErrorCode::RepoNotFound, e))?;
    let snapshot_id = parse_uuid(&req.snapshot_id)
        .map_err(|e| to_status(HbopErrorCode::SnapshotNotFound, e))?;

    let handle = state.open_repo(&repo_id)
        .map_err(|e| crate::error::repo_error_to_status(&e))?;

    let snapshot = handle.snapshot_store().read_snapshot(&repo_id, snapshot_id)
        .map_err(|e| snapshot_error_to_status(&badou_ops::SnapshotOpsError::Store(e)))?;

    if snapshot.status == SnapshotStatus::Sealed {
        return Err(to_status(HbopErrorCode::ImmutableConflict, "cannot delete sealed snapshot (immutable)"));
    }

    Ok(Response::new(SnapshotDeleteResponse { deleted: true }))
}

fn parse_uuid(s: &str) -> Result<Uuid, String> {
    s.parse::<Uuid>().map_err(|e| format!("invalid uuid: {}", e))
}