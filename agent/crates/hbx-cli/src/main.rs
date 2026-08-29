use std::env;
use std::process::ExitCode;

use anyhow::Result;

use hbx_cli::args;
use hbx_cli::client;
use hbx_cli::compat;

fn main() -> ExitCode {
    let raw_args: Vec<String> = env::args().skip(1).collect();

    if raw_args.is_empty() {
        print_help();
        return ExitCode::SUCCESS;
    }

    match run(&raw_args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("Error: {}", e);
            ExitCode::FAILURE
        }
    }
}

fn run(raw_args: &[String]) -> Result<()> {
    let parsed = args::Args::parse(raw_args);

    let command = parsed
        .positional
        .first()
        .ok_or_else(|| anyhow::anyhow!("missing command"))?;

    if command == "help" || command == "--help" || command == "-h" {
        print_help();
        return Ok(());
    }

    if command == "version" || command == "--version" || command == "-V" {
        println!("hbx-cli {} (HyperBackup X CLI)", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }

    let server_url = env::var("HBX_SERVER_URL").unwrap_or_else(|_| "http://localhost:8080".to_string());
    let token = env::var("HBX_TOKEN").ok();

    let client = client::ApiClient::new(&server_url, token);

    let sub_args: Vec<String> = parsed.positional[1..].to_vec();
    let sub_parsed = args::Args::parse(&sub_args);

    match command.as_str() {
        "compat" => compat::run(&sub_parsed, &client),
        "backup" => {
            let mut compat_args = vec!["backup".to_string()];
            compat_args.extend(sub_args);
            let compat_parsed = args::Args::parse(&compat_args);
            compat::run(&compat_parsed, &client)
        }
        "restore" => {
            let mut compat_args = vec!["restore".to_string()];
            compat_args.extend(sub_args);
            let compat_parsed = args::Args::parse(&compat_args);
            compat::run(&compat_parsed, &client)
        }
        "list" => {
            let mut compat_args = vec!["list".to_string()];
            compat_args.extend(sub_args);
            let compat_parsed = args::Args::parse(&compat_args);
            compat::run(&compat_parsed, &client)
        }
        "import" => {
            let mut compat_args = vec!["import".to_string()];
            compat_args.extend(sub_args);
            let compat_parsed = args::Args::parse(&compat_args);
            compat::run(&compat_parsed, &client)
        }
        _ => Err(anyhow::anyhow!(
            "unknown command: {}\navailable: compat, backup, restore, list, import, help, version",
            command
        )),
    }
}

fn print_help() {
    println!("hbx-cli - HyperBackup X CLI (Duplicati-compatible)");
    println!();
    println!("USAGE:");
    println!("    hbx-cli <command> [subcommand] [options]");
    println!();
    println!("COMMANDS:");
    println!("    compat backup <job-id> --repo <repo-id>");
    println!("        Trigger a compatibility backup for the specified job");
    println!();
    println!("    compat restore <version-id> --target <path> [--source <prefix>] [--overwrite <policy>] [--selection <rule>] [--mode <mode>]");
    println!("        Restore a compatibility version to the target path");
    println!("        --source: path prefix to restore (default: all)");
    println!("        --overwrite: skip|overwrite|rename (default: skip)");
    println!("        --selection: all|include:<patterns>|exclude:<patterns> (default: all)");
    println!("        --mode: original|overwrite|merge (default: original)");
    println!();
    println!("    compat list <repo-id> [--versions] [--files <version>]");
    println!("        List compatibility jobs, versions, or files");
    println!("        --versions: list versions for the repo");
    println!("        --files <version>: list files for the specified version");
    println!();
    println!("    compat delete <version-id> --force");
    println!("        Delete a compatibility version (requires --force)");
    println!();
    println!("    compat verify <repo-id> [--mode <Quick|Full|Deep>]");
    println!("        Verify a compatibility repository");
    println!();
    println!("    compat import <config-file> [--dry-run]");
    println!("        Import a Duplicati configuration file");
    println!("        --dry-run: parse and preview without importing");
    println!();
    println!("    help, --help, -h    Show this help message");
    println!("    version, --version, -V    Show version information");
    println!();
    println!("ENVIRONMENT:");
    println!("    HBX_SERVER_URL    Control Plane API URL (default: http://localhost:8080)");
    println!("    HBX_TOKEN         JWT authentication token");
    println!();
    println!("EXAMPLES:");
    println!("    hbx-cli compat backup 550e8400-e29b-41d4-a716-446655440000 --repo 660e8400-e29b-41d4-a716-446655440000");
    println!("    hbx-cli compat restore 770e8400-e29b-41d4-a716-446655440000 --target /restore/path");
    println!("    hbx-cli compat list 660e8400-e29b-41d4-a716-446655440000 --versions");
    println!("    hbx-cli compat import duplicati-config.json --dry-run");
}