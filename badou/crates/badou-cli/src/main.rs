use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use anyhow::{Result, bail};
use tonic::transport::Channel;
use badou_proto::ba_dou_storage_client::BaDouStorageClient;

const METADATA_AUTH: &str = "authorization";
const METADATA_VERSION: &str = "x-hbop-version";

struct ParsedArgs {
    positional: Vec<String>,
    options: HashMap<String, String>,
    flags: HashSet<String>,
}

fn parse_args(args: &[String]) -> ParsedArgs {
    let mut positional = Vec::new();
    let mut options = HashMap::new();
    let mut flags = HashSet::new();

    let mut i = 0;
    while i < args.len() {
        if args[i].starts_with("--") {
            let key = args[i][2..].to_string();
            if i + 1 < args.len() && !args[i + 1].starts_with("--") {
                options.insert(key, args[i + 1].clone());
                i += 2;
            } else {
                flags.insert(key);
                i += 1;
            }
        } else {
            positional.push(args[i].clone());
            i += 1;
        }
    }

    ParsedArgs { positional, options, flags }
}

fn make_auth_metadata(token: &str) -> tonic::metadata::MetadataMap {
    use std::str::FromStr;
    let mut metadata = tonic::metadata::MetadataMap::new();
    metadata.insert(
        METADATA_AUTH,
        tonic::metadata::MetadataValue::from_str(&format!("Bearer {}", token)).unwrap(),
    );
    metadata.insert(
        METADATA_VERSION,
        tonic::metadata::MetadataValue::from_str("1").unwrap(),
    );
    metadata
}

fn make_request<T>(token: &str, msg: T) -> tonic::Request<T> {
    tonic::Request::from_parts(make_auth_metadata(token), tonic::Extensions::default(), msg)
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let args: Vec<String> = std::env::args().skip(1).collect();

    if args.is_empty() {
        print_usage();
        return Ok(());
    }

    let command = args[0].as_str();
    let rest = &args[1..];

    match command {
        "init" => cmd_init(rest),
        "verify" => cmd_verify(rest).await,
        "gc" => cmd_gc(rest).await,
        "health" => cmd_health(rest).await,
        "cluster" => cmd_cluster(rest),
        "recovery" => cmd_recovery(rest).await,
        "help" | "--help" | "-h" => {
            print_usage();
            Ok(())
        }
        _ => {
            eprintln!("未知命令: {}", command);
            print_usage();
            std::process::exit(1);
        }
    }
}

fn print_usage() {
    eprintln!("badou-cli — 八斗存储桶运维工具");
    eprintln!();
    eprintln!("用法: badou-cli <命令> [选项]");
    eprintln!();
    eprintln!("命令:");
    eprintln!("  init <data-dir> [--cluster-size <n>]");
    eprintln!("    初始化八斗节点");
    eprintln!();
    eprintln!("  verify <repo-id> --endpoint <url> --token <jwt>");
    eprintln!("    [--level Repository|Version|Chunk] [--mode Quick|Full|Deep]");
    eprintln!("    [--version <id>] [--chunk <hash>]");
    eprintln!("    触发校验");
    eprintln!();
    eprintln!("  gc <repo-id> --endpoint <url> [--trigger] [--report]");
    eprintln!("    触发/查询 GC (通过 Prometheus 指标)");
    eprintln!();
    eprintln!("  health --endpoint <url> [--node <id>]");
    eprintln!("    健康检查 (通过 Prometheus 指标)");
    eprintln!();
    eprintln!("  cluster join --peer <addr> --endpoint <url>");
    eprintln!("  cluster leave --node <id> --endpoint <url>");
    eprintln!("    集群操作");
    eprintln!();
    eprintln!("  recovery <repo-id> --endpoint <url> --token <jwt>");
    eprintln!("    --version <id> --target <path> [--file <path>]");
    eprintln!("    流式恢复");
    eprintln!();
    eprintln!("  help");
    eprintln!("    显示帮助");
}

fn cmd_init(args: &[String]) -> Result<()> {
    let parsed = parse_args(args);
    if parsed.positional.is_empty() {
        bail!("用法: badou-cli init <data-dir> [--cluster-size <n>]");
    }
    let data_dir = PathBuf::from(&parsed.positional[0]);
    let cluster_size: usize = parsed
        .options
        .get("cluster-size")
        .map(|s| s.parse().unwrap_or(1))
        .unwrap_or(1);

    std::fs::create_dir_all(&data_dir)?;

    let config_json = serde_json::json!({
        "data_root": data_dir.to_string_lossy(),
        "bind_addr": "0.0.0.0:9090",
        "metrics_addr": "0.0.0.0:9091",
        "jwt_secret": "change-me-please",
        "tls": null,
        "cluster": {
            "mode": "single"
        }
    });

    let config_path = data_dir.join("badou-server.json");
    std::fs::write(&config_path, serde_json::to_string_pretty(&config_json)?)?;

    println!("八斗节点已初始化: {}", data_dir.display());
    println!("配置文件: {}", config_path.display());
    println!("集群大小: {}", cluster_size);
    println!();
    println!("请编辑配置文件修改 jwt_secret 和监听地址，然后运行:");
    println!("  badou-server {}", config_path.display());

    Ok(())
}

async fn cmd_verify(args: &[String]) -> Result<()> {
    let parsed = parse_args(args);
    if parsed.positional.is_empty() {
        bail!("用法: badou-cli verify <repo-id> --endpoint <url> --token <jwt>");
    }

    let repo_id = &parsed.positional[0];
    let endpoint = parsed
        .options
        .get("endpoint")
        .cloned()
        .unwrap_or_else(|| "http://localhost:9090".to_string());
    let token = parsed
        .options
        .get("token")
        .cloned()
        .unwrap_or_default();
    let level = parsed
        .options
        .get("level")
        .cloned()
        .unwrap_or_else(|| "Repository".to_string());
    let mode = parsed
        .options
        .get("mode")
        .cloned()
        .unwrap_or_else(|| "Quick".to_string());
    let version_id = parsed.options.get("version").cloned();

    if token.is_empty() {
        bail!("需要 --token <jwt> 进行鉴权");
    }

    let channel = Channel::from_shared(endpoint)?.connect().await?;
    let mut client = BaDouStorageClient::new(channel);

    let deep = matches!(mode.as_str(), "Full" | "full" | "Deep" | "deep");

    match level.as_str() {
        "Repository" | "repository" => {
            let req = make_request(
                &token,
                badou_proto::VerifyRepositoryRequest {
                    repo_id: repo_id.clone(),
                    deep,
                },
            );
            let mut stream = client.verify_repository(req).await?.into_inner();
            use tokio_stream::StreamExt;
            while let Some(report) = stream.next().await {
                match report {
                    Ok(r) => {
                        println!(
                            "校验报告: target={} passed={} checked={} failed={}",
                            r.target_id, r.passed, r.total_checked, r.total_failed
                        );
                        for item in &r.failed_items {
                            println!("  失败项: {}", item);
                        }
                    }
                    Err(e) => eprintln!("校验错误: {}", e),
                }
            }
        }
        "Version" | "version" => {
            let vid = version_id
                .clone()
                .ok_or_else(|| anyhow::anyhow!("需要 --version <id>"))?;
            let req = make_request(
                &token,
                badou_proto::VerifyVersionRequest {
                    repo_id: repo_id.clone(),
                    version_id: vid,
                    deep,
                },
            );
            let mut stream = client.verify_version(req).await?.into_inner();
            use tokio_stream::StreamExt;
            while let Some(report) = stream.next().await {
                match report {
                    Ok(r) => {
                        println!(
                            "校验报告: target={} passed={} checked={} failed={}",
                            r.target_id, r.passed, r.total_checked, r.total_failed
                        );
                        for item in &r.failed_items {
                            println!("  失败项: {}", item);
                        }
                    }
                    Err(e) => eprintln!("校验错误: {}", e),
                }
            }
        }
        "Chunk" | "chunk" => {
            let chunk_hash = parsed
                .options
                .get("chunk")
                .cloned()
                .ok_or_else(|| anyhow::anyhow!("需要 --chunk <hash>"))?;
            let req = make_request(
                &token,
                badou_proto::VerifyChunkRequest {
                    repo_id: repo_id.clone(),
                    chunk_hash,
                },
            );
            let resp = client.verify_chunk(req).await?.into_inner();
            println!(
                "Chunk 校验结果: passed={} expected={} actual={}",
                resp.passed, resp.expected_hash, resp.actual_hash
            );
        }
        _ => bail!("未知校验级别: {} (可选: Repository, Version, Chunk)", level),
    }

    Ok(())
}

async fn cmd_gc(args: &[String]) -> Result<()> {
    let parsed = parse_args(args);
    let endpoint = parsed
        .options
        .get("endpoint")
        .cloned()
        .unwrap_or_else(|| "localhost:9091".to_string());

    if parsed.flags.contains("trigger") {
        println!("GC 触发: 当前版本通过 Prometheus 指标端点不支持远程触发 GC。");
        println!("GC 由服务器内部调度器自动运行。");
        return Ok(());
    }

    let body = fetch_metrics(&endpoint).await?;

    if parsed.flags.contains("report") || !parsed.flags.contains("trigger") {
        println!("GC 指标 (来自 Prometheus 端点 {}):", endpoint);
        for line in body.lines() {
            if line.contains("badou_gc") || line.contains("badou_commit") {
                println!("  {}", line);
            }
        }
    }

    Ok(())
}

async fn cmd_health(args: &[String]) -> Result<()> {
    let parsed = parse_args(args);
    let endpoint = parsed
        .options
        .get("endpoint")
        .cloned()
        .unwrap_or_else(|| "localhost:9091".to_string());

    let body = fetch_metrics(&endpoint).await?;

    println!("八斗存储桶健康状态 (Prometheus 端点 {}):", endpoint);
    println!();

    let mut has_issues = false;
    for line in body.lines() {
        if line.starts_with('#') || line.is_empty() {
            continue;
        }
        println!("  {}", line);
        if line.contains("corrupted") && line.ends_with(" 1") {
            has_issues = true;
        }
    }

    if has_issues {
        println!();
        println!("警告: 检测到异常指标!");
    }

    Ok(())
}

fn cmd_cluster(args: &[String]) -> Result<()> {
    if args.is_empty() {
        bail!("用法: badou-cli cluster join --peer <addr> | cluster leave --node <id>");
    }

    let subcommand = args[0].as_str();
    let rest = &args[1..];
    let parsed = parse_args(rest);

    match subcommand {
        "join" => {
            let peer = parsed
                .options
                .get("peer")
                .cloned()
                .ok_or_else(|| anyhow::anyhow!("需要 --peer <addr>"))?;
            println!("集群加入请求: peer={}", peer);
            println!("注意: 集群管理 API 尚未实现。");
            println!("请直接在目标节点上运行 badou-server 并配置 Raft 模式。");
        }
        "leave" => {
            let node_id = parsed
                .options
                .get("node")
                .cloned()
                .ok_or_else(|| anyhow::anyhow!("需要 --node <id>"))?;
            println!("集群离开请求: node={}", node_id);
            println!("注意: 集群管理 API 尚未实现。");
            println!("请停止目标节点上的 badou-server 进程。");
        }
        _ => bail!("未知子命令: {} (可选: join, leave)", subcommand),
    }

    Ok(())
}

async fn cmd_recovery(args: &[String]) -> Result<()> {
    let parsed = parse_args(args);
    if parsed.positional.is_empty() {
        bail!("用法: badou-cli recovery <repo-id> --endpoint <url> --token <jwt> --version <id> --target <path>");
    }

    let repo_id = &parsed.positional[0];
    let endpoint = parsed
        .options
        .get("endpoint")
        .cloned()
        .unwrap_or_else(|| "http://localhost:9090".to_string());
    let token = parsed
        .options
        .get("token")
        .cloned()
        .unwrap_or_default();
    let version_id = parsed
        .options
        .get("version")
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("需要 --version <id>"))?;
    let target = parsed
        .options
        .get("target")
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("需要 --target <path>"))?;
    let file_path = parsed.options.get("file").cloned();

    if token.is_empty() {
        bail!("需要 --token <jwt> 进行鉴权");
    }

    let channel = Channel::from_shared(endpoint)?.connect().await?;
    let mut client = BaDouStorageClient::new(channel);

    let req = make_request(
        &token,
        badou_proto::RecoveryOpenRequest {
            repo_id: repo_id.clone(),
            version_id,
            file_path,
        },
    );

    let mut stream = client.recovery_open(req).await?.into_inner();

    use tokio::io::AsyncWriteExt;
    let mut file = tokio::fs::File::create(&target).await?;
    let mut total_bytes: u64 = 0;
    let mut total_chunks: u64 = 0;

    use tokio_stream::StreamExt;
    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        file.write_all(&chunk.data).await?;
        total_bytes += chunk.size;
        total_chunks += 1;
    }
    file.flush().await?;

    println!("恢复完成: {} 个 Chunk, {} 字节 -> {}", total_chunks, total_bytes, target);

    Ok(())
}

async fn fetch_metrics(endpoint: &str) -> Result<String> {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    let mut stream = tokio::net::TcpStream::connect(endpoint).await?;
    stream
        .write_all(b"GET /metrics HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
        .await?;
    stream.flush().await?;

    let mut buf = Vec::new();
    stream.read_to_end(&mut buf).await?;
    let response = String::from_utf8_lossy(&buf);

    let body = response
        .split("\r\n\r\n")
        .nth(1)
        .unwrap_or("")
        .to_string();

    Ok(body)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_args_basic() {
        let args = vec![
            "foo".to_string(),
            "--bar".to_string(),
            "baz".to_string(),
        ];
        let parsed = parse_args(&args);
        assert_eq!(parsed.positional, vec!["foo".to_string()]);
        assert_eq!(parsed.options.get("bar"), Some(&"baz".to_string()));
    }

    #[test]
    fn parse_args_flags() {
        let args = vec!["foo".to_string(), "--flag".to_string()];
        let parsed = parse_args(&args);
        assert_eq!(parsed.positional, vec!["foo".to_string()]);
        assert!(parsed.flags.contains("flag"));
    }

    #[test]
    fn parse_args_multiple_options() {
        let args = vec![
            "--endpoint".to_string(),
            "http://localhost:9090".to_string(),
            "--token".to_string(),
            "abc123".to_string(),
        ];
        let parsed = parse_args(&args);
        assert_eq!(
            parsed.options.get("endpoint"),
            Some(&"http://localhost:9090".to_string())
        );
        assert_eq!(parsed.options.get("token"), Some(&"abc123".to_string()));
    }

    #[test]
    fn parse_args_mixed() {
        let args = vec![
            "repo-1".to_string(),
            "--deep".to_string(),
            "--level".to_string(),
            "Repository".to_string(),
            "extra".to_string(),
        ];
        let parsed = parse_args(&args);
        assert_eq!(parsed.positional, vec!["repo-1".to_string(), "extra".to_string()]);
        assert!(parsed.flags.contains("deep"));
        assert_eq!(parsed.options.get("level"), Some(&"Repository".to_string()));
    }
}
