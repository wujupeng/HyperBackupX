use anyhow::{Context, Result};
use std::env;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

mod control_client;
mod task_executor;

use control_client::{ControlClient, RegisterRequest};
use task_executor::{TaskExecutor, TaskSpec};

static RUNNING: AtomicBool = AtomicBool::new(true);

fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let cp_url = env::var("HBX_AGENT_CP_URL").unwrap_or_else(|_| "http://localhost:8080".to_string());
    let badou_grpc = env::var("HBX_AGENT_BADOU_GRPC").unwrap_or_else(|_| "http://192.168.2.3:9090".to_string());

    tracing::info!("HBX Agent starting");
    tracing::info!("Control Plane URL: {}", cp_url);
    tracing::info!("Badou gRPC endpoint: {}", badou_grpc);

    let hostname = hostname().unwrap_or_else(|_| "unknown".to_string());
    let os_version = os_version();
    let agent_version = env!("CARGO_PKG_VERSION").to_string();

    tracing::info!("hostname={}, os={}, agent_version={}", hostname, os_version, agent_version);

    let mut client = ControlClient::new(&cp_url);

    let register_req = RegisterRequest {
        hostname: hostname.clone(),
        os_version: os_version.clone(),
        agent_version: agent_version.clone(),
        tier: "standard".to_string(),
        supported_protocols: vec!["grpc".to_string(), "rest".to_string()],
        device_fingerprint: format!("{}-{}", hostname, os_version),
    };

    tracing::info!("registering with Control Plane...");
    let register_resp = client
        .register_with_retry(&register_req, 10)
        .context("register with Control Plane")?;

    tracing::info!(
        "registered: agent_id={}, group={}, heartbeat={}s",
        register_resp.agent_id,
        register_resp.assigned_group,
        register_resp.heartbeat_interval_secs
    );

    let agent_id = client.agent_id().unwrap().to_string();
    let executor = TaskExecutor::new(&agent_id);
    let heartbeat_interval = client.heartbeat_interval();

    setup_signal_handler();

    tracing::info!("entering main loop (heartbeat={}s)", heartbeat_interval);

    while RUNNING.load(Ordering::SeqCst) {
        let resources = collect_resources();
        let status = "idle".to_string();

        match client.heartbeat(&status, resources) {
            Ok(resp) => {
                if !resp.pending_commands.is_empty() {
                    for cmd in &resp.pending_commands {
                        tracing::info!("received command: {}", cmd);
                        match serde_json::from_str::<TaskSpec>(cmd) {
                            Ok(spec) => {
                                if let Err(e) = executor.execute(&spec, &client) {
                                    tracing::error!("task execution failed: {}", e);
                                }
                            }
                            Err(e) => {
                                tracing::warn!("failed to parse command as TaskSpec: {}", e);
                            }
                        }
                    }
                }
            }
            Err(e) => {
                tracing::warn!("heartbeat failed: {}", e);
                std::thread::sleep(Duration::from_secs(5));
                continue;
            }
        }

        std::thread::sleep(Duration::from_secs(heartbeat_interval as u64));
    }

    tracing::info!("HBX Agent shutting down");
    Ok(())
}

fn hostname() -> Result<String> {
    let output = std::process::Command::new("hostname").output().context("run hostname")?;
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn os_version() -> String {
    #[cfg(target_os = "windows")]
    {
        "windows".to_string()
    }
    #[cfg(target_os = "linux")]
    {
        let output = std::process::Command::new("uname")
            .arg("-r")
            .output()
            .ok();
        if let Some(o) = output {
            if o.status.success() {
                return format!("linux {}", String::from_utf8_lossy(&o.stdout).trim());
            }
        }
        "linux".to_string()
    }
    #[cfg(not(any(target_os = "windows", target_os = "linux")))]
    {
        "unknown".to_string()
    }
}

fn collect_resources() -> serde_json::Value {
    use sysinfo::System;
    let mut sys = System::new_all();
    sys.refresh_all();

    serde_json::json!({
        "cpu_usage": sys.global_cpu_usage(),
        "total_memory": sys.total_memory(),
        "used_memory": sys.used_memory(),
        "total_swap": sys.total_swap(),
        "used_swap": sys.used_swap(),
    })
}

#[cfg(unix)]
fn setup_signal_handler() {
    use std::os::raw::c_int;
    extern "C" {
        fn signal(signum: c_int, handler: extern "C" fn(c_int)) -> extern "C" fn(c_int);
    }
    extern "C" fn handle(_: c_int) {
        RUNNING.store(false, Ordering::SeqCst);
    }
    unsafe {
        signal(2, handle);
        signal(15, handle);
    }
}

#[cfg(windows)]
fn setup_signal_handler() {
    use std::os::raw::c_int;
    extern "system" {
        fn SetConsoleCtrlHandler(handlerroutine: extern "system" fn(c_int) -> c_int, add: c_int) -> c_int;
    }
    extern "system" fn handle(_: c_int) -> c_int {
        RUNNING.store(false, Ordering::SeqCst);
        1
    }
    unsafe {
        SetConsoleCtrlHandler(handle, 1);
    }
}
