//! Recovery API 1 RPC 实现（服务端流式返回 RecoveryChunk）。

use tonic::{Request, Response, Status};
use tokio_stream::wrappers::ReceiverStream;
use badou_proto::{RecoveryOpenRequest, RecoveryChunk};
use std::sync::Arc;
use tokio::sync::mpsc;

use crate::state::ServerState;
use crate::auth::require_auth;
use crate::error::{to_status, recovery_error_to_status};
use crate::convert::*;
use badou_proto::HbopErrorCode;
use badou_recovery::{RecoveryEngine, RecoveryRequest};

pub async fn recovery_open(
    state: &Arc<ServerState>,
    request: Request<RecoveryOpenRequest>,
) -> Result<Response<ReceiverStream<Result<RecoveryChunk, Status>>>, Status> {
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

    let engine = RecoveryEngine::new(
        &repo_id,
        handle.chunk_store(),
        handle.manifest_store(),
        handle.snapshot_store(),
    );

    let file_filter = req.file_path.as_ref()
        .map(|p| vec![p.clone()]);

    let recovery_req = RecoveryRequest {
        snapshot_id: version.snapshot_id,
        file_filter,
    };

    let result = engine.recover(&recovery_req)
        .map_err(|e| recovery_error_to_status(&e))?;

    let (tx, rx) = mpsc::channel(32);
    tokio::spawn(async move {
        for file in &result.recovered_files {
            let hash_hex = file.chunk_hashes.first().cloned().unwrap_or_default();
            let chunk = make_recovery_chunk(
                &hash_hex,
                file.data.clone(),
                &file.path,
                0,
            );
            if tx.send(Ok(chunk)).await.is_err() {
                break;
            }
        }
    });

    Ok(Response::new(ReceiverStream::new(rx)))
}