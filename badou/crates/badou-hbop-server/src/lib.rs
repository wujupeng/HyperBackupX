//! HBOP gRPC Server。
//!
//! 实现 `BaDouStorage` service 全部 20 RPC，强制 mTLS + JWT 鉴权 + 版本协商 + 统一错误码。
//! 映射 design.md §2.2.2.1-2.2.2.7、spec.md §5.1、C-SEC-BD-001/002。

#![allow(clippy::result_large_err)]

pub mod auth;
pub mod error;
pub mod state;
pub mod convert;
pub mod repository_rpc;
pub mod chunk_rpc;
pub mod snapshot_rpc;
pub mod verify_rpc;
pub mod recovery_rpc;


use std::sync::Arc;
use tonic::{Request, Response, Status};
use tonic::transport::ServerTlsConfig;
use tonic::transport::Identity;

use badou_proto::ba_dou_storage_server::{BaDouStorage, BaDouStorageServer};
use badou_proto::{
    RepositoryCreateRequest, RepositoryCreateResponse,
    RepositoryOpenRequest, RepositoryOpenResponse,
    RepositoryCloseRequest, RepositoryCloseResponse,
    RepositoryListRequest, RepositoryListResponse,
    RepositoryDeleteRequest, RepositoryDeleteResponse,
    RepositoryConfigureRequest, RepositoryConfigureResponse,
    RepositoryStatRequest, RepositoryStatResponse,
    ChunkPutRequest, ChunkPutResponse,
    ChunkGetRequest, ChunkGetResponse,
    ChunkExistsRequest, ChunkExistsResponse,
    ChunkDeleteRequest, ChunkDeleteResponse,
    ChunkBatchPutResponse,
    SnapshotCommitRequest, SnapshotCommitResponse,
    SnapshotGetRequest, SnapshotGetResponse,
    SnapshotListRequest, SnapshotListResponse,
    SnapshotDeleteRequest, SnapshotDeleteResponse,
    VerifyRepositoryRequest, VerifyReport,
    VerifyVersionRequest,
    VerifyChunkRequest, VerifyChunkResponse,
    RecoveryOpenRequest, RecoveryChunk,
};

use state::ServerState;
use auth::AuthConfig;

/// HBOP gRPC Server 组装 tonic Server + badou-ops/gc/verify/recovery。
pub struct BadouHbopServer {
    state: Arc<ServerState>,
}

impl BadouHbopServer {
    /// 创建服务端实例。
    pub fn new(data_root: impl AsRef<std::path::Path>, auth_config: AuthConfig) -> Self {
        Self {
            state: Arc::new(ServerState::new(data_root, auth_config)),
        }
    }

    /// 启动 gRPC Server（明文，仅用于测试/开发）。
    pub async fn serve(
        self,
        addr: std::net::SocketAddr,
    ) -> Result<(), tonic::transport::Error> {
        tonic::transport::Server::builder()
            .add_service(BaDouStorageServer::new(self))
            .serve(addr)
            .await
    }

    /// 启动 gRPC Server（mTLS 强制，C-SEC-BD-001）。
    pub async fn serve_with_tls(
        self,
        addr: std::net::SocketAddr,
        server_cert_pem: Vec<u8>,
        server_key_pem: Vec<u8>,
        client_ca_cert_pem: Vec<u8>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let identity = Identity::from_pem(server_cert_pem, server_key_pem);

        let tls = ServerTlsConfig::new()
            .identity(identity)
            .client_ca_root(tonic::transport::Certificate::from_pem(client_ca_cert_pem));

        tonic::transport::Server::builder()
            .tls_config(tls)?
            .add_service(BaDouStorageServer::new(self))
            .serve(addr)
            .await?;

        Ok(())
    }

    /// 获取共享状态引用（供外部测试/集成使用）。
    pub fn state(&self) -> &Arc<ServerState> {
        &self.state
    }
}

#[tonic::async_trait]
impl BaDouStorage for BadouHbopServer {
    async fn repository_create(
        &self,
        request: Request<RepositoryCreateRequest>,
    ) -> Result<Response<RepositoryCreateResponse>, Status> {
        repository_rpc::repository_create(&self.state, request).await
    }

    async fn repository_open(
        &self,
        request: Request<RepositoryOpenRequest>,
    ) -> Result<Response<RepositoryOpenResponse>, Status> {
        repository_rpc::repository_open(&self.state, request).await
    }

    async fn repository_close(
        &self,
        request: Request<RepositoryCloseRequest>,
    ) -> Result<Response<RepositoryCloseResponse>, Status> {
        repository_rpc::repository_close(&self.state, request).await
    }

    async fn repository_list(
        &self,
        request: Request<RepositoryListRequest>,
    ) -> Result<Response<RepositoryListResponse>, Status> {
        repository_rpc::repository_list(&self.state, request).await
    }

    async fn repository_delete(
        &self,
        request: Request<RepositoryDeleteRequest>,
    ) -> Result<Response<RepositoryDeleteResponse>, Status> {
        repository_rpc::repository_delete(&self.state, request).await
    }

    async fn repository_configure(
        &self,
        request: Request<RepositoryConfigureRequest>,
    ) -> Result<Response<RepositoryConfigureResponse>, Status> {
        repository_rpc::repository_configure(&self.state, request).await
    }

    async fn repository_stat(
        &self,
        request: Request<RepositoryStatRequest>,
    ) -> Result<Response<RepositoryStatResponse>, Status> {
        repository_rpc::repository_stat(&self.state, request).await
    }

    async fn chunk_put(
        &self,
        request: Request<ChunkPutRequest>,
    ) -> Result<Response<ChunkPutResponse>, Status> {
        chunk_rpc::chunk_put(&self.state, request).await
    }

    async fn chunk_get(
        &self,
        request: Request<ChunkGetRequest>,
    ) -> Result<Response<ChunkGetResponse>, Status> {
        chunk_rpc::chunk_get(&self.state, request).await
    }

    async fn chunk_exists(
        &self,
        request: Request<ChunkExistsRequest>,
    ) -> Result<Response<ChunkExistsResponse>, Status> {
        chunk_rpc::chunk_exists(&self.state, request).await
    }

    async fn chunk_delete(
        &self,
        request: Request<ChunkDeleteRequest>,
    ) -> Result<Response<ChunkDeleteResponse>, Status> {
        chunk_rpc::chunk_delete(&self.state, request).await
    }


    async fn chunk_batch_put(
        &self,
        request: Request<tonic::Streaming<ChunkPutRequest>>,
    ) -> Result<Response<ChunkBatchPutResponse>, Status> {
        chunk_rpc::chunk_batch_put(&self.state, request).await
    }

    type VerifyRepositoryStream = tokio_stream::wrappers::ReceiverStream<Result<VerifyReport, Status>>;

    async fn verify_repository(
        &self,
        request: Request<VerifyRepositoryRequest>,
    ) -> Result<Response<Self::VerifyRepositoryStream>, Status> {
        verify_rpc::verify_repository(&self.state, request).await
    }

    type VerifyVersionStream = tokio_stream::wrappers::ReceiverStream<Result<VerifyReport, Status>>;

    async fn verify_version(
        &self,
        request: Request<VerifyVersionRequest>,
    ) -> Result<Response<Self::VerifyVersionStream>, Status> {
        verify_rpc::verify_version(&self.state, request).await
    }

    async fn verify_chunk(
        &self,
        request: Request<VerifyChunkRequest>,
    ) -> Result<Response<VerifyChunkResponse>, Status> {
        verify_rpc::verify_chunk(&self.state, request).await
    }

    type RecoveryOpenStream = tokio_stream::wrappers::ReceiverStream<Result<RecoveryChunk, Status>>;

    async fn recovery_open(
        &self,
        request: Request<RecoveryOpenRequest>,
    ) -> Result<Response<Self::RecoveryOpenStream>, Status> {
        recovery_rpc::recovery_open(&self.state, request).await
    }

    async fn snapshot_commit(
        &self,
        request: Request<SnapshotCommitRequest>,
    ) -> Result<Response<SnapshotCommitResponse>, Status> {
        snapshot_rpc::snapshot_commit(&self.state, request).await
    }

    async fn snapshot_get(
        &self,
        request: Request<SnapshotGetRequest>,
    ) -> Result<Response<SnapshotGetResponse>, Status> {
        snapshot_rpc::snapshot_get(&self.state, request).await
    }

    async fn snapshot_list(
        &self,
        request: Request<SnapshotListRequest>,
    ) -> Result<Response<SnapshotListResponse>, Status> {
        snapshot_rpc::snapshot_list(&self.state, request).await
    }

    async fn snapshot_delete(
        &self,
        request: Request<SnapshotDeleteRequest>,
    ) -> Result<Response<SnapshotDeleteResponse>, Status> {
        snapshot_rpc::snapshot_delete(&self.state, request).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;
    use badou_proto::ba_dou_storage_client::BaDouStorageClient;
    use badou_proto::{
        RepositoryCreateRequest, RepositoryConfig, RepositoryListRequest,
        ChunkPutRequest, ChunkData, ChunkGetRequest,
    };
    use auth::{AuthConfig, METADATA_AUTH, METADATA_VERSION, generate_jwt};
    use auth::JwtClaims;
    use chrono::Utc;
    use std::net::SocketAddr;
    use tonic::metadata::MetadataValue;

    fn make_jwt_token(secret: &[u8], role: &str) -> String {
        let claims = JwtClaims {
            sub: "test-user".to_string(),
            role: role.to_string(),
            exp: (Utc::now().timestamp() + 3600) as u64,
            iat: Utc::now().timestamp() as u64,
        };
        generate_jwt(secret, &claims).unwrap()
    }

    async fn start_test_server() -> SocketAddr {
        let tmp = tempfile::tempdir().unwrap();
        let data_root = tmp.path().to_path_buf();
        std::mem::forget(tmp);

        let auth_config = AuthConfig::from_secret(b"test-secret");
        let server = BadouHbopServer::new(data_root, auth_config);

        let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
        let listener = std::net::TcpListener::bind(addr).unwrap();
        let local_addr = listener.local_addr().unwrap();
        drop(listener);

        tokio::spawn(async move {
            let _ = server.serve(local_addr).await;
        });

        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        local_addr
    }

    fn make_auth_metadata(token: &str) -> tonic::metadata::MetadataMap {
        let mut metadata = tonic::metadata::MetadataMap::new();
        metadata.insert(METADATA_AUTH, MetadataValue::from_str(&format!("Bearer {}", token)).unwrap());
        metadata.insert(METADATA_VERSION, MetadataValue::from_str("1").unwrap());
        metadata
    }

    fn make_request<T>(metadata: &tonic::metadata::MetadataMap, msg: T) -> Request<T> {
        Request::from_parts(metadata.clone(), tonic::Extensions::default(), msg)
    }

    #[tokio::test]
    async fn repository_create_and_list() {
        let addr = start_test_server().await;
        let channel = tonic::transport::Channel::from_shared(format!("http://{}", addr))
            .unwrap().connect().await.unwrap();
        let mut client = BaDouStorageClient::new(channel);

        let token = make_jwt_token(b"test-secret", "admin");
        let metadata = make_auth_metadata(&token);

        let create_req = make_request(&metadata, RepositoryCreateRequest {
            config: Some(RepositoryConfig {
                name: "test-repo".to_string(),
                immutable: None,
                immutable_until: None,
                options: std::collections::HashMap::new(),
            }),
        });

        let resp = client.repository_create(create_req).await.unwrap();
        let repo_info = resp.into_inner().repo.unwrap();
        assert_eq!(repo_info.name, "test-repo");

        let list_req = make_request(&metadata, RepositoryListRequest {});
        let list_resp = client.repository_list(list_req).await.unwrap();
        let repos = list_resp.into_inner().repos;
        assert!(!repos.is_empty());
    }

    #[tokio::test]
    async fn unauthenticated_request_fails() {
        let addr = start_test_server().await;
        let channel = tonic::transport::Channel::from_shared(format!("http://{}", addr))
            .unwrap().connect().await.unwrap();
        let mut client = BaDouStorageClient::new(channel);

        let req = Request::new(RepositoryListRequest {});
        let result = client.repository_list(req).await;
        assert!(result.is_err());
        let status = result.unwrap_err();
        assert_eq!(status.code(), tonic::Code::Unauthenticated);
    }

    #[tokio::test]
    async fn viewer_cannot_write() {
        let addr = start_test_server().await;
        let channel = tonic::transport::Channel::from_shared(format!("http://{}", addr))
            .unwrap().connect().await.unwrap();
        let mut client = BaDouStorageClient::new(channel);

        let token = make_jwt_token(b"test-secret", "viewer");
        let metadata = make_auth_metadata(&token);

        let create_req = make_request(&metadata, RepositoryCreateRequest {
            config: Some(RepositoryConfig {
                name: "viewer-test".to_string(),
                immutable: None,
                immutable_until: None,
                options: std::collections::HashMap::new(),
            }),
        });

        let result = client.repository_create(create_req).await;
        assert!(result.is_err());
        let status = result.unwrap_err();
        assert_eq!(status.code(), tonic::Code::PermissionDenied);
    }

    #[tokio::test]
    async fn version_mismatch_fails() {
        let addr = start_test_server().await;
        let channel = tonic::transport::Channel::from_shared(format!("http://{}", addr))
            .unwrap().connect().await.unwrap();
        let mut client = BaDouStorageClient::new(channel);

        let token = make_jwt_token(b"test-secret", "admin");
        let mut metadata = tonic::metadata::MetadataMap::new();
        metadata.insert(METADATA_AUTH, MetadataValue::from_str(&format!("Bearer {}", token)).unwrap());
        metadata.insert(METADATA_VERSION, MetadataValue::from_str("99").unwrap());

        let list_req = make_request(&metadata, RepositoryListRequest {});
        let result = client.repository_list(list_req).await;
        assert!(result.is_err());
        let status = result.unwrap_err();
        assert_eq!(status.code(), tonic::Code::InvalidArgument);
    }

    #[tokio::test]
    async fn chunk_put_and_get() {
        let addr = start_test_server().await;
        let channel = tonic::transport::Channel::from_shared(format!("http://{}", addr))
            .unwrap().connect().await.unwrap();
        let mut client = BaDouStorageClient::new(channel);

        let token = make_jwt_token(b"test-secret", "admin");
        let metadata = make_auth_metadata(&token);

        let create_req = make_request(&metadata, RepositoryCreateRequest {
            config: Some(RepositoryConfig {
                name: "chunk-test-repo".to_string(),
                immutable: None,
                immutable_until: None,
                options: std::collections::HashMap::new(),
            }),
        });
        let create_resp = client.repository_create(create_req).await.unwrap();
        let repo_id = create_resp.into_inner().repo.unwrap().repo_id;

        let data = b"test chunk data for put and get";
        let hash = blake3::hash(data);
        let hash_hex = hex::encode(hash.as_bytes());

        let put_req = make_request(&metadata, ChunkPutRequest {
            repo_id: repo_id.clone(),
            chunk: Some(ChunkData {
                chunk_hash: hash_hex.clone(),
                data: data.to_vec(),
                size: data.len() as u64,
            }),
        });
        let put_resp = client.chunk_put(put_req).await.unwrap();
        let info = put_resp.into_inner().info.unwrap();
        assert_eq!(info.chunk_hash, hash_hex);

        let get_req = make_request(&metadata, ChunkGetRequest {
            repo_id: repo_id.clone(),
            chunk_hash: hash_hex.clone(),
        });
        let get_resp = client.chunk_get(get_req).await.unwrap();
        let chunk = get_resp.into_inner().chunk.unwrap();
        assert_eq!(chunk.data, data.to_vec());
    }
}
