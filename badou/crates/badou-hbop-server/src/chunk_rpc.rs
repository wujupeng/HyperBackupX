//! Chunk API 5 RPC 实现（含流式 BatchPut）。

use tonic::{Request, Response, Status};
use tonic::Streaming;
use badou_proto::{
    ChunkPutRequest, ChunkPutResponse,
    ChunkGetRequest, ChunkGetResponse,
    ChunkExistsRequest, ChunkExistsResponse,
    ChunkDeleteRequest, ChunkDeleteResponse,
    ChunkBatchPutResponse,
    ChunkData,
};
use std::sync::Arc;

use crate::state::ServerState;
use crate::auth::{require_write, require_auth};
use crate::error::{chunk_error_to_status, to_status};
use crate::convert::*;
use badou_proto::HbopErrorCode;
use badou_ops::ChunkOps;

pub async fn chunk_put(
    state: &Arc<ServerState>,
    request: Request<ChunkPutRequest>,
) -> Result<Response<ChunkPutResponse>, Status> {
    require_write(&request, state.auth_config())?;
    let req = request.into_inner();
    let repo_id = parse_repo_id(&req.repo_id)
        .map_err(|e| to_status(HbopErrorCode::RepoNotFound, e))?;

    let chunk = req.chunk.as_ref()
        .ok_or_else(|| to_status(HbopErrorCode::ChunkNotFound, "missing chunk data"))?;

    let (hash, data) = pb_to_chunk_data(chunk)
        .map_err(|e| to_status(HbopErrorCode::HashMismatch, e))?;

    let actual_hash = blake3::hash(&data);
    if actual_hash.as_bytes() != hash.0.as_slice() {
        return Err(to_status(HbopErrorCode::HashMismatch, "chunk hash does not match data"));
    }

    let handle = state.open_repo(&repo_id)
        .map_err(|e| crate::error::repo_error_to_status(&e))?;

    let ops = ChunkOps::new(&repo_id, handle.chunk_store());
    let _location = ops.put_chunk(&hash, &data)
        .map_err(|e| chunk_error_to_status(&e))?;

    let info = make_chunk_info(
        &chunk.chunk_hash,
        chunk.size,
        chunk.size,
        handle.chunk_store().ref_count(&hash),
    );

    Ok(Response::new(ChunkPutResponse { info: Some(info) }))
}

pub async fn chunk_get(
    state: &Arc<ServerState>,
    request: Request<ChunkGetRequest>,
) -> Result<Response<ChunkGetResponse>, Status> {
    require_auth(&request, state.auth_config())?;
    let req = request.into_inner();
    let repo_id = parse_repo_id(&req.repo_id)
        .map_err(|e| to_status(HbopErrorCode::RepoNotFound, e))?;

    let hash = parse_hash(&req.chunk_hash)
        .map_err(|e| to_status(HbopErrorCode::HashMismatch, e))?;

    let handle = state.open_repo(&repo_id)
        .map_err(|e| crate::error::repo_error_to_status(&e))?;

    let ops = ChunkOps::new(&repo_id, handle.chunk_store());
    let data = ops.get_chunk(&hash)
        .map_err(|e| chunk_error_to_status(&e))?;

    let size = data.len() as u64;
    Ok(Response::new(ChunkGetResponse {
        chunk: Some(ChunkData {
            chunk_hash: req.chunk_hash,
            data,
            size,
        }),
    }))
}

pub async fn chunk_exists(
    state: &Arc<ServerState>,
    request: Request<ChunkExistsRequest>,
) -> Result<Response<ChunkExistsResponse>, Status> {
    require_auth(&request, state.auth_config())?;
    let req = request.into_inner();
    let repo_id = parse_repo_id(&req.repo_id)
        .map_err(|e| to_status(HbopErrorCode::RepoNotFound, e))?;

    let hash = parse_hash(&req.chunk_hash)
        .map_err(|e| to_status(HbopErrorCode::HashMismatch, e))?;

    let handle = state.open_repo(&repo_id)
        .map_err(|e| crate::error::repo_error_to_status(&e))?;

    let ops = ChunkOps::new(&repo_id, handle.chunk_store());
    let (exists, ref_count) = ops.chunk_exists(&hash);

    let info = if exists {
        Some(make_chunk_info(&req.chunk_hash, 0, 0, ref_count))
    } else {
        None
    };

    Ok(Response::new(ChunkExistsResponse { exists, info }))
}

pub async fn chunk_delete(
    state: &Arc<ServerState>,
    request: Request<ChunkDeleteRequest>,
) -> Result<Response<ChunkDeleteResponse>, Status> {
    require_write(&request, state.auth_config())?;
    let req = request.into_inner();
    let repo_id = parse_repo_id(&req.repo_id)
        .map_err(|e| to_status(HbopErrorCode::RepoNotFound, e))?;

    let hash = parse_hash(&req.chunk_hash)
        .map_err(|e| to_status(HbopErrorCode::HashMismatch, e))?;

    let handle = state.open_repo(&repo_id)
        .map_err(|e| crate::error::repo_error_to_status(&e))?;

    let ops = ChunkOps::new(&repo_id, handle.chunk_store());
    match ops.delete_chunk(&hash) {
        Ok(()) => Ok(Response::new(ChunkDeleteResponse { deleted: true })),
        Err(e) => Err(chunk_error_to_status(&e)),
    }
}

pub async fn chunk_batch_put(
    state: &Arc<ServerState>,
    request: Request<Streaming<ChunkPutRequest>>,
) -> Result<Response<ChunkBatchPutResponse>, Status> {
    let metadata = request.metadata().clone();
    let dummy_req = Request::from_parts(metadata, tonic::Extensions::default(), ());
    require_write(&dummy_req, state.auth_config())?;

    let mut stream = request.into_inner();
    let mut infos = Vec::new();
    let mut success_count = 0u32;
    let mut failure_count = 0u32;

    while let Some(chunk_req) = stream.message().await? {
        let repo_id = parse_repo_id(&chunk_req.repo_id)
            .map_err(|e| to_status(HbopErrorCode::RepoNotFound, e))?;

        let chunk = chunk_req.chunk.as_ref()
            .ok_or_else(|| to_status(HbopErrorCode::ChunkNotFound, "missing chunk data"))?;

        let (hash, data) = pb_to_chunk_data(chunk)
            .map_err(|e| to_status(HbopErrorCode::HashMismatch, e))?;

        let actual_hash = blake3::hash(&data);
        if actual_hash.as_bytes() != hash.0.as_slice() {
            failure_count += 1;
            continue;
        }

        let handle = state.open_repo(&repo_id)
            .map_err(|e| crate::error::repo_error_to_status(&e))?;

        let ops = ChunkOps::new(&repo_id, handle.chunk_store());
        match ops.put_chunk(&hash, &data) {
            Ok(_loc) => {
                let info = make_chunk_info(
                    &chunk.chunk_hash,
                    chunk.size,
                    chunk.size,
                    handle.chunk_store().ref_count(&hash),
                );
                infos.push(info);
                success_count += 1;
            }
            Err(_) => {
                failure_count += 1;
            }
        }
    }

    Ok(Response::new(ChunkBatchPutResponse {
        infos,
        success_count,
        failure_count,
    }))
}