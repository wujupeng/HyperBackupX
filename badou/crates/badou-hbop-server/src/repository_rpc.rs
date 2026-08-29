//! Repository API 7 RPC 实现。

use tonic::{Request, Response, Status};
use badou_proto::{
    RepositoryCreateRequest, RepositoryCreateResponse,
    RepositoryOpenRequest, RepositoryOpenResponse,
    RepositoryCloseRequest, RepositoryCloseResponse,
    RepositoryListRequest, RepositoryListResponse,
    RepositoryDeleteRequest, RepositoryDeleteResponse,
    RepositoryConfigureRequest, RepositoryConfigureResponse,
    RepositoryStatRequest, RepositoryStatResponse,
    RepositoryInfo,
};
use std::sync::Arc;
use chrono::Utc;

use crate::state::ServerState;
use crate::auth::{require_write, require_admin, require_auth};
use crate::error::{repo_error_to_status, to_status};
use crate::convert::*;
use badou_proto::HbopErrorCode;

pub async fn repository_create(
    state: &Arc<ServerState>,
    request: Request<RepositoryCreateRequest>,
) -> Result<Response<RepositoryCreateResponse>, Status> {
    require_admin(&request, state.auth_config())?;
    let req = request.into_inner();

    if req.config.is_none() {
        return Err(to_status(HbopErrorCode::RepoNotFound, "missing config"));
    }

    let config = pb_to_repo_config(req.config.as_ref().unwrap());
    let name = config.name.clone();

    let repo = state.repo_manager()
        .create_repository(&name, config)
        .map_err(|e| repo_error_to_status(&e))?;

    Ok(Response::new(RepositoryCreateResponse {
        repo: Some(repo_to_pb_info(&repo)),
    }))
}

pub async fn repository_open(
    state: &Arc<ServerState>,
    request: Request<RepositoryOpenRequest>,
) -> Result<Response<RepositoryOpenResponse>, Status> {
    require_auth(&request, state.auth_config())?;
    let req = request.into_inner();
    let repo_id = parse_repo_id(&req.repo_id)
        .map_err(|e| to_status(HbopErrorCode::RepoNotFound, e))?;

    let handle = state.open_repo(&repo_id)
        .map_err(|e| repo_error_to_status(&e))?;

    let repo = &state.repo_manager().stat_repository(repo_id)
        .map_err(|e| repo_error_to_status(&e))?;

    let info = RepositoryInfo {
        repo_id: repo.repo_id.0.to_string(),
        name: repo.name.clone(),
        status: repo_status_to_pb(repo.status) as i32,
        immutable_until: None,
        version_count: repo.version_count,
        total_size: repo.total_size,
        stored_size: repo.stored_size,
        created_at: Some(chrono_to_ts(&Utc::now())),
    };

    drop(handle);
    Ok(Response::new(RepositoryOpenResponse { repo: Some(info) }))
}

pub async fn repository_close(
    state: &Arc<ServerState>,
    request: Request<RepositoryCloseRequest>,
) -> Result<Response<RepositoryCloseResponse>, Status> {
    require_auth(&request, state.auth_config())?;
    let req = request.into_inner();
    let repo_id = parse_repo_id(&req.repo_id)
        .map_err(|e| to_status(HbopErrorCode::RepoNotFound, e))?;

    state.close_repo(&repo_id);
    Ok(Response::new(RepositoryCloseResponse {}))
}

pub async fn repository_list(
    state: &Arc<ServerState>,
    request: Request<RepositoryListRequest>,
) -> Result<Response<RepositoryListResponse>, Status> {
    require_auth(&request, state.auth_config())?;

    let repos = state.repo_manager().list_repositories()
        .map_err(|e| repo_error_to_status(&e))?;

    let pb_repos = repos.iter().map(|r| RepositoryInfo {
        repo_id: r.repo_id.0.to_string(),
        name: r.name.clone(),
        status: repo_status_to_pb(r.status) as i32,
        immutable_until: None,
        version_count: r.version_count,
        total_size: r.total_size,
        stored_size: r.stored_size,
        created_at: Some(chrono_to_ts(&Utc::now())),
    }).collect();

    Ok(Response::new(RepositoryListResponse { repos: pb_repos }))
}

pub async fn repository_delete(
    state: &Arc<ServerState>,
    request: Request<RepositoryDeleteRequest>,
) -> Result<Response<RepositoryDeleteResponse>, Status> {
    require_admin(&request, state.auth_config())?;
    let req = request.into_inner();
    let repo_id = parse_repo_id(&req.repo_id)
        .map_err(|e| to_status(HbopErrorCode::RepoNotFound, e))?;

    state.close_repo(&repo_id);

    state.repo_manager().delete_repository(repo_id)
        .map_err(|e| repo_error_to_status(&e))?;

    Ok(Response::new(RepositoryDeleteResponse { gc_report: None }))
}

pub async fn repository_configure(
    state: &Arc<ServerState>,
    request: Request<RepositoryConfigureRequest>,
) -> Result<Response<RepositoryConfigureResponse>, Status> {
    require_write(&request, state.auth_config())?;
    let req = request.into_inner();
    let repo_id = parse_repo_id(&req.repo_id)
        .map_err(|e| to_status(HbopErrorCode::RepoNotFound, e))?;

    if req.config.is_none() {
        return Err(to_status(HbopErrorCode::RepoNotFound, "missing config"));
    }

    let stat = state.repo_manager().stat_repository(repo_id)
        .map_err(|e| repo_error_to_status(&e))?;

    let info = RepositoryInfo {
        repo_id: stat.repo_id.0.to_string(),
        name: stat.name.clone(),
        status: repo_status_to_pb(stat.status) as i32,
        immutable_until: None,
        version_count: stat.version_count,
        total_size: stat.total_size,
        stored_size: stat.stored_size,
        created_at: Some(chrono_to_ts(&Utc::now())),
    };

    Ok(Response::new(RepositoryConfigureResponse { repo: Some(info) }))
}

pub async fn repository_stat(
    state: &Arc<ServerState>,
    request: Request<RepositoryStatRequest>,
) -> Result<Response<RepositoryStatResponse>, Status> {
    require_auth(&request, state.auth_config())?;
    let req = request.into_inner();
    let repo_id = parse_repo_id(&req.repo_id)
        .map_err(|e| to_status(HbopErrorCode::RepoNotFound, e))?;

    let stat = state.repo_manager().stat_repository(repo_id.clone())
        .map_err(|e| repo_error_to_status(&e))?;

    let info = RepositoryInfo {
        repo_id: stat.repo_id.0.to_string(),
        name: stat.name.clone(),
        status: repo_status_to_pb(stat.status) as i32,
        immutable_until: None,
        version_count: stat.version_count,
        total_size: stat.total_size,
        stored_size: stat.stored_size,
        created_at: Some(chrono_to_ts(&Utc::now())),
    };

    Ok(Response::new(RepositoryStatResponse {
        repo: Some(info),
        chunk_count: stat.chunk_count,
        snapshot_count: state.version_ops().version_count(&repo_id) as u64,
        index_size: 0,
    }))
}