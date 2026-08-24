use std::path::PathBuf;
use std::sync::Arc;

use hbx_chunker::FixedChunker;
use hbx_compress::ZstdCompressor;
use hbx_core::domain::backup::{BackupDestination, BackupJob, BackupSource, JobStatus};
use hbx_core::domain::common::{
    CompressionAlgorithm, CompressionProfile, EncryptionProfileRef, JobId, RepositoryId,
    RetentionPolicyRef, ScheduleRef,
};
use hbx_core::domain::repository::BackendType;
use hbx_core::pipeline::{ChunkStrategy, IBackupRepository, IIntegrityVerifier};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        print_usage();
        std::process::exit(0);
    }
    let command = &args[0];
    let rest = &args[1..];
    if let Err(e) = run_command(command, rest) {
        eprintln!("Error: {e}");
        std::process::exit(1);
    }
}

fn run_command(command: &str, args: &[String]) -> Result<(), String> {
    match command {
        "backup" => cmd_backup(args),
        "restore" => cmd_restore(args),
        "verify" => cmd_verify(args),
        "consistency" => cmd_consistency(args),
        "init-repo" => cmd_init_repo(args),
        "list-versions" => cmd_list_versions(args),
        "register" => cmd_register(args),
        "heartbeat" => cmd_heartbeat(args),
        "build" => cmd_build(args),
        "help" | "--help" | "-h" => {
            print_usage();
            Ok(())
        }
        _ => Err(format!("unknown command: {command}")),
    }
}

fn print_usage() {
    eprintln!("HyperBackup X (HBX) - CLI v0.1.0");
    eprintln!();
    eprintln!("USAGE:");
    eprintln!("    xtask <COMMAND> [OPTIONS]");
    eprintln!();
    eprintln!("COMMANDS:");
    eprintln!("    init-repo <path>                    Initialize a new backup repository");
    eprintln!("    backup <source> <repo>              Run a full backup");
    eprintln!("    restore <repo> <version-id> <dest>  Restore a version");
    eprintln!("    verify <repo> <version-id>          Verify a backup version");
    eprintln!("    consistency <repo>                  Run consistency check");
    eprintln!("    list-versions <repo>                List all backup versions");
    eprintln!("    register <server-url> <hostname>    Register agent with control plane");
    eprintln!("    heartbeat <server-url> <agent-id>   Send heartbeat to control plane");
    eprintln!("    build <tier>                        Build agent binary (legacy/standard/modern)");
    eprintln!("    help                                Show this help message");
    eprintln!();
    eprintln!("EXAMPLES:");
    eprintln!("    xtask init-repo /backups/my-repo");
    eprintln!("    xtask backup /home/user/docs /backups/my-repo");
    eprintln!("    xtask restore /backups/my-repo <version-id> /home/user/restored");
    eprintln!("    xtask verify /backups/my-repo <version-id>");
    eprintln!("    xtask consistency /backups/my-repo");
}

fn cmd_init_repo(args: &[String]) -> Result<(), String> {
    if args.is_empty() {
        return Err("usage: xtask init-repo <path>".into());
    }
    let path = &args[0];
    hbx_repo::RepositoryInitializer::new(path)
        .init(RepositoryId(uuid::Uuid::new_v4()), BackendType::Local)
        .map_err(|e| format!("failed to init repo: {e}"))?;
    println!("Repository initialized at {}", path);
    Ok(())
}

fn cmd_backup(args: &[String]) -> Result<(), String> {
    if args.len() < 2 {
        return Err("usage: xtask backup <source> <repo>".into());
    }
    let source = &args[0];
    let repo_path = &args[1];

    let repo = hbx_repo::LocalRepository::open(repo_path)
        .map_err(|e| format!("failed to open repo: {e}"))?;

    let engine = hbx_engine::BackupEngine::builder()
        .scanner(hbx_scanner::LocalScanner::new())
        .chunker(FixedChunker::new())
        .dedup(hbx_dedup::LocalDedupIndex::new())
        .compressor(ZstdCompressor::default())
        .encryption(hbx_engine::NoOpEncryptionProvider)
        .repo(repo)
        .memory_limit(512 * 1024 * 1024)
        .chunk_strategy(ChunkStrategy::Fixed { chunk_size: 4096 })
        .build()
        .map_err(|e| format!("failed to build engine: {e}"))?;

    let job = BackupJob {
        job_id: JobId(uuid::Uuid::new_v4()),
        name: "cli-backup".to_string(),
        source: BackupSource {
            paths: vec![PathBuf::from(source)],
            include_rules: vec![],
            exclude_rules: vec![],
        },
        destination: BackupDestination {
            repository_id: RepositoryId(uuid::Uuid::new_v4()),
            logical_path: "/".to_string(),
        },
        schedule: ScheduleRef(uuid::Uuid::new_v4()),
        retention_policy: RetentionPolicyRef(uuid::Uuid::new_v4()),
        encryption_profile: EncryptionProfileRef(uuid::Uuid::new_v4()),
        compression_profile: CompressionProfile {
            algorithm: CompressionAlgorithm::Zstd,
            level: 3,
        },
        status: JobStatus::Active,
        created_at: chrono::Utc::now(),
    };

    let tracker = engine.execution_tracker(&job.job_id);

    let runtime = tokio::runtime::Runtime::new().map_err(|e| format!("runtime error: {e}"))?;
    let result = runtime.block_on(engine.run_backup(&job, &tracker))
        .map_err(|e| format!("backup failed: {e}"))?;

    println!("Backup completed:");
    println!("  Version ID:    {:?}", result.version_id);
    println!("  Files:         {}", result.file_count);
    println!("  Chunks:        {}", result.chunk_count);
    println!("  Data processed: {} bytes", result.data_processed);
    println!("  Data stored:   {} bytes", result.data_stored);
    println!("  Dedup ratio:   {:.2}%", result.dedup_ratio * 100.0);
    println!("  Duration:      {:?}", result.duration);
    Ok(())
}

fn cmd_restore(args: &[String]) -> Result<(), String> {
    if args.len() < 3 {
        return Err("usage: xtask restore <repo> <version-id> <dest>".into());
    }
    let repo_path = &args[0];
    let version_id_str = &args[1];
    let dest = &args[2];

    let repo = hbx_repo::LocalRepository::open(repo_path)
        .map_err(|e| format!("failed to open repo: {e}"))?;

    let version_uuid = uuid::Uuid::parse_str(version_id_str)
        .map_err(|e| format!("invalid version ID: {e}"))?;
    let version_id = hbx_core::domain::common::VersionId(version_uuid);

    let restore_engine = hbx_restore::RestoreEngine::new(
        Arc::new(ZstdCompressor::default()),
        Arc::new(hbx_engine::NoOpEncryptionProvider),
    );

    let job = hbx_core::domain::restore::RestoreJob {
        restore_id: hbx_core::domain::common::RestoreId(uuid::Uuid::new_v4()),
        source_version_id: version_id,
        file_selection: hbx_core::domain::restore::FileSelection::All,
        restore_mode: hbx_core::domain::restore::RestoreMode::Overwrite,
        target_location: PathBuf::from(dest),
        status: hbx_core::domain::restore::RestoreStatus::Pending,
        started_at: None,
        completed_at: None,
        failed_files: vec![],
    };

    let tracker = hbx_restore::RestoreTracker::new();

    let runtime = tokio::runtime::Runtime::new().map_err(|e| format!("runtime error: {e}"))?;
    let result = runtime.block_on(restore_engine.run_restore(&job, &repo, &tracker))
        .map_err(|e| format!("restore failed: {e}"))?;

    println!("Restore completed:");
    println!("  Files restored: {}", result.files_restored);
    println!("  Files failed:    {}", result.files_failed);
    println!("  All verified:    {}", result.all_verified);
    Ok(())
}

fn cmd_verify(args: &[String]) -> Result<(), String> {
    if args.len() < 2 {
        return Err("usage: xtask verify <repo> <version-id>".into());
    }
    let repo_path = &args[0];
    let version_id_str = &args[1];

    let repo = hbx_repo::LocalRepository::open(repo_path)
        .map_err(|e| format!("failed to open repo: {e}"))?;

    let version_uuid = uuid::Uuid::parse_str(version_id_str)
        .map_err(|e| format!("invalid version ID: {e}"))?;
    let version_id = hbx_core::domain::common::VersionId(version_uuid);

    let verifier = hbx_verify::IntegrityVerifier::new(
        Arc::new(ZstdCompressor::default()),
        Arc::new(hbx_engine::NoOpEncryptionProvider),
    );

    let report = verifier
        .verify(&version_id, hbx_core::domain::verify::VerifyMode::Full, &repo)
        .map_err(|e| format!("verify failed: {e}"))?;

    println!("Verification report:");
    println!("  Mode:          {:?}", report.mode);
    println!("  Total checked: {}", report.total_checked);
    println!("  Passed:        {}", report.passed);
    println!("  Failed:        {}", report.failed);
    if !report.failures.is_empty() {
        println!("  Failures:");
        for f in &report.failures {
            println!("    - {:?}: {}", f.item_type, f.identifier);
        }
    }
    Ok(())
}

fn cmd_consistency(args: &[String]) -> Result<(), String> {
    if args.is_empty() {
        return Err("usage: xtask consistency <repo>".into());
    }
    let repo_path = &args[0];

    let repo = hbx_repo::LocalRepository::open(repo_path)
        .map_err(|e| format!("failed to open repo: {e}"))?;

    let checker = hbx_verify::ConsistencyChecker::new();
    let report = checker
        .check(&repo, &[])
        .map_err(|e| format!("consistency check failed: {e}"))?;

    println!("Consistency report:");
    println!("  Checked versions:  {}", report.checked_versions.len());
    println!("  Healthy versions:  {}", report.healthy_versions.len());
    println!("  Incomplete versions: {}", report.incomplete_versions.len());
    println!("  Orphan chunks:     {}", report.orphan_chunks.len());
    println!("  Missing chunks:    {}", report.missing_chunks.len());
    println!("  Consistent:        {}", report.is_consistent());
    Ok(())
}

fn cmd_list_versions(args: &[String]) -> Result<(), String> {
    if args.is_empty() {
        return Err("usage: xtask list-versions <repo>".into());
    }
    let repo_path = &args[0];

    let repo = hbx_repo::LocalRepository::open(repo_path)
        .map_err(|e| format!("failed to open repo: {e}"))?;

    let versions = repo
        .list_versions()
        .map_err(|e| format!("failed to list versions: {e}"))?;

    if versions.is_empty() {
        println!("No versions found.");
        return Ok(());
    }

    println!("{:<38} {:<8} {:<20} {:<10} {:>12}", "VERSION ID", "NUMBER", "TIMESTAMP", "TYPE", "SIZE");
    for v in &versions {
        println!(
            "{:<38} {:<8} {:<20} {:<10} {:>12}",
            v.version_id,
            v.version_number,
            v.timestamp.format("%Y-%m-%d %H:%M:%S"),
            format!("{:?}", v.backup_type),
            v.stored_size
        );
    }
    Ok(())
}

fn cmd_register(args: &[String]) -> Result<(), String> {
    if args.len() < 2 {
        return Err("usage: xtask register <server-url> <hostname>".into());
    }
    let server_url = &args[0];
    let hostname = &args[1];

    let mut client = hbx_client::HbxClient::new(server_url);
    let req = hbx_proto::RegisterDeviceRequest {
        hostname: hostname.clone(),
        os_version: std::env::consts::OS.to_string(),
        agent_version: env!("CARGO_PKG_VERSION").to_string(),
        tier: hbx_proto::HardwareTier::Standard,
        supported_protocols: vec!["v1".to_string()],
        device_fingerprint: uuid::Uuid::new_v4().to_string(),
    };

    let resp = client
        .register_device(&req)
        .map_err(|e| format!("register failed: {e}"))?;

    println!("Agent registered:");
    println!("  Agent ID:     {}", resp.agent_id);
    println!("  Group:        {}", resp.assigned_group);
    println!("  Heartbeat:    {}s", resp.heartbeat_interval_secs);
    Ok(())
}

fn cmd_heartbeat(args: &[String]) -> Result<(), String> {
    if args.len() < 2 {
        return Err("usage: xtask heartbeat <server-url> <agent-id>".into());
    }
    let server_url = &args[0];
    let agent_id = &args[1];

    let mut client = hbx_client::HbxClient::new(server_url);
    client.set_agent_id(agent_id);

    let resp = client
        .heartbeat(
            hbx_proto::AgentStatus::Idle,
            hbx_proto::ResourceInfo {
                total_memory_bytes: 0,
                available_memory_bytes: 0,
                cpu_cores: num_cpus(),
                disk_free_bytes: 0,
                cpu_usage_percent: 0.0,
                disk_io_mbps: 0.0,
                net_io_mbps: 0.0,
            },
        )
        .map_err(|e| format!("heartbeat failed: {e}"))?;

    println!("Heartbeat response:");
    println!("  Server time:        {}", resp.server_time);
    println!("  Pending commands:   {}", resp.pending_commands.len());
    println!("  Config updated:     {}", resp.config_updated);
    Ok(())
}

fn num_cpus() -> u32 {
    std::thread::available_parallelism()
        .map(|n| n.get() as u32)
        .unwrap_or(1)
}

fn cmd_build(args: &[String]) -> Result<(), String> {
    let tier = args.first().map(|s| s.as_str()).unwrap_or("standard");
    match tier {
        "legacy" => {
            println!("Building legacy tier (Win7 x86_64-pc-windows-gnu)...");
            println!("  Target: x86_64-pc-windows-gnu");
            println!("  CRT:    static");
            println!("  Chunk:  1MB");
        }
        "standard" => {
            println!("Building standard tier (Win10 x86_64-pc-windows-msvc)...");
            println!("  Target: x86_64-pc-windows-msvc");
            println!("  CRT:    dynamic");
            println!("  Chunk:  4MB");
        }
        "modern" => {
            println!("Building modern tier (Win11 x86_64-pc-windows-msvc)...");
            println!("  Target: x86_64-pc-windows-msvc");
            println!("  CRT:    dynamic");
            println!("  Chunk:  8MB");
            println!("  VSS:    enabled");
        }
        _ => return Err(format!("unknown tier: {tier} (use legacy/standard/modern)")),
    }
    println!("Build configuration ready. Run 'cargo build --release' to compile.");
    Ok(())
}
