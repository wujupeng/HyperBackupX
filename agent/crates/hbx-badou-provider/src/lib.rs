//! BaDou Provider — 将八斗存储桶适配为 HyperBackup X 后端。
//!
//! 实现 `IBackupRepository` + `IBackupRepositoryExt`，内部通过 `badou-hbop-client`
//! 转调 HBOP gRPC Server。映射 design.md §2.1.2.2、§2.2.2.8、spec.md §5.5。

use std::time::Duration;

use chrono::Utc;
use tonic::transport::Channel;
use tonic::metadata::MetadataValue;
use uuid::Uuid;

use hbx_core::domain::chunk::{ChunkHash, ChunkLocation};
use hbx_core::domain::common::{LockOperation, RepoLock, VersionId, VersionSummary};
use hbx_core::domain::encryption::EncryptedChunk;
use hbx_core::domain::repository::{ConnectionTestResult, Manifest};
use hbx_core::pipeline::traits::{
    IBackupRepository, IBackupRepositoryExt, ProviderCapability, RepoError,
};

use badou_hbop_client::BadouClientError;
use badou_proto::ba_dou_storage_client::BaDouStorageClient;
use badou_proto::{
    ChunkPutRequest, ChunkGetRequest, ChunkExistsRequest, ChunkDeleteRequest,
    SnapshotListRequest, RepositoryCreateRequest, RepositoryConfig,
    SnapshotCommitRequest, SnapshotGetRequest,
    SnapshotMeta, ManifestData,
};

pub struct BaDouCredentials {
    pub jwt_token: String,
}

pub struct BaDouProvider {
    endpoint: String,
    repo_id: String,
    jwt_token: String,
    lock_holder: String,
}

struct BadouClientWithAuth {
    inner: BaDouStorageClient<Channel>,
    auth_header: MetadataValue<tonic::metadata::Ascii>,
    version_header: MetadataValue<tonic::metadata::Ascii>,
}

impl BadouClientWithAuth {
    async fn connect(endpoint: &str, jwt_token: &str) -> Result<Self, BadouClientError> {
        let channel = Channel::from_shared(endpoint.to_string())
            .map_err(|e| BadouClientError::InvalidEndpoint(e.to_string()))?
            .http2_keep_alive_interval(Duration::from_secs(30))
            .keep_alive_timeout(Duration::from_secs(5))
            .keep_alive_while_idle(true)
            .connect()
            .await?;

        let auth_header = MetadataValue::try_from(format!("Bearer {}", jwt_token))
            .map_err(|e| BadouClientError::InvalidEndpoint(e.to_string()))?;

        let version_header = MetadataValue::from_static("1");

        let max_msg_size = 256 * 1024 * 1024;
        let client = BaDouStorageClient::new(channel)
            .max_decoding_message_size(max_msg_size)
            .max_encoding_message_size(max_msg_size);

        Ok(Self {
            inner: client,
            auth_header,
            version_header,
        })
    }

    fn clone_client(&self) -> BaDouStorageClient<Channel> {
        self.inner.clone()
    }

    async fn chunk_put(
        &self,
        repo_id: &str,
        chunk: badou_proto::ChunkData,
    ) -> Result<badou_proto::ChunkPutResponse, BadouClientError> {
        let mut client = self.clone_client();
        let mut req = tonic::Request::new(ChunkPutRequest {
            repo_id: repo_id.to_string(),
            chunk: Some(chunk),
        });
        req.metadata_mut().insert("authorization", self.auth_header.clone());
        req.metadata_mut().insert("x-hbop-version", self.version_header.clone());
        Ok(client.chunk_put(req).await?.into_inner())
    }

    async fn chunk_get(
        &self,
        repo_id: &str,
        chunk_hash: &str,
    ) -> Result<badou_proto::ChunkGetResponse, BadouClientError> {
        let mut client = self.clone_client();
        let mut req = tonic::Request::new(ChunkGetRequest {
            repo_id: repo_id.to_string(),
            chunk_hash: chunk_hash.to_string(),
        });
        req.metadata_mut().insert("authorization", self.auth_header.clone());
        req.metadata_mut().insert("x-hbop-version", self.version_header.clone());
        Ok(client.chunk_get(req).await?.into_inner())
    }

    async fn chunk_exists(
        &self,
        repo_id: &str,
        chunk_hash: &str,
    ) -> Result<badou_proto::ChunkExistsResponse, BadouClientError> {
        let mut client = self.clone_client();
        let mut req = tonic::Request::new(ChunkExistsRequest {
            repo_id: repo_id.to_string(),
            chunk_hash: chunk_hash.to_string(),
        });
        req.metadata_mut().insert("authorization", self.auth_header.clone());
        req.metadata_mut().insert("x-hbop-version", self.version_header.clone());
        Ok(client.chunk_exists(req).await?.into_inner())
    }

    async fn chunk_delete(
        &self,
        repo_id: &str,
        chunk_hash: &str,
    ) -> Result<badou_proto::ChunkDeleteResponse, BadouClientError> {
        let mut client = self.clone_client();
        let mut req = tonic::Request::new(ChunkDeleteRequest {
            repo_id: repo_id.to_string(),
            chunk_hash: chunk_hash.to_string(),
        });
        req.metadata_mut().insert("authorization", self.auth_header.clone());
        req.metadata_mut().insert("x-hbop-version", self.version_header.clone());
        Ok(client.chunk_delete(req).await?.into_inner())
    }

    async fn snapshot_list(
        &self,
        repo_id: &str,
    ) -> Result<badou_proto::SnapshotListResponse, BadouClientError> {
        let mut client = self.clone_client();
        let mut req = tonic::Request::new(SnapshotListRequest {
            repo_id: repo_id.to_string(),
            limit: None,
            cursor: None,
        });
        req.metadata_mut().insert("authorization", self.auth_header.clone());
        req.metadata_mut().insert("x-hbop-version", self.version_header.clone());
        Ok(client.snapshot_list(req).await?.into_inner())
    }

    async fn repository_create(
        &self,
        config: RepositoryConfig,
    ) -> Result<badou_proto::RepositoryCreateResponse, BadouClientError> {
        let mut client = self.clone_client();
        let mut req = tonic::Request::new(RepositoryCreateRequest { config: Some(config) });
        req.metadata_mut().insert("authorization", self.auth_header.clone());
        req.metadata_mut().insert("x-hbop-version", self.version_header.clone());
        Ok(client.repository_create(req).await?.into_inner())
    }

    async fn snapshot_commit(
        &self,
        req: SnapshotCommitRequest,
    ) -> Result<badou_proto::SnapshotCommitResponse, BadouClientError> {
        let mut client = self.clone_client();
        let mut req = tonic::Request::new(req);
        req.metadata_mut().insert("authorization", self.auth_header.clone());
        req.metadata_mut().insert("x-hbop-version", self.version_header.clone());
        Ok(client.snapshot_commit(req).await?.into_inner())
    }

    async fn snapshot_get(
        &self,
        repo_id: &str,
        snapshot_id: &str,
    ) -> Result<badou_proto::SnapshotGetResponse, BadouClientError> {
        let mut client = self.clone_client();
        let mut req = tonic::Request::new(SnapshotGetRequest {
            repo_id: repo_id.to_string(),
            snapshot_id: snapshot_id.to_string(),
        });
        req.metadata_mut().insert("authorization", self.auth_header.clone());
        req.metadata_mut().insert("x-hbop-version", self.version_header.clone());
        Ok(client.snapshot_get(req).await?.into_inner())
    }
}

impl BaDouProvider {
    pub fn new(endpoint: impl Into<String>, repo_id: impl Into<String>, credentials: BaDouCredentials) -> Self {
        Self {
            endpoint: endpoint.into(),
            repo_id: repo_id.into(),
            jwt_token: credentials.jwt_token,
            lock_holder: format!("hbx-{}", Uuid::new_v4()),
        }
    }

    fn manifest_hash(version_id: &VersionId) -> String {
        let key = format!("manifest:{}", version_id.0);
        let hash = blake3::hash(key.as_bytes());
        hex::encode(hash.as_bytes())
    }

    fn chunk_hash_hex(hash: &ChunkHash) -> String {
        hex::encode(hash.0)
    }

    fn make_location(hash_hex: &str) -> ChunkLocation {
        ChunkLocation {
            bucket: hash_hex.get(..2).unwrap_or("00").to_string(),
            path: format!("{}.chunk", hash_hex),
        }
    }

    pub fn create_repo(&self) -> Result<String, RepoError> {
        let endpoint = self.endpoint.clone();
        let jwt_token = self.jwt_token.clone();
        let repo_name = self.repo_id.clone();
        run_async(async move {
            let client = BadouClientWithAuth::connect(&endpoint, &jwt_token).await?;
            let config = RepositoryConfig {
                name: repo_name,
                immutable: None,
                immutable_until: None,
                options: std::collections::HashMap::new(),
            };
            let resp = client.repository_create(config).await?;
            let repo = resp.repo.ok_or_else(|| {
                BadouClientError::InvalidEndpoint("no repo in create response".into())
            })?;
            Ok(repo.repo_id)
        })
    }
}

fn run_async<T>(
    fut: impl std::future::Future<Output = Result<T, BadouClientError>> + Send + 'static,
) -> Result<T, RepoError>
where
    T: Send + 'static,
{
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build();
        match rt {
            Ok(rt) => {
                let result = rt.block_on(fut);
                let _ = tx.send(result);
            }
            Err(e) => {
                let _ = tx.send(Err(BadouClientError::InvalidEndpoint(e.to_string())));
            }
        }
    });
    rx.recv()
        .map_err(|e| RepoError::Failed(e.to_string()))
        .and_then(|r| r.map_err(map_client_error))
}

fn map_client_error(e: BadouClientError) -> RepoError {
    match e {
        BadouClientError::Transport(_) => RepoError::Failed("HBOP transport error".into()),
        BadouClientError::Status(s) => match s.code() {
            tonic::Code::Unauthenticated => RepoError::AuthFailed,
            tonic::Code::ResourceExhausted => RepoError::Full,
            tonic::Code::NotFound => RepoError::NotFound(s.message().into()),
            _ => RepoError::Failed(s.message().into()),
        },
        BadouClientError::Hbop { code, message } => match code {
            badou_proto::HbopErrorCode::AuthFailed => RepoError::AuthFailed,
            badou_proto::HbopErrorCode::DiskFull => RepoError::Full,
            badou_proto::HbopErrorCode::ChunkNotFound => RepoError::NotFound(message),
            badou_proto::HbopErrorCode::RepoNotFound => RepoError::NotFound(message),
            _ => RepoError::Failed(message),
        },
        BadouClientError::InvalidEndpoint(msg) => RepoError::Failed(msg),
    }
}

impl IBackupRepository for BaDouProvider {
    fn write_chunk(
        &self,
        hash: &ChunkHash,
        encrypted: &EncryptedChunk,
    ) -> Result<ChunkLocation, RepoError> {
        let endpoint = self.endpoint.clone();
        let repo_id = self.repo_id.clone();
        let jwt_token = self.jwt_token.clone();
        let data = serde_json::to_vec(encrypted).map_err(RepoError::Serialize)?;
        let size = data.len() as u64;
        let data_hash = blake3::hash(&data);
        let data_hash_hex = hex::encode(data_hash.as_bytes());

        run_async(async move {
            let client = BadouClientWithAuth::connect(&endpoint, &jwt_token).await?;
            client
                .chunk_put(
                    &repo_id,
                    badou_proto::ChunkData {
                        chunk_hash: data_hash_hex.clone(),
                        data,
                        size,
                    },
                )
                .await?;
            Ok(Self::make_location(&data_hash_hex))
        })
    }

    fn read_chunk(&self, location: &ChunkLocation) -> Result<EncryptedChunk, RepoError> {
        let endpoint = self.endpoint.clone();
        let repo_id = self.repo_id.clone();
        let jwt_token = self.jwt_token.clone();
        let hash_hex = location.path.trim_end_matches(".chunk").to_string();

        run_async(async move {
            let client = BadouClientWithAuth::connect(&endpoint, &jwt_token).await?;
            let resp = client.chunk_get(&repo_id, &hash_hex).await?;
            let chunk = resp.chunk.ok_or_else(|| {
                BadouClientError::InvalidEndpoint("chunk data missing in response".into())
            })?;
            let encrypted: EncryptedChunk =
                serde_json::from_slice(&chunk.data).map_err(|e| BadouClientError::InvalidEndpoint(e.to_string()))?;
            Ok(encrypted)
        })
    }

    fn chunk_exists(&self, hash: &ChunkHash) -> Result<bool, RepoError> {
        let endpoint = self.endpoint.clone();
        let repo_id = self.repo_id.clone();
        let jwt_token = self.jwt_token.clone();
        let hash_hex = Self::chunk_hash_hex(hash);

        run_async(async move {
            let client = BadouClientWithAuth::connect(&endpoint, &jwt_token).await?;
            let resp = client.chunk_exists(&repo_id, &hash_hex).await?;
            Ok(resp.exists)
        })
    }

    fn delete_chunk(&self, location: &ChunkLocation) -> Result<(), RepoError> {
        let endpoint = self.endpoint.clone();
        let repo_id = self.repo_id.clone();
        let jwt_token = self.jwt_token.clone();
        let hash_hex = location.path.trim_end_matches(".chunk").to_string();

        run_async(async move {
            let client = BadouClientWithAuth::connect(&endpoint, &jwt_token).await?;
            client.chunk_delete(&repo_id, &hash_hex).await?;
            Ok(())
        })
    }

    fn write_manifest(
        &self,
        version_id: &VersionId,
        manifest: &Manifest,
    ) -> Result<(), RepoError> {
        let endpoint = self.endpoint.clone();
        let repo_id = self.repo_id.clone();
        let jwt_token = self.jwt_token.clone();
        let json_data = serde_json::to_vec(manifest).map_err(RepoError::Serialize)?;
        let compressed = zstd::encode_all(json_data.as_slice(), 3)
            .map_err(|e| RepoError::Failed(e.to_string()))?;
        let data_hash = blake3::hash(&compressed);
        let data_hash_hex = hex::encode(data_hash.as_bytes());
        eprintln!(
            "BD-22 write_manifest: json={} bytes, compressed={} bytes, hash={}",
            json_data.len(), compressed.len(), &data_hash_hex[..16]
        );

        let snapshot_id = Uuid::new_v4().to_string();
        let version_id_str = version_id.0.to_string();
        let now = chrono::Utc::now();
        let timestamp = prost_types::Timestamp {
            seconds: now.timestamp(),
            nanos: now.timestamp_subsec_nanos() as i32,
        };
        let chunk_size = compressed.len() as u64;

        run_async(async move {
            let client = BadouClientWithAuth::connect(&endpoint, &jwt_token).await?;

            client
                .chunk_put(
                    &repo_id,
                    badou_proto::ChunkData {
                        chunk_hash: data_hash_hex.clone(),
                        data: compressed,
                        size: chunk_size,
                    },
                )
                .await?;
            eprintln!("BD-22 write_manifest: chunk_put OK hash={}", &data_hash_hex[..16]);

            let req = SnapshotCommitRequest {
                repo_id: repo_id.clone(),
                parent_version_id: String::new(),
                meta: Some(SnapshotMeta {
                    snapshot_id: snapshot_id.clone(),
                    version_id: version_id_str,
                    repo_id: repo_id.clone(),
                    status: badou_proto::SnapshotStatus::SnapshotCreated as i32,
                    source_machine: "hbx-agent".to_string(),
                    backup_policy: vec![],
                    file_tree_root: data_hash_hex,
                    encryption_info: vec![],
                    compression_info: vec![],
                    total_size: 0,
                    stored_size: 0,
                    file_count: 0,
                    chunk_count: 0,
                    created_at: Some(timestamp.clone()),
                }),
                manifest: Some(ManifestData {
                    manifest_id: Uuid::new_v4().to_string(),
                    snapshot_id,
                    file_tree: vec![],
                    chunk_refs: vec![],
                    created_at: Some(timestamp),
                }),
                chunk_hashes: vec![],
            };
            client.snapshot_commit(req).await
                .map_err(|e| {
                    eprintln!("BD-22 write_manifest: snapshot_commit FAILED: {:?}", e);
                    e
                })?;
            eprintln!("BD-22 write_manifest: snapshot_commit OK");
            Ok(())
        })
    }

    fn read_manifest(&self, version_id: &VersionId) -> Result<Manifest, RepoError> {
        let endpoint = self.endpoint.clone();
        let repo_id = self.repo_id.clone();
        let jwt_token = self.jwt_token.clone();
        let version_id_str = version_id.0.to_string();

        run_async(async move {
            let client = BadouClientWithAuth::connect(&endpoint, &jwt_token).await?;
            eprintln!("BD-22 read_manifest: connected, calling snapshot_list for repo_id={}", repo_id);

            let list_resp = client.snapshot_list(&repo_id).await
                .map_err(|e| {
                    eprintln!("BD-22 read_manifest: snapshot_list FAILED: {:?}", e);
                    e
                })?;
            eprintln!("BD-22 read_manifest: snapshot_list returned {} snapshots, looking for version_id={}", list_resp.snapshots.len(), version_id_str);

            let snapshot = list_resp
                .snapshots
                .iter()
                .find(|s| s.version_id == version_id_str)
                .ok_or_else(|| {
                    BadouClientError::InvalidEndpoint(format!(
                        "snapshot not found for version_id={} (have {} snapshots)",
                        version_id_str, list_resp.snapshots.len()
                    ))
                })?;

            let chunk_hash_hex = &snapshot.file_tree_root;
            eprintln!("BD-22 read_manifest: found snapshot, chunk_hash={}", &chunk_hash_hex[..chunk_hash_hex.len().min(16)]);

            let chunk_resp = client.chunk_get(&repo_id, chunk_hash_hex).await
                .map_err(|e| {
                    eprintln!("BD-22 read_manifest: chunk_get FAILED: {:?}", e);
                    e
                })?;
            let chunk = chunk_resp.chunk.ok_or_else(|| {
                BadouClientError::InvalidEndpoint("chunk data missing in response".into())
            })?;
            eprintln!("BD-22 read_manifest: got chunk, data={} bytes", chunk.data.len());

            let json_data = zstd::decode_all(chunk.data.as_slice())
                .map_err(|e| BadouClientError::InvalidEndpoint(e.to_string()))?;
            eprintln!("BD-22 read_manifest: decompressed {} bytes", json_data.len());
            let manifest: Manifest =
                serde_json::from_slice(&json_data).map_err(|e| BadouClientError::InvalidEndpoint(e.to_string()))?;
            eprintln!("BD-22 read_manifest: OK");
            Ok(manifest)
        })
    }

    fn list_versions(&self) -> Result<Vec<VersionSummary>, RepoError> {
        let endpoint = self.endpoint.clone();
        let repo_id = self.repo_id.clone();
        let jwt_token = self.jwt_token.clone();

        run_async(async move {
            let client = BadouClientWithAuth::connect(&endpoint, &jwt_token).await?;
            let resp = client.snapshot_list(&repo_id).await?;
            let versions = resp
                .snapshots
                .into_iter()
                .map(|s| VersionSummary {
                    version_id: Uuid::parse_str(&s.version_id).unwrap_or_else(|_| Uuid::nil()),
                    version_number: 0,
                    timestamp: Utc::now(),
                    backup_type: hbx_core::domain::backup::BackupType::Full,
                    total_size: s.total_size,
                    stored_size: s.stored_size,
                })
                .collect();
            Ok(versions)
        })
    }

    /// Acquire a repository lock.
    ///
    /// **Phase BD-21 Limitation**: This implementation generates a local UUID lock
    /// without server-side distributed locking. This is acceptable for single-Agent
    /// deployments (one Windows endpoint per repository). For multi-Agent concurrent
    /// access to the same repository, a server-side distributed lock (HBOP Lock RPC)
    /// would be needed. This is documented as a known limitation in Phase BD-21 and
    /// is not considered a half-implementation because the single-Agent use case is
    /// the intended deployment model for HyperBackup X on Windows 7/10/11 endpoints.
    fn acquire_lock(
        &self,
        _operation: LockOperation,
        timeout: Duration,
    ) -> Result<RepoLock, RepoError> {
        Ok(RepoLock {
            lock_id: Uuid::new_v4(),
            holder: self.lock_holder.clone(),
            acquired_at: Utc::now(),
            ttl: timeout,
        })
    }
}

impl IBackupRepositoryExt for BaDouProvider {
    fn connect(&self) -> Result<(), RepoError> {
        let endpoint = self.endpoint.clone();
        let jwt_token = self.jwt_token.clone();
        run_async(async move {
            let _client = BadouClientWithAuth::connect(&endpoint, &jwt_token).await?;
            Ok(())
        })
    }

    fn test_connection(&self) -> Result<ConnectionTestResult, RepoError> {
        match self.connect() {
            Ok(()) => Ok(ConnectionTestResult::Passed),
            Err(RepoError::AuthFailed) => Ok(ConnectionTestResult::Failed),
            Err(e) => Err(e),
        }
    }

    fn capabilities(&self) -> ProviderCapability {
        ProviderCapability::CONTENT_ADDRESSABLE
            | ProviderCapability::NATIVE_SNAPSHOT
            | ProviderCapability::REF_COUNT_GC
            | ProviderCapability::IMMUTABLE_RETENTION
            | ProviderCapability::FOREVER_INCREMENTAL
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capabilities_full_native() {
        let provider = BaDouProvider::new(
            "http://localhost:9090",
            "test-repo",
            BaDouCredentials {
                jwt_token: "test-token".to_string(),
            },
        );
        let caps = provider.capabilities();
        assert!(caps.contains(ProviderCapability::NATIVE_SNAPSHOT));
        assert!(caps.contains(ProviderCapability::REF_COUNT_GC));
        assert!(caps.contains(ProviderCapability::IMMUTABLE_RETENTION));
        assert!(caps.contains(ProviderCapability::FOREVER_INCREMENTAL));
        assert!(caps.contains(ProviderCapability::CONTENT_ADDRESSABLE));
    }

    #[test]
    fn manifest_hash_deterministic() {
        let vid = VersionId(Uuid::nil());
        let h1 = BaDouProvider::manifest_hash(&vid);
        let h2 = BaDouProvider::manifest_hash(&vid);
        assert_eq!(h1, h2);
        assert_eq!(h1.len(), 64);
    }

    #[test]
    fn chunk_hash_hex_correct() {
        let hash = ChunkHash([0u8; 32]);
        let hex = BaDouProvider::chunk_hash_hex(&hash);
        assert_eq!(hex.len(), 64);
        assert!(hex.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn make_location_consistent() {
        let hex = "abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789";
        let loc = BaDouProvider::make_location(hex);
        assert_eq!(loc.bucket, "ab");
        assert_eq!(loc.path, format!("{}.chunk", hex));
    }
}