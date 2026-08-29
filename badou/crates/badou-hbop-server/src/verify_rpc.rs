//! Verify API 3 RPC 实现（2 服务端流式 + 1 单次）。

use tonic::{Request, Response, Status};
use tokio_stream::wrappers::ReceiverStream;
use badou_proto::{
    VerifyRepositoryRequest, VerifyReport,
    VerifyVersionRequest,
    VerifyChunkRequest, VerifyChunkResponse,
};
use std::sync::Arc;
use tokio::sync::mpsc;

use crate::state::ServerState;
use crate::auth::require_auth;
use crate::error::{to_status, verify_error_to_status};
use crate::convert::*;
use badou_proto::HbopErrorCode;
use badou_verify::Verifier;

pub async fn verify_repository(
    state: &Arc<ServerState>,
    request: Request<VerifyRepositoryRequest>,
) -> Result<Response<ReceiverStream<Result<VerifyReport, Status>>>, Status> {
    require_auth(&request, state.auth_config())?;
    let req = request.into_inner();
    let repo_id = parse_repo_id(&req.repo_id)
        .map_err(|e| to_status(HbopErrorCode::RepoNotFound, e))?;

    let handle = state.open_repo(&repo_id)
        .map_err(|e| crate::error::repo_error_to_status(&e))?;

    let verifier = Verifier::new(
        &repo_id,
        handle.chunk_store(),
        handle.manifest_store(),
        handle.snapshot_store(),
        handle.index(),
    );

    let reports = verifier.verify_repository()
        .map_err(|e| verify_error_to_status(&e))?;

    let (tx, rx) = mpsc::channel(32);
    tokio::spawn(async move {
        for report in reports {
            let pb_report = verify_report_to_pb(&report, &req.repo_id);
            if tx.send(Ok(pb_report)).await.is_err() {
                break;
            }
        }
    });

    Ok(Response::new(ReceiverStream::new(rx)))
}

pub async fn verify_version(
    state: &Arc<ServerState>,
    request: Request<VerifyVersionRequest>,
) -> Result<Response<ReceiverStream<Result<VerifyReport, Status>>>, Status> {
    require_auth(&request, state.auth_config())?;
    let req = request.into_inner();
    let repo_id = parse_repo_id(&req.repo_id)
        .map_err(|e| to_status(HbopErrorCode::RepoNotFound, e))?;
    let version_id = parse_version_id(&req.version_id)
        .map_err(|e| to_status(HbopErrorCode::VersionNotFound, e))?;

    let version = state.version_ops().get_version(&version_id)
        .map_err(|e| crate::error::version_error_to_status(&e))?;

    let handle = state.open_repo(&repo_id)
        .map_err(|e| crate::error::repo_error_to_status(&e))?;

    let verifier = Verifier::new(
        &repo_id,
        handle.chunk_store(),
        handle.manifest_store(),
        handle.snapshot_store(),
        handle.index(),
    );

    let snapshot_report = verifier.verify_snapshot(version.snapshot_id)
        .map_err(|e| verify_error_to_status(&e))?;

    let (tx, rx) = mpsc::channel(32);
    tokio::spawn(async move {
        let pb_report = verify_report_to_pb(&snapshot_report, &req.version_id);
        let _ = tx.send(Ok(pb_report)).await;
    });

    Ok(Response::new(ReceiverStream::new(rx)))
}

pub async fn verify_chunk(
    state: &Arc<ServerState>,
    request: Request<VerifyChunkRequest>,
) -> Result<Response<VerifyChunkResponse>, Status> {
    require_auth(&request, state.auth_config())?;
    let req = request.into_inner();
    let repo_id = parse_repo_id(&req.repo_id)
        .map_err(|e| to_status(HbopErrorCode::RepoNotFound, e))?;

    let hash = parse_hash(&req.chunk_hash)
        .map_err(|e| to_status(HbopErrorCode::HashMismatch, e))?;

    let handle = state.open_repo(&repo_id)
        .map_err(|e| crate::error::repo_error_to_status(&e))?;

    let verifier = Verifier::new(
        &repo_id,
        handle.chunk_store(),
        handle.manifest_store(),
        handle.snapshot_store(),
        handle.index(),
    );

    let report = verifier.verify_chunk(&hash);

    let (passed, expected, actual) = match &report.status {
        badou_verify::VerifyStatus::Pass => (true, req.chunk_hash.clone(), req.chunk_hash.clone()),
        badou_verify::VerifyStatus::Mismatch { expected, actual } => (false, expected.clone(), actual.clone()),
        badou_verify::VerifyStatus::Missing { .. } => (false, req.chunk_hash.clone(), String::new()),
        badou_verify::VerifyStatus::Fail => (false, req.chunk_hash.clone(), String::new()),
    };

    Ok(Response::new(VerifyChunkResponse {
        passed,
        expected_hash: expected,
        actual_hash: actual,
    }))
}