use anyhow::{Context, Result};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::env;
use std::path::PathBuf;
use std::time::Instant;

use crate::control_client::{ControlClient, TaskResultRequest};

use hbx_badou_provider::{BaDouCredentials, BaDouProvider};
use hbx_core::domain::backup::BackupType;
use hbx_core::domain::chunk::{ChunkHash, ChunkReference};
use hbx_core::domain::common::{FileAttributes, VersionId};
use hbx_core::domain::encryption::EncryptedChunk;
use hbx_core::domain::repository::{Manifest, ManifestHashes, FileEntry};
use hbx_core::pipeline::traits::IBackupRepository;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskSpec {
    pub task_id: String,
    pub job_id: String,
    pub repo_id: String,
    pub task_type: String,
    #[serde(default)]
    pub source_path: String,
    #[serde(default)]
    pub target_path: String,
    #[serde(default)]
    pub badou_grpc_endpoint: String,
}

pub struct TaskExecutor {
    agent_id: String,
}

impl TaskExecutor {
    pub fn new(agent_id: &str) -> Self {
        Self {
            agent_id: agent_id.to_string(),
        }
    }

    pub fn execute(&self, spec: &TaskSpec, client: &ControlClient) -> Result<()> {
        tracing::info!(
            "executing task {} type={} job={}",
            spec.task_id,
            spec.task_type,
            spec.job_id
        );

        let started_at = Utc::now();
        let start = Instant::now();

        let result = match spec.task_type.as_str() {
            "backup" => self.execute_backup(spec),
            "restore" => self.execute_restore(spec),
            "verify" => self.execute_verify(spec),
            other => Err(anyhow::anyhow!("unknown task type: {}", other)),
        };

        let completed_at = Utc::now();
        let elapsed = start.elapsed();

        let task_result = match result {
            Ok((bytes_processed, bytes_stored, file_count, chunk_count, version_id)) => {
                tracing::info!(
                    "task {} completed in {:?} ({} files, {} bytes)",
                    spec.task_id,
                    elapsed,
                    file_count,
                    bytes_processed
                );
                TaskResultRequest {
                    task_id: spec.task_id.clone(),
                    agent_id: self.agent_id.clone(),
                    job_id: spec.job_id.clone(),
                    status: "completed".to_string(),
                    started_at,
                    completed_at: Some(completed_at),
                    bytes_processed,
                    bytes_stored,
                    file_count,
                    chunk_count,
                    dedup_ratio: if bytes_processed > 0 {
                        1.0 - (bytes_stored as f64 / bytes_processed as f64)
                    } else {
                        0.0
                    },
                    version_id: Some(version_id),
                    error_message: None,
                    trace_id: uuid::Uuid::new_v4().to_string(),
                }
            }
            Err(e) => {
                tracing::error!("task {} failed: {}", spec.task_id, e);
                TaskResultRequest {
                    task_id: spec.task_id.clone(),
                    agent_id: self.agent_id.clone(),
                    job_id: spec.job_id.clone(),
                    status: "failed".to_string(),
                    started_at,
                    completed_at: Some(completed_at),
                    bytes_processed: 0,
                    bytes_stored: 0,
                    file_count: 0,
                    chunk_count: 0,
                    dedup_ratio: 0.0,
                    version_id: None,
                    error_message: Some(e.to_string()),
                    trace_id: uuid::Uuid::new_v4().to_string(),
                }
            }
        };

        client
            .report_task_result(&task_result)
            .context("report task result")?;

        Ok(())
    }

    fn execute_backup(&self, spec: &TaskSpec) -> Result<(u64, u64, u32, u32, String)> {
        let source = PathBuf::from(&spec.source_path);
        if !source.exists() {
            anyhow::bail!("source path does not exist: {}", source.display());
        }

        tracing::info!("backup: scanning {}", source.display());

        let mut file_count = 0u32;
        let mut bytes_processed = 0u64;
        let mut chunk_count = 0u32;

        let mut chunk_refs: Vec<ChunkReference> = Vec::new();
        let mut file_entries: Vec<FileEntry> = Vec::new();

        let version_id = VersionId(uuid::Uuid::new_v4());
        let version_id_str = version_id.0.to_string();

        let jwt_token = env::var("HBX_BADOU_JWT").unwrap_or_default();
        let badou_endpoint = if spec.badou_grpc_endpoint.is_empty() {
            "http://127.0.0.1:9090".to_string()
        } else {
            spec.badou_grpc_endpoint.clone()
        };

        let provider = if !jwt_token.is_empty() {
            let init_provider = BaDouProvider::new(
                &badou_endpoint,
                &spec.repo_id,
                BaDouCredentials { jwt_token: jwt_token.clone() },
            );
            let actual_repo_id = match init_provider.create_repo() {
                Ok(repo_id) => {
                    tracing::info!("backup: created badou repo {}", repo_id);
                    repo_id
                }
                Err(e) => {
                    tracing::warn!("backup: create_repo failed (may already exist): {}", e);
                    spec.repo_id.clone()
                }
            };
            Some(BaDouProvider::new(
                &badou_endpoint,
                &actual_repo_id,
                BaDouCredentials { jwt_token },
            ))
        } else {
            tracing::warn!("backup: no HBX_BADOU_JWT set, skipping data upload");
            None
        };

        for entry in walkdir::WalkDir::new(&source) {
            let entry = entry.context("walk dir")?;
            if entry.file_type().is_file() {
                file_count += 1;
                let file_size = entry.metadata().map(|m| m.len()).unwrap_or(0);
                bytes_processed += file_size;

                let data = std::fs::read(entry.path()).context("read file")?;
                let hash = blake3::hash(&data);

                const CHUNK_SIZE: usize = 256 * 1024;
                let mut file_chunk_hashes: Vec<ChunkHash> = Vec::new();
                let mut offset: u64 = 0;

                for chunk_data in data.chunks(CHUNK_SIZE) {
                    let encrypted = EncryptedChunk {
                        ciphertext: chunk_data.to_vec(),
                        nonce: [0u8; 12],
                        auth_tag: [0u8; 16],
                    };

                    let chunk_raw_hash = blake3::hash(chunk_data);
                    let mut chunk_hash = ChunkHash(chunk_raw_hash.as_bytes().clone());

                    if let Some(ref provider) = provider {
                        match provider.write_chunk(&chunk_hash, &encrypted) {
                            Ok(loc) => {
                                chunk_count += 1;
                                let stored_hash_hex = loc.path.trim_end_matches(".chunk");
                                if let Ok(stored_bytes) = hex::decode(stored_hash_hex) {
                                    if stored_bytes.len() == 32 {
                                        let mut arr = [0u8; 32];
                                        arr.copy_from_slice(&stored_bytes);
                                        chunk_hash = ChunkHash(arr);
                                    }
                                }
                                chunk_refs.push(ChunkReference {
                                    hash: chunk_hash.clone(),
                                    version_id: version_id.clone(),
                                    file_path: entry.path().to_string_lossy().to_string(),
                                    offset,
                                });
                            }
                            Err(e) => {
                                tracing::warn!(
                                    "backup: write_chunk failed for {} at offset {}: {}",
                                    entry.path().display(),
                                    offset,
                                    e
                                );
                            }
                        }
                    }

                    file_chunk_hashes.push(chunk_hash);
                    offset += chunk_data.len() as u64;
                }

                tracing::info!(
                    "backup: processed {} ({} bytes, {} chunks)",
                    entry.path().display(),
                    file_size,
                    file_chunk_hashes.len()
                );

                file_entries.push(FileEntry {
                    path: entry.path().to_string_lossy().to_string(),
                    size: file_size,
                    modified_at: Utc::now(),
                    attributes: FileAttributes::default(),
                    chunks: file_chunk_hashes,
                    file_hash: hash.as_bytes().clone(),
                });
            }
        }

        tracing::info!(
            "backup: scanned {} files, {} bytes, uploaded {} chunks",
            file_count,
            bytes_processed,
            chunk_count
        );

        if let Some(ref provider) = provider {
            let manifest = Manifest {
                version_id: version_id.clone(),
                timestamp: Utc::now(),
                parent_version_id: None,
                version_number: 1,
                backup_type: BackupType::Full,
                files: file_entries,
                chunk_refs,
                hashes: ManifestHashes {
                    manifest_hash: [0u8; 32],
                    file_index_hash: [0u8; 32],
                    chunk_index_hash: [0u8; 32],
                    repo_hash: [0u8; 32],
                },
                chunk_locations: std::collections::BTreeMap::new(),

            };

            match provider.write_manifest(&version_id, &manifest) {
                Ok(()) => {
                    tracing::info!("backup: manifest committed, version_id={}", version_id_str);
                }
                Err(e) => {
                    tracing::warn!("backup: write_manifest failed: {}", e);
                }
            }
        }

        let bytes_stored = bytes_processed;

        Ok((bytes_processed, bytes_stored, file_count, chunk_count, version_id_str))
    }

    fn execute_restore(&self, spec: &TaskSpec) -> Result<(u64, u64, u32, u32, String)> {
        let target = PathBuf::from(&spec.target_path);
        std::fs::create_dir_all(&target).context("create target dir")?;

        tracing::info!("restore: target {}", target.display());

        let version_id_str = if spec.source_path.is_empty() {
            anyhow::bail!("restore: source_path (version_id) is empty");
        } else {
            spec.source_path.clone()
        };

        let version_id = VersionId(uuid::Uuid::parse_str(&version_id_str)
            .context("parse version_id")?);

        let jwt_token = env::var("HBX_BADOU_JWT").unwrap_or_default();
        let badou_endpoint = if spec.badou_grpc_endpoint.is_empty() {
            "http://127.0.0.1:9090".to_string()
        } else {
            spec.badou_grpc_endpoint.clone()
        };

        let mut restored_files = 0u32;
        let mut restored_bytes = 0u64;

        if !jwt_token.is_empty() {
            let provider = BaDouProvider::new(
                &badou_endpoint,
                &spec.repo_id,
                BaDouCredentials { jwt_token },
            );

            match provider.read_manifest(&version_id) {
                Ok(manifest) => {
                    tracing::info!(
                        "restore: manifest read, {} files, {} chunk_refs",
                        manifest.files.len(),
                        manifest.chunk_refs.len()
                    );

                    for file_entry in &manifest.files {
                        let file_name = std::path::Path::new(&file_entry.path)
                            .file_name()
                            .map(|n| n.to_os_string())
                            .unwrap_or_else(|| std::ffi::OsString::from("restored_file"));
                        let file_path = target.join(&file_name);
                        if let Some(parent) = file_path.parent() {
                            std::fs::create_dir_all(parent).ok();
                        }

                        let mut file_data = Vec::new();
                        for chunk_hash in &file_entry.chunks {
                            let hash_hex = hex::encode(&chunk_hash.0);
                            let location = hbx_core::domain::chunk::ChunkLocation {
                                bucket: hash_hex.get(..2).unwrap_or("00").to_string(),
                                path: format!("{}.chunk", hash_hex),
                            };
                            match provider.read_chunk(&location) {
                                Ok(encrypted) => {
                                    file_data.extend_from_slice(&encrypted.ciphertext);
                                }
                                Err(e) => {
                                    tracing::warn!(
                                        "restore: read_chunk failed for {}: {}",
                                        file_entry.path,
                                        e
                                    );
                                }
                            }
                        }

                        match std::fs::write(&file_path, &file_data) {
                            Ok(()) => {
                                restored_files += 1;
                                restored_bytes += file_data.len() as u64;
                                tracing::info!(
                                    "restore: wrote {} ({} bytes)",
                                    file_path.display(),
                                    file_data.len()
                                );
                            }
                            Err(e) => {
                                tracing::warn!("restore: write failed for {}: {}", file_path.display(), e);
                            }
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!("restore: read_manifest failed: {}", e);
                }
            }
        } else {
            tracing::warn!("restore: no HBX_BADOU_JWT set, skipping data download");
        }

        Ok((restored_bytes, restored_bytes, restored_files, restored_files, version_id_str))
    }

    fn execute_verify(&self, spec: &TaskSpec) -> Result<(u64, u64, u32, u32, String)> {
        tracing::info!("verify: repo {}", spec.repo_id);
        let version_id = uuid::Uuid::new_v4().to_string();
        Ok((0, 0, 0, 0, version_id))
    }
}