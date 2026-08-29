use anyhow::{Context, Result};
use std::path::Path;

use crate::args::Args;
use crate::client::ApiClient;

pub fn run(args: &Args, client: &ApiClient) -> Result<()> {
    let subcommand = args
        .positional
        .first()
        .ok_or_else(|| anyhow::anyhow!("missing compat subcommand"))?;

    match subcommand.as_str() {
        "backup" => backup(args, client),
        "restore" => restore(args, client),
        "list" => list(args, client),
        "delete" => delete(args, client),
        "verify" => verify(args, client),
        "import" => import(args, client),
        _ => Err(anyhow::anyhow!(
            "unknown compat subcommand: {}\navailable: backup, restore, list, delete, verify, import",
            subcommand
        )),
    }
}

fn resolve_repo_id(client: &ApiClient, repo_id_or_name: &str) -> Result<String> {
    if repo_id_or_name.parse::<uuid::Uuid>().is_ok() {
        return Ok(repo_id_or_name.to_string());
    }
    let resp = client.get("/api/v1/badou/repositories")?;
    let parsed: serde_json::Value = serde_json::from_str(&resp).context("failed to parse repos response")?;
    let repos = parsed.get("repositories").and_then(|v| v.as_array())
        .ok_or_else(|| anyhow::anyhow!("no repositories field in response"))?;
    for repo in repos {
        let name = repo.get("name").and_then(|v| v.as_str()).unwrap_or("");
        let rid = repo.get("repo_id").and_then(|v| v.as_str()).unwrap_or("");
        if name == repo_id_or_name {
            return Ok(rid.to_string());
        }
    }
    Err(anyhow::anyhow!("repo '{}' not found (neither UUID nor matching name)", repo_id_or_name))
}

fn backup(args: &Args, client: &ApiClient) -> Result<()> {
    let job_id = args
        .positional
        .get(1)
        .ok_or_else(|| anyhow::anyhow!("missing <job-id> argument\nusage: hbx-cli compat backup <job-id> --repo <repo-id>"))?;

    let repo_id = args
        .get("repo")
        .ok_or_else(|| anyhow::anyhow!("missing --repo <repo-id> option"))?;

    println!("Triggering compat backup for job {} on repo {}", job_id, repo_id);

    let body = serde_json::json!({
        "job_id": job_id,
        "repo_id": repo_id,
    })
    .to_string();

    let resp = client.post(&format!("/api/v1/compat/jobs/{}/trigger", job_id), &body);
    match resp {
        Ok(text) => {
            println!("Backup triggered successfully: {}", text);
            Ok(())
        }
        Err(e) => {
            eprintln!("Failed to trigger backup: {}", e);
            Err(e)
        }
    }
}

fn restore(args: &Args, client: &ApiClient) -> Result<()> {
    let version_id = args
        .positional
        .get(1)
        .ok_or_else(|| anyhow::anyhow!("missing <version-id> argument\nusage: hbx-cli compat restore <version-id> --target <dir> [--source <path-prefix>] [--overwrite <skip|overwrite|rename>]"))?;

    let target = args
        .get("target")
        .ok_or_else(|| anyhow::anyhow!("missing --target <dir> option"))?;

    let source = args.get("source");
    let overwrite = args.get_or("overwrite", "skip");

    let selection = args.get_or("selection", "all");
    let mode = args.get_or("mode", overwrite);

    println!(
        "Restoring version {} to {} (source: {}, overwrite: {})",
        version_id,
        target,
        source.unwrap_or("(all)"),
        overwrite
    );

    let mut body = serde_json::json!({
        "source_version_id": version_id,
        "target_path": target,
        "target_location": target,
        "overwrite_policy": overwrite,
        "file_selection": selection,
        "restore_mode": mode,
    });

    if let Some(ref src) = source {
        body["source_path_prefix"] = serde_json::Value::String(src.to_string());
    }

    let body_str = body.to_string();

    let resp = client.post("/api/v1/restores", &body_str);
    match resp {
        Ok(text) => {
            println!("Restore initiated: {}", text);
            Ok(())
        }
        Err(e) => {
            eprintln!("Failed to initiate restore: {}", e);
            Err(e)
        }
    }
}

fn list(args: &Args, client: &ApiClient) -> Result<()> {
    let repo_id_or_name = args
        .positional
        .get(1)
        .ok_or_else(|| anyhow::anyhow!("missing <repo-id> argument\nusage: hbx-cli compat list <repo-id> [--versions] [--files <version>]"))?;

    let repo_id = resolve_repo_id(client, repo_id_or_name)?;

    if args.has("versions") {
        println!("Listing compat versions for repo {} (UUID: {})", repo_id_or_name, repo_id);
        let resp = client.get(&format!("/api/v1/badou/repositories/{}/versions", repo_id))?;
        println!("{}", resp);
    } else if let Some(version) = args.get("files") {
        println!("Listing files for version {}", version);
        let path = format!("/api/v1/badou/repositories/{}/versions/{}/files", repo_id, version);
        let resp = client.get(&path)?;
        println!("{}", resp);
    } else {
        println!("Listing compat jobs for repo {} (UUID: {})", repo_id_or_name, repo_id);
        let resp = client.get("/api/v1/compat/jobs")?;
        println!("{}", resp);
    }
    Ok(())
}

fn delete(args: &Args, client: &ApiClient) -> Result<()> {
    let version_id = args
        .positional
        .get(1)
        .ok_or_else(|| anyhow::anyhow!("missing <version-id> argument\nusage: hbx-cli compat delete <version-id> [--force]"))?;

    if !args.has("force") {
        return Err(anyhow::anyhow!(
            "delete requires --force flag to confirm\nusage: hbx-cli compat delete <version-id> --force"
        ));
    }

    println!("Deleting compat version {}", version_id);
    let path = format!("/api/v1/versions/{}", version_id);
    let resp = client.delete(&path)?;
    println!("{}", resp);
    Ok(())
}

fn verify(args: &Args, client: &ApiClient) -> Result<()> {
    let repo_id_or_name = args
        .positional
        .get(1)
        .ok_or_else(|| anyhow::anyhow!("missing <repo-id> argument\nusage: hbx-cli compat verify <repo-id> [--mode <Quick|Full|Deep>]"))?;

    let mode = args.get_or("mode", "Quick");
    let repo_id = resolve_repo_id(client, repo_id_or_name)?;
    println!("Verifying compat repo {} (UUID: {}, mode: {})", repo_id_or_name, repo_id, mode);

    let path = format!("/api/v1/badou/repositories/{}/verify", repo_id);
    let resp = client.post(&path, "{}")?;
    println!("{}", resp);
    Ok(())
}

fn import(args: &Args, client: &ApiClient) -> Result<()> {
    let config_file = args
        .positional
        .get(1)
        .ok_or_else(|| anyhow::anyhow!("missing <config-file> argument\nusage: hbx-cli compat import <config-file> [--dry-run]"))?;

    let dry_run = args.has("dry-run");

    let config_data = std::fs::read_to_string(Path::new(config_file))
        .context(format!("failed to read config file: {}", config_file))?;

    println!(
        "Importing Duplicati config from {}{}",
        config_file,
        if dry_run { " (dry-run)" } else { "" }
    );

    if dry_run {
        let parsed: serde_json::Value = serde_json::from_str(&config_data)
            .context("failed to parse config as JSON")?;
        println!("Config name: {}", parsed.get("Name").map(|v| v.as_str().unwrap_or("(unnamed)")).unwrap_or("(unnamed)"));
        if let Some(sources) = parsed.get("Sources").and_then(|v| v.as_array()) {
            println!("Sources ({}):", sources.len());
            for src in sources {
                println!("  - {}", src.as_str().unwrap_or("?"));
            }
        }
        if let Some(dest) = parsed.get("Destination") {
            println!("Destination: {}", serde_json::to_string_pretty(dest)?);
        }
        if let Some(enc) = parsed.get("Encryption") {
            println!("Encryption: {}", serde_json::to_string_pretty(enc)?);
        }
        println!("\nDry-run complete. No changes made.");
        return Ok(());
    }

    let body = serde_json::json!({
        "format": "json",
        "config": config_data,
    })
    .to_string();

    let resp = client.post("/api/v1/compat/import", &body)?;
    println!("Import result: {}", resp);
    Ok(())
}