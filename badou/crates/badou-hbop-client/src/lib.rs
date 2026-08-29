//! HBOP gRPC Client。
//!
//! 封装 tonic `BaDouStorageClient`，提供高层 API 与 mTLS 连接、版本协商。

#![allow(clippy::result_large_err)]

use tonic::transport::Channel;

use thiserror::Error;

use badou_proto::ba_dou_storage_client::BaDouStorageClient;
use badou_proto::{
    RepositoryCreateRequest, RepositoryOpenRequest, RepositoryCloseRequest,
    RepositoryListRequest, RepositoryDeleteRequest, RepositoryConfigureRequest,
    RepositoryStatRequest, RepositoryConfig,
    ChunkPutRequest, ChunkGetRequest, ChunkExistsRequest, ChunkDeleteRequest,
    SnapshotCommitRequest, SnapshotGetRequest, SnapshotListRequest, SnapshotDeleteRequest,
    VerifyRepositoryRequest, VerifyVersionRequest, VerifyChunkRequest,
    RecoveryOpenRequest,
    HbopErrorCode,
};


#[derive(Debug, Error)]
pub enum BadouClientError {
    #[error("gRPC transport error: {0}")]
    Transport(#[from] tonic::transport::Error),
    #[error("gRPC status error: {0}")]
    Status(#[from] tonic::Status),
    #[error("HBOP error: {code:?} - {message}")]
    Hbop { code: HbopErrorCode, message: String },
    #[error("invalid endpoint URL: {0}")]
    InvalidEndpoint(String),
}

pub struct BadouHbopClient {
    inner: BaDouStorageClient<Channel>,
}

impl BadouHbopClient {
    pub async fn connect(endpoint: &str) -> Result<Self, BadouClientError> {
        let channel = Channel::from_shared(endpoint.to_string())
            .map_err(|e| BadouClientError::InvalidEndpoint(e.to_string()))?
            .connect()
            .await?;

        let client = BaDouStorageClient::new(channel);

        Ok(Self {
            inner: client,
        })
    }

    pub async fn connect_with_tls(
        endpoint: &str,
        ca_cert_pem: &[u8],
        client_cert_pem: &[u8],
        client_key_pem: &[u8],
    ) -> Result<Self, BadouClientError> {
        let ca = tonic::transport::Certificate::from_pem(ca_cert_pem);
        let cert = tonic::transport::Identity::from_pem(client_cert_pem, client_key_pem);

        let tls = tonic::transport::ClientTlsConfig::new()
            .ca_certificate(ca)
            .identity(cert)
            .domain_name("badou");

        let channel = Channel::from_shared(endpoint.to_string())
            .map_err(|e| BadouClientError::InvalidEndpoint(e.to_string()))?
            .tls_config(tls)?
            .connect()
            .await?;

        let client = BaDouStorageClient::new(channel);

        Ok(Self {
            inner: client,
        })
    }

    fn clone_client(&self) -> BaDouStorageClient<Channel> {
        self.inner.clone()
    }

    pub async fn repository_create(&self, config: RepositoryConfig) -> Result<badou_proto::RepositoryCreateResponse, BadouClientError> {
        let mut client = self.clone_client();
        let req = tonic::Request::new(RepositoryCreateRequest { config: Some(config) });
        Ok(client.repository_create(req).await?.into_inner())
    }

    pub async fn repository_open(&self, repo_id: &str) -> Result<badou_proto::RepositoryOpenResponse, BadouClientError> {
        let mut client = self.clone_client();
        let req = tonic::Request::new(RepositoryOpenRequest { repo_id: repo_id.to_string() });
        Ok(client.repository_open(req).await?.into_inner())
    }

    pub async fn repository_close(&self, repo_id: &str) -> Result<badou_proto::RepositoryCloseResponse, BadouClientError> {
        let mut client = self.clone_client();
        let req = tonic::Request::new(RepositoryCloseRequest { repo_id: repo_id.to_string() });
        Ok(client.repository_close(req).await?.into_inner())
    }

    pub async fn repository_list(&self) -> Result<badou_proto::RepositoryListResponse, BadouClientError> {
        let mut client = self.clone_client();
        let req = tonic::Request::new(RepositoryListRequest {});
        Ok(client.repository_list(req).await?.into_inner())
    }

    pub async fn repository_delete(&self, repo_id: &str, force: bool) -> Result<badou_proto::RepositoryDeleteResponse, BadouClientError> {
        let mut client = self.clone_client();
        let req = tonic::Request::new(RepositoryDeleteRequest { repo_id: repo_id.to_string(), force });
        Ok(client.repository_delete(req).await?.into_inner())
    }

    pub async fn repository_configure(&self, repo_id: &str, config: RepositoryConfig) -> Result<badou_proto::RepositoryConfigureResponse, BadouClientError> {
        let mut client = self.clone_client();
        let req = tonic::Request::new(RepositoryConfigureRequest { repo_id: repo_id.to_string(), config: Some(config) });
        Ok(client.repository_configure(req).await?.into_inner())
    }

    pub async fn repository_stat(&self, repo_id: &str) -> Result<badou_proto::RepositoryStatResponse, BadouClientError> {
        let mut client = self.clone_client();
        let req = tonic::Request::new(RepositoryStatRequest { repo_id: repo_id.to_string() });
        Ok(client.repository_stat(req).await?.into_inner())
    }

    pub async fn chunk_put(&self, repo_id: &str, chunk: badou_proto::ChunkData) -> Result<badou_proto::ChunkPutResponse, BadouClientError> {
        let mut client = self.clone_client();
        let req = tonic::Request::new(ChunkPutRequest { repo_id: repo_id.to_string(), chunk: Some(chunk) });
        Ok(client.chunk_put(req).await?.into_inner())
    }

    pub async fn chunk_get(&self, repo_id: &str, chunk_hash: &str) -> Result<badou_proto::ChunkGetResponse, BadouClientError> {
        let mut client = self.clone_client();
        let req = tonic::Request::new(ChunkGetRequest { repo_id: repo_id.to_string(), chunk_hash: chunk_hash.to_string() });
        Ok(client.chunk_get(req).await?.into_inner())
    }

    pub async fn chunk_exists(&self, repo_id: &str, chunk_hash: &str) -> Result<badou_proto::ChunkExistsResponse, BadouClientError> {
        let mut client = self.clone_client();
        let req = tonic::Request::new(ChunkExistsRequest { repo_id: repo_id.to_string(), chunk_hash: chunk_hash.to_string() });
        Ok(client.chunk_exists(req).await?.into_inner())
    }

    pub async fn chunk_delete(&self, repo_id: &str, chunk_hash: &str) -> Result<badou_proto::ChunkDeleteResponse, BadouClientError> {
        let mut client = self.clone_client();
        let req = tonic::Request::new(ChunkDeleteRequest { repo_id: repo_id.to_string(), chunk_hash: chunk_hash.to_string() });
        Ok(client.chunk_delete(req).await?.into_inner())
    }

    pub async fn snapshot_commit(&self, req: SnapshotCommitRequest) -> Result<badou_proto::SnapshotCommitResponse, BadouClientError> {
        let mut client = self.clone_client();
        Ok(client.snapshot_commit(tonic::Request::new(req)).await?.into_inner())
    }

    pub async fn snapshot_get(&self, repo_id: &str, snapshot_id: &str) -> Result<badou_proto::SnapshotGetResponse, BadouClientError> {
        let mut client = self.clone_client();
        let req = tonic::Request::new(SnapshotGetRequest { repo_id: repo_id.to_string(), snapshot_id: snapshot_id.to_string() });
        Ok(client.snapshot_get(req).await?.into_inner())
    }

    pub async fn snapshot_list(&self, repo_id: &str) -> Result<badou_proto::SnapshotListResponse, BadouClientError> {
        let mut client = self.clone_client();
        let req = tonic::Request::new(SnapshotListRequest { repo_id: repo_id.to_string(), limit: None, cursor: None });
        Ok(client.snapshot_list(req).await?.into_inner())
    }

    pub async fn snapshot_delete(&self, repo_id: &str, snapshot_id: &str) -> Result<badou_proto::SnapshotDeleteResponse, BadouClientError> {
        let mut client = self.clone_client();
        let req = tonic::Request::new(SnapshotDeleteRequest { repo_id: repo_id.to_string(), snapshot_id: snapshot_id.to_string() });
        Ok(client.snapshot_delete(req).await?.into_inner())
    }

    pub async fn verify_repository(&self, repo_id: &str, deep: bool) -> Result<tonic::Streaming<badou_proto::VerifyReport>, BadouClientError> {
        let mut client = self.clone_client();
        let req = tonic::Request::new(VerifyRepositoryRequest { repo_id: repo_id.to_string(), deep });
        Ok(client.verify_repository(req).await?.into_inner())
    }

    pub async fn verify_version(&self, repo_id: &str, version_id: &str, deep: bool) -> Result<tonic::Streaming<badou_proto::VerifyReport>, BadouClientError> {
        let mut client = self.clone_client();
        let req = tonic::Request::new(VerifyVersionRequest { repo_id: repo_id.to_string(), version_id: version_id.to_string(), deep });
        Ok(client.verify_version(req).await?.into_inner())
    }

    pub async fn verify_chunk(&self, repo_id: &str, chunk_hash: &str) -> Result<badou_proto::VerifyChunkResponse, BadouClientError> {
        let mut client = self.clone_client();
        let req = tonic::Request::new(VerifyChunkRequest { repo_id: repo_id.to_string(), chunk_hash: chunk_hash.to_string() });
        Ok(client.verify_chunk(req).await?.into_inner())
    }

    pub async fn recovery_open(&self, repo_id: &str, version_id: &str, file_path: Option<&str>) -> Result<tonic::Streaming<badou_proto::RecoveryChunk>, BadouClientError> {
        let mut client = self.clone_client();
        let req = tonic::Request::new(RecoveryOpenRequest {
            repo_id: repo_id.to_string(),
            version_id: version_id.to_string(),
            file_path: file_path.map(|s| s.to_string()),
        });
        Ok(client.recovery_open(req).await?.into_inner())
    }
}

pub fn map_status_to_hbop_error(status: &tonic::Status) -> BadouClientError {
    let code = match status.code() {
        tonic::Code::Unauthenticated => HbopErrorCode::AuthFailed,
        tonic::Code::PermissionDenied => HbopErrorCode::PermissionDenied,
        tonic::Code::NotFound => HbopErrorCode::RepoNotFound,
        tonic::Code::AlreadyExists => HbopErrorCode::RepoAlreadyExists,
        tonic::Code::FailedPrecondition => HbopErrorCode::StateConflict,
        tonic::Code::Aborted => HbopErrorCode::ImmutableConflict,
        tonic::Code::ResourceExhausted => HbopErrorCode::RateLimited,
        _ => HbopErrorCode::InternalError,
    };
    BadouClientError::Hbop { code, message: status.message().to_string() }
}
