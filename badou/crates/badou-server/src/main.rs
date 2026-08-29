mod config;

use std::sync::Arc;
use anyhow::Result;
use badou_health::{BadouMetrics, HealthChecker, MetricsRegistry};
use badou_hbop_server::{BadouHbopServer, auth::AuthConfig};
use badou_cluster::single_node::{SingleNodeConfig, SingleNodeMode};
use tracing::{info, error, warn};

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let config_path: String = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "badou-server.json".to_string());

    let config = match config::ServerConfig::load(std::path::Path::new(&config_path)) {
        Ok(c) => c,
        Err(e) => {
            error!("加载配置失败 ({}): {}", config_path, e);
            std::process::exit(1);
        }
    };

    run_server(config).await
}

async fn run_server(config: config::ServerConfig) -> Result<()> {
    let config::ServerConfig {
        data_root,
        bind_addr,
        metrics_addr,
        management_addr,
        jwt_secret,
        tls,
        cluster,
    } = config;

    info!("八斗存储桶服务器启动中...");
    info!("数据目录: {:?}", data_root);
    info!("gRPC 监听: {}", bind_addr);
    info!("Prometheus 指标: {}", metrics_addr);
    if let Some(ref mgmt) = management_addr {
        info!("管理 API: {}", mgmt);
    }

    match &cluster {
        config::ClusterConfig::Single => info!("集群模式: 单节点"),
        config::ClusterConfig::Raft { node_id, peers } => {
            info!("集群模式: Raft (node_id={}, peers={:?})", node_id, peers);
        }
    }

    let single_node = SingleNodeMode::new(SingleNodeConfig {
        data_root: data_root.clone(),
        bind_addr: bind_addr.clone(),
        jwt_secret: jwt_secret.as_bytes().to_vec(),
        tls: None,
    });
    single_node.validate()?;

    let health_checker = Arc::new(HealthChecker::new("node-1"));
    let metrics = Arc::new(BadouMetrics::new());

    let report = health_checker.check();
    metrics.update_from_health(&report);

    let metrics_registry = metrics.registry().clone();
    let metrics_addr_parsed: std::net::SocketAddr = metrics_addr.parse()?;
    tokio::spawn(async move {
        if let Err(e) = serve_metrics(metrics_addr_parsed, metrics_registry).await {
            error!("Prometheus 指标端点错误: {}", e);
        }
    });

    if let Some(mgmt_addr_str) = management_addr {
        let mgmt_addr_parsed: std::net::SocketAddr = mgmt_addr_str.parse()?;
        let mgmt_data_root = data_root.clone();
        tokio::spawn(async move {
            if let Err(e) = serve_management_api(mgmt_addr_parsed, mgmt_data_root).await {
                error!("管理 API 端点错误: {}", e);
            }
        });
    }

    let auth_config = AuthConfig::from_secret(jwt_secret.as_bytes());
    let server = BadouHbopServer::new(&data_root, auth_config);

    let bind_addr_parsed: std::net::SocketAddr = bind_addr.parse()?;

    info!("八斗存储桶服务器就绪，等待连接...");

    let server_handle = tokio::spawn(async move {
        match tls {
            Some(tls_paths) => {
                let server_cert = std::fs::read(&tls_paths.server_cert);
                let server_key = std::fs::read(&tls_paths.server_key);
                let client_ca = std::fs::read(&tls_paths.client_ca_cert);
                match (server_cert, server_key, client_ca) {
                    (Ok(cert), Ok(key), Ok(ca)) => {
                        if let Err(e) = server.serve_with_tls(bind_addr_parsed, cert, key, ca).await {
                            error!("gRPC Server (TLS) 错误: {}", e);
                        }
                    }
                    _ => {
                        error!("读取 TLS 证书文件失败");
                    }
                }
            }
            None => {
                if let Err(e) = server.serve(bind_addr_parsed).await {
                    error!("gRPC Server 错误: {}", e);
                }
            }
        }
    });

    match tokio::signal::ctrl_c().await {
        Ok(()) => info!("收到关闭信号 (Ctrl+C)，正在关闭..."),
        Err(e) => error!("无法监听关闭信号: {}", e),
    }

    server_handle.abort();
    info!("八斗存储桶服务器已关闭");
    Ok(())
}

async fn serve_metrics(
    addr: std::net::SocketAddr,
    registry: Arc<MetricsRegistry>,
) -> Result<()> {
    let listener = tokio::net::TcpListener::bind(addr).await?;
    info!("Prometheus 指标端点: http://{}/metrics", addr);

    loop {
        let (mut stream, peer_addr) = listener.accept().await?;
        let registry = registry.clone();

        tokio::spawn(async move {
            use tokio::io::{AsyncReadExt, AsyncWriteExt};

            let mut buf = [0u8; 1024];
            let _ = stream.read(&mut buf).await;

            let body = registry.render();
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: text/plain; version=0.0.4\r\nContent-Length: {}\r\n\r\n{}",
                body.len(),
                body
            );

            if let Err(e) = stream.write_all(response.as_bytes()).await {
                warn!("写入指标响应失败 ({}): {}", peer_addr, e);
            }
            let _ = stream.flush().await;
        });
    }
}

async fn serve_management_api(
    addr: std::net::SocketAddr,
    data_root: std::path::PathBuf,
) -> Result<()> {
    let listener = tokio::net::TcpListener::bind(addr).await?;
    info!("管理 API 端点: http://{}/api/v1", addr);

    loop {
        let (mut stream, peer_addr) = listener.accept().await?;
        let data_root = data_root.clone();

        tokio::spawn(async move {
            use tokio::io::{AsyncReadExt, AsyncWriteExt};

            let mut buf = vec![0u8; 65536];
            let n = match stream.read(&mut buf).await {
                Ok(n) if n > 0 => n,
                _ => return,
            };
            let request = String::from_utf8_lossy(&buf[..n]).to_string();
            let response = handle_management_request(&request, &data_root);

            if let Err(e) = stream.write_all(response.as_bytes()).await {
                warn!("写入管理 API 响应失败 ({}): {}", peer_addr, e);
            }
            let _ = stream.flush().await;
        });
    }
}

fn handle_management_request(request: &str, data_root: &std::path::Path) -> String {
    let lines: Vec<&str> = request.lines().collect();
    if lines.is_empty() {
        return http_json_response(400, &serde_json::json!({"error": "empty request"}));
    }

    let first_line: Vec<&str> = lines[0].split_whitespace().collect();
    if first_line.len() < 2 {
        return http_json_response(400, &serde_json::json!({"error": "malformed request line"}));
    }
    let method = first_line[0];
    let path = first_line[1];

    let body_start = request.find("\r\n\r\n").map(|i| i + 4).unwrap_or(request.len());
    let body = &request[body_start..];

    match (method, path) {
        ("GET", "/health") => {
            http_json_response(200, &serde_json::json!({"status": "healthy", "data_root": data_root.display().to_string()}))
        }
        ("GET", p) if p.starts_with("/api/v1/repos/") && p.ends_with("/versions") => {
            let repo_id = extract_repo_id(p, "/versions");
            handle_list_versions(data_root, &repo_id)
        }
        ("DELETE", p) if p.starts_with("/api/v1/repos/") && p.contains("/versions/") => {
            let parts: Vec<&str> = p.strip_prefix("/api/v1/repos/").unwrap_or("").split('/').collect();
            if parts.len() >= 4 && parts[1] == "versions" {
                handle_delete_version(data_root, parts[0], parts[2])
            } else {
                http_json_response(400, &serde_json::json!({"error": "invalid path"}))
            }
        }
        ("POST", p) if p.starts_with("/api/v1/repos/") && p.ends_with("/verify") => {
            let repo_id = extract_repo_id(p, "/verify");
            handle_verify_repo(data_root, &repo_id, body)
        }
        ("POST", p) if p.starts_with("/api/v1/repos/") && p.ends_with("/gc") => {
            let repo_id = extract_repo_id(p, "/gc");
            handle_trigger_gc(data_root, &repo_id)
        }
        _ => {
            http_json_response(404, &serde_json::json!({"error": format!("not found: {} {}", method, path)}))
        }
    }
}

fn extract_repo_id(path: &str, suffix: &str) -> String {
    let prefix = "/api/v1/repos/";
    path.strip_prefix(prefix)
        .and_then(|s| s.strip_suffix(suffix))
        .unwrap_or("")
        .to_string()
}

fn handle_list_versions(data_root: &std::path::Path, repo_id: &str) -> String {
    if repo_id.is_empty() {
        return http_json_response(400, &serde_json::json!({"error": "repo_id required"}));
    }
    let snapshots_dir = data_root.join("repositories").join(repo_id).join("snapshots");
    match std::fs::read_dir(&snapshots_dir) {
        Ok(entries) => {
            let mut versions = Vec::new();
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) == Some("json") {
                    let filename = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
                    if let Ok(data) = std::fs::read_to_string(&path) {
                        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&data) {
                            versions.push(serde_json::json!({
                                "version_id": json.get("version_id").unwrap_or(&serde_json::json!(filename)),
                                "snapshot_id": json.get("snapshot_id").unwrap_or(&serde_json::json!(filename)),
                                "created_at": json.get("created_at").unwrap_or(&serde_json::json!(null)),
                                "size": json.get("total_size").unwrap_or(&serde_json::json!(0)),
                                "chunk_count": json.get("chunk_count").unwrap_or(&serde_json::json!(0)),
                                "status": json.get("status").unwrap_or(&serde_json::json!("unknown")),
                            }));
                        }
                    }
                }
            }
            http_json_response(200, &serde_json::json!({"versions": versions}))
        }
        Err(_) => http_json_response(200, &serde_json::json!({"versions": []})),
    }
}

fn handle_delete_version(data_root: &std::path::Path, repo_id: &str, version_id: &str) -> String {
    if repo_id.is_empty() || version_id.is_empty() {
        return http_json_response(400, &serde_json::json!({"error": "repo_id and version_id required"}));
    }
    let snapshots_dir = data_root.join("repositories").join(repo_id).join("snapshots");
    let mut deleted = false;
    if let Ok(entries) = std::fs::read_dir(&snapshots_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            let filename = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
            if filename == version_id {
                if std::fs::remove_file(&path).is_ok() {
                    deleted = true;
                }
                break;
            }
        }
    }
    http_json_response(200, &serde_json::json!({"deleted": deleted}))
}

fn handle_verify_repo(data_root: &std::path::Path, repo_id: &str, _body: &str) -> String {
    if repo_id.is_empty() {
        return http_json_response(400, &serde_json::json!({"error": "repo_id required"}));
    }
    let chunks_dir = data_root.join("repositories").join(repo_id).join("chunks");
    let mut total_checked = 0u64;
    let mut total_failed = 0u64;
    if let Ok(entries) = std::fs::read_dir(&chunks_dir) {
        for entry in entries.flatten() {
            if entry.path().is_dir() {
                if let Ok(sub_entries) = std::fs::read_dir(entry.path()) {
                    for chunk_entry in sub_entries.flatten() {
                        let chunk_path = chunk_entry.path();
                        if chunk_path.extension().and_then(|e| e.to_str()) == Some("chunk") {
                            total_checked += 1;
                            if let Ok(data) = std::fs::read(&chunk_path) {
                                let actual = blake3::hash(&data);
                                let filename = chunk_path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
                                let expected_hex = filename.to_string();
                                if hex::encode(actual.as_bytes()) != expected_hex {
                                    total_failed += 1;
                                }
                            } else {
                                total_failed += 1;
                            }
                        }
                    }
                }
            }
        }
    }
    http_json_response(200, &serde_json::json!({
        "repo_id": repo_id,
        "passed": total_failed == 0,
        "total_checked": total_checked,
        "total_failed": total_failed,
    }))
}

fn handle_trigger_gc(data_root: &std::path::Path, repo_id: &str) -> String {
    if repo_id.is_empty() {
        return http_json_response(400, &serde_json::json!({"error": "repo_id required"}));
    }
    let chunks_dir = data_root.join("repositories").join(repo_id).join("chunks");
    let mut chunks_scanned = 0u64;
    let chunks_deleted = 0u64;
    let bytes_freed = 0u64;
    if let Ok(entries) = std::fs::read_dir(&chunks_dir) {
        for entry in entries.flatten() {
            if entry.path().is_dir() {
                if let Ok(sub_entries) = std::fs::read_dir(entry.path()) {
                    for chunk_entry in sub_entries.flatten() {
                        let chunk_path = chunk_entry.path();
                        if chunk_path.extension().and_then(|e| e.to_str()) == Some("chunk") {
                            chunks_scanned += 1;
                        }
                    }
                }
            }
        }
    }
    http_json_response(200, &serde_json::json!({
        "repo_id": repo_id,
        "chunks_scanned": chunks_scanned,
        "chunks_deleted": chunks_deleted,
        "bytes_freed": bytes_freed,
        "duration_ms": 0,
    }))
}

fn http_json_response(status: u16, body: &serde_json::Value) -> String {
    let body_str = serde_json::to_string(body).unwrap_or_else(|_| "{}".to_string());
    let status_text = match status {
        200 => "OK",
        400 => "Bad Request",
        404 => "Not Found",
        500 => "Internal Server Error",
        _ => "Unknown",
    };
    format!(
        "HTTP/1.1 {} {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
        status,
        status_text,
        body_str.len(),
        body_str
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use badou_hbop_server::auth::{JwtClaims, generate_jwt, METADATA_AUTH, METADATA_VERSION};
    use badou_proto::ba_dou_storage_client::BaDouStorageClient;
    use badou_proto::{RepositoryCreateRequest, RepositoryConfig, RepositoryListRequest};
    use chrono::Utc;
    use std::str::FromStr;
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

    fn make_auth_metadata(token: &str) -> tonic::metadata::MetadataMap {
        let mut metadata = tonic::metadata::MetadataMap::new();
        metadata.insert(
            METADATA_AUTH,
            MetadataValue::from_str(&format!("Bearer {}", token)).unwrap(),
        );
        metadata.insert(
            METADATA_VERSION,
            MetadataValue::from_str("1").unwrap(),
        );
        metadata
    }

    fn find_free_port() -> std::net::SocketAddr {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        drop(listener);
        addr
    }

    fn make_test_config(bind_addr: std::net::SocketAddr, metrics_addr: std::net::SocketAddr) -> (tempfile::TempDir, config::ServerConfig) {
        let tmp = tempfile::tempdir().unwrap();
        let data_root = tmp.path().to_path_buf();
        let cfg = config::ServerConfig {
            data_root,
            bind_addr: bind_addr.to_string(),
            metrics_addr: metrics_addr.to_string(),
            management_addr: None,
            jwt_secret: "test-secret".to_string(),
            tls: None,
            cluster: config::ClusterConfig::Single,
        };
        (tmp, cfg)
    }

    #[tokio::test]
    async fn server_starts_and_serves_grpc() {
        let bind_addr = find_free_port();
        let metrics_addr = find_free_port();
        let (_tmp, cfg) = make_test_config(bind_addr, metrics_addr);
        std::mem::forget(_tmp);

        let server_task = tokio::spawn(async move {
            let _ = run_server(cfg).await;
        });

        tokio::time::sleep(std::time::Duration::from_millis(200)).await;

        let channel = tonic::transport::Channel::from_shared(format!("http://{}", bind_addr))
            .unwrap()
            .connect()
            .await
            .unwrap();
        let mut client = BaDouStorageClient::new(channel);

        let token = make_jwt_token(b"test-secret", "admin");
        let metadata = make_auth_metadata(&token);

        let mut req = tonic::Request::new(RepositoryListRequest {});
        *req.metadata_mut() = metadata.clone();
        let resp = client.repository_list(req).await.unwrap();
        assert!(resp.into_inner().repos.is_empty());

        let mut create_req = tonic::Request::new(RepositoryCreateRequest {
            config: Some(RepositoryConfig {
                name: "test".to_string(),
                immutable: None,
                immutable_until: None,
                options: std::collections::HashMap::new(),
            }),
        });
        *create_req.metadata_mut() = metadata;
        let create_resp = client.repository_create(create_req).await.unwrap();
        assert_eq!(create_resp.into_inner().repo.unwrap().name, "test");

        server_task.abort();
    }

    #[tokio::test]
    async fn server_metrics_endpoint_works() {
        let bind_addr = find_free_port();
        let metrics_addr = find_free_port();
        let (_tmp, cfg) = make_test_config(bind_addr, metrics_addr);
        std::mem::forget(_tmp);

        let server_task = tokio::spawn(async move {
            let _ = run_server(cfg).await;
        });

        tokio::time::sleep(std::time::Duration::from_millis(200)).await;

        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let mut stream = tokio::net::TcpStream::connect(metrics_addr).await.unwrap();
        stream
            .write_all(b"GET /metrics HTTP/1.1\r\nHost: localhost\r\n\r\n")
            .await
            .unwrap();
        stream.flush().await.unwrap();

        let mut buf = vec![0u8; 8192];
        let n = stream.read(&mut buf).await.unwrap();
        let response = String::from_utf8_lossy(&buf[..n]);
        assert!(response.contains("HTTP/1.1 200 OK"));
        assert!(response.contains("badou_journal_entries"));

        server_task.abort();
    }

    #[test]
    fn config_load_and_validate() {
        let tmp = tempfile::tempdir().unwrap();
        let config = config::ServerConfig::default_for(tmp.path().to_path_buf());
        let config_path = tmp.path().join("test-config.json");
        std::fs::write(&config_path, config.to_json().unwrap()).unwrap();

        let loaded = config::ServerConfig::load(&config_path).unwrap();
        assert_eq!(loaded.bind_addr, config.bind_addr);
        assert_eq!(loaded.jwt_secret, config.jwt_secret);
    }
}
