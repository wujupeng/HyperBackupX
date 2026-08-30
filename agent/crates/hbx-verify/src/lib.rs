use std::sync::Arc;

use hbx_core::domain::chunk::{ChunkHash, ChunkLocation};
use hbx_core::domain::common::VersionId;
use hbx_core::domain::repository::{Manifest, ManifestHashes};
use hbx_core::domain::verify::{VerifyFailure, VerifyItemType, VerifyMode, VerifyReport};
use hbx_core::pipeline::{
    IBackupRepository, ICompressor, IEncryptionProvider, IIntegrityVerifier, VerifyError,
};
use sha2::{Digest, Sha256};

mod consistency;
mod multilayer;

pub use consistency::{
    ConsistencyChecker, ConsistencyReport, RepairResult,
};
pub use multilayer::{
    FileMetadata, FileVerificationResult, LayerStatus,
    MultiLayerReport, MultiLayerVerifier, VerificationLayer,
};

pub struct IntegrityVerifier {
    compressor: Arc<dyn ICompressor>,
    encryption: Arc<dyn IEncryptionProvider>,
}

impl IntegrityVerifier {
    pub fn new(
        compressor: Arc<dyn ICompressor>,
        encryption: Arc<dyn IEncryptionProvider>,
    ) -> Self {
        Self { compressor, encryption }
    }
}

impl IIntegrityVerifier for IntegrityVerifier {
    fn verify(
        &self,
        version_id: &VersionId,
        mode: VerifyMode,
        repo: &dyn IBackupRepository,
    ) -> Result<VerifyReport, VerifyError> {
        let started_at = chrono::Utc::now();
        let manifest = repo.read_manifest(version_id)?;
        let mut failures: Vec<VerifyFailure> = Vec::new();
        let mut total_checked: u64 = 0;
        let mut passed: u64 = 0;

        let manifest_ok = self.verify_manifest_hash(&manifest)?;
        total_checked += 1;
        if manifest_ok {
            passed += 1;
        } else {
            failures.push(VerifyFailure {
                item_type: VerifyItemType::Manifest,
                identifier: version_id.0.to_string(),
                expected_hash: manifest.hashes.manifest_hash,
                actual_hash: [0u8; 32],
            });
        }

        let repo_hash_ok = self.verify_repo_hash(&manifest)?;
        total_checked += 1;
        if repo_hash_ok {
            passed += 1;
        } else {
            failures.push(VerifyFailure {
                item_type: VerifyItemType::Repo,
                identifier: version_id.0.to_string(),
                expected_hash: manifest.hashes.repo_hash,
                actual_hash: [0u8; 32],
            });
        }

        let chunk_hashes: Vec<ChunkHash> = manifest
            .chunk_refs
            .iter()
            .map(|cr| cr.hash.clone())
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();

        match mode {
            VerifyMode::Quick => {}
            VerifyMode::Random { ratio } => {
                let sample = sample_chunks(&chunk_hashes, ratio);
                for hash in &sample {
                    total_checked += 1;
                    match self.verify_chunk(hash, repo) {
                        Ok(true) => passed += 1,
                        Ok(false) => {
                            failures.push(VerifyFailure {
                                item_type: VerifyItemType::Chunk,
                                identifier: hex::encode(hash.0),
                                expected_hash: hash.0,
                                actual_hash: [0u8; 32],
                            });
                        }
                        Err(e) => {
                            tracing::warn!(error = %e, chunk = %hex::encode(hash.0), "chunk verification error");
                            failures.push(VerifyFailure {
                                item_type: VerifyItemType::Chunk,
                                identifier: hex::encode(hash.0),
                                expected_hash: hash.0,
                                actual_hash: [0u8; 32],
                            });
                        }
                    }
                }
            }
            VerifyMode::Full | VerifyMode::Deep => {
                for hash in &chunk_hashes {
                    total_checked += 1;
                    match self.verify_chunk(hash, repo) {
                        Ok(true) => passed += 1,
                        Ok(false) => {
                            failures.push(VerifyFailure {
                                item_type: VerifyItemType::Chunk,
                                identifier: hex::encode(hash.0),
                                expected_hash: hash.0,
                                actual_hash: [0u8; 32],
                            });
                        }
                        Err(e) => {
                            tracing::warn!(error = %e, chunk = %hex::encode(hash.0), "chunk verification error");
                            failures.push(VerifyFailure {
                                item_type: VerifyItemType::Chunk,
                                identifier: hex::encode(hash.0),
                                expected_hash: hash.0,
                                actual_hash: [0u8; 32],
                            });
                        }
                    }
                }

                if mode == VerifyMode::Deep {
                    for file in &manifest.files {
                        total_checked += 1;
                        match self.verify_file_hash(file, repo) {
                            Ok(true) => passed += 1,
                            Ok(false) => {
                                failures.push(VerifyFailure {
                                    item_type: VerifyItemType::File,
                                    identifier: file.path.clone(),
                                    expected_hash: file.file_hash,
                                    actual_hash: [0u8; 32],
                                });
                            }
                            Err(e) => {
                                tracing::warn!(error = %e, file = %file.path, "file verification error");
                                failures.push(VerifyFailure {
                                    item_type: VerifyItemType::File,
                                    identifier: file.path.clone(),
                                    expected_hash: file.file_hash,
                                    actual_hash: [0u8; 32],
                                });
                            }
                        }
                    }
                }
            }
        }

        let failed = total_checked - passed;
        let completed_at = chrono::Utc::now();

        Ok(VerifyReport {
            version_id: version_id.clone(),
            mode,
            started_at,
            completed_at,
            total_checked,
            passed,
            failed,
            failures,
        })
    }
}

impl IntegrityVerifier {
    fn verify_manifest_hash(&self, manifest: &Manifest) -> Result<bool, VerifyError> {
        let temp_manifest = Manifest {
            version_id: manifest.version_id.clone(),
            timestamp: manifest.timestamp,
            parent_version_id: manifest.parent_version_id.clone(),
            version_number: manifest.version_number,
            backup_type: manifest.backup_type,
            files: manifest.files.clone(),
            chunk_refs: manifest.chunk_refs.clone(),
            hashes: ManifestHashes {
                manifest_hash: [0u8; 32],
                file_index_hash: manifest.hashes.file_index_hash,
                chunk_index_hash: manifest.hashes.chunk_index_hash,
                repo_hash: [0u8; 32],
            },
            chunk_locations: manifest.chunk_locations.clone(),
        };
        let bytes = serde_json::to_vec(&temp_manifest)
            .map_err(|e| VerifyError::Failed(format!("serialize: {}", e)))?;
        let computed: [u8; 32] = {
            let mut h = Sha256::new();
            h.update(&bytes);
            h.finalize().into()
        };
        Ok(computed == manifest.hashes.manifest_hash)
    }

    fn verify_repo_hash(&self, manifest: &Manifest) -> Result<bool, VerifyError> {
        let mut hasher = Sha256::new();
        hasher.update(manifest.hashes.manifest_hash);
        hasher.update(manifest.hashes.file_index_hash);
        hasher.update(manifest.hashes.chunk_index_hash);
        let computed: [u8; 32] = hasher.finalize().into();
        Ok(computed == manifest.hashes.repo_hash)
    }

    fn verify_chunk(
        &self,
        hash: &ChunkHash,
        repo: &dyn IBackupRepository,
    ) -> Result<bool, VerifyError> {
        let location = chunk_location_from_hash(hash);
        let encrypted = repo.read_chunk(&location)?;
        let compressed = self.encryption.decrypt_chunk(&encrypted)?;
        let plain = self.compressor.decompress(&compressed)?;
        let computed = blake3::hash(&plain);
        Ok(computed.as_bytes() == &hash.0)
    }

    fn verify_file_hash(
        &self,
        file: &hbx_core::domain::repository::FileEntry,
        repo: &dyn IBackupRepository,
    ) -> Result<bool, VerifyError> {
        let mut hasher = Sha256::new();
        for chunk_hash in &file.chunks {
            let location = chunk_location_from_hash(chunk_hash);
            let encrypted = repo.read_chunk(&location)?;
            let compressed = self.encryption.decrypt_chunk(&encrypted)?;
            let plain = self.compressor.decompress(&compressed)?;
            hasher.update(&plain);
        }
        let computed: [u8; 32] = hasher.finalize().into();
        Ok(computed == file.file_hash)
    }
}

fn chunk_location_from_hash(hash: &ChunkHash) -> ChunkLocation {
    ChunkLocation {
        bucket: format!("{:02x}", hash.0[0]),
        path: hex::encode(hash.0) + ".chunk",
    }
}

fn sample_chunks(hashes: &[ChunkHash], ratio: f64) -> Vec<ChunkHash> {
    if hashes.is_empty() {
        return Vec::new();
    }
    let ratio = ratio.clamp(0.0, 1.0);
    let target = ((hashes.len() as f64) * ratio).ceil() as usize;
    let target = target.max(1).min(hashes.len());

    let mut indices: Vec<usize> = (0..hashes.len()).collect();
    let mut rng = rand::thread_rng();
    for i in (1..indices.len()).rev() {
        let j = rand::Rng::gen_range(&mut rng, 0..=i);
        indices.swap(i, j);
    }
    indices.truncate(target);
    indices.into_iter().map(|i| hashes[i].clone()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use hbx_chunker::FixedChunker;
    use hbx_compress::ZstdCompressor;
    use hbx_core::domain::backup::{
        BackupDestination, BackupJob, BackupSource, JobStatus,
    };
    use hbx_core::domain::common::{
        CompressionAlgorithm, CompressionProfile, EncryptionProfileRef, JobId,
        RepositoryId, RetentionPolicyRef, ScheduleRef,
    };
    use hbx_core::domain::repository::BackendType;
    use hbx_core::pipeline::ChunkStrategy;
    use hbx_dedup::LocalDedupIndex;
    use hbx_engine::{BackupEngine, NoOpEncryptionProvider};
    use hbx_repo::{LocalRepository, RepositoryInitializer};
    use hbx_scanner::LocalScanner;
    use std::fs;
    use std::sync::Arc;
    use uuid::Uuid;

    fn make_job(source_path: std::path::PathBuf) -> BackupJob {
        BackupJob {
            job_id: JobId(Uuid::new_v4()),
            name: "test-verify".to_string(),
            source: BackupSource {
                paths: vec![source_path],
                include_rules: vec![],
                exclude_rules: vec![],
            },
            destination: BackupDestination {
                repository_id: RepositoryId(Uuid::new_v4()),
                logical_path: "/".to_string(),
            },
            schedule: ScheduleRef(Uuid::new_v4()),
            retention_policy: RetentionPolicyRef(Uuid::new_v4()),
            encryption_profile: EncryptionProfileRef(Uuid::new_v4()),
            compression_profile: CompressionProfile {
                algorithm: CompressionAlgorithm::Zstd,
                level: 3,
            },
            chunking_profile: hbx_core::domain::chunking::ChunkingProfile::Standard,
            status: JobStatus::Active,
            created_at: chrono::Utc::now(),
        }
    }

    struct TestSetup {
        _src_dir: tempfile::TempDir,
        repo_dir: tempfile::TempDir,
        repo: LocalRepository,
        version_id: VersionId,
    }

    async fn setup_backup() -> TestSetup {
        let src_dir = tempfile::tempdir().unwrap();
        let repo_dir = tempfile::tempdir().unwrap();

        fs::write(src_dir.path().join("a.txt"), b"hello world").unwrap();
        fs::write(src_dir.path().join("b.txt"), b"foo bar baz qux").unwrap();
        fs::create_dir(src_dir.path().join("sub")).unwrap();
        fs::write(src_dir.path().join("sub").join("c.txt"), b"nested file content here").unwrap();

        RepositoryInitializer::new(repo_dir.path())
            .init(RepositoryId(Uuid::new_v4()), BackendType::Local)
            .unwrap();
        let repo = LocalRepository::open(repo_dir.path()).unwrap();

        let engine = BackupEngine::builder()
            .scanner(LocalScanner::new())
            .chunker(FixedChunker::new())
            .dedup(LocalDedupIndex::new())
            .compressor(ZstdCompressor::default())
            .encryption(NoOpEncryptionProvider)
            .repo(LocalRepository::open(repo_dir.path()).unwrap())
            .memory_limit(256 * 1024 * 1024)
            .chunk_strategy(ChunkStrategy::Fixed { chunk_size: 1024 })
            .build()
            .unwrap();

        let job = make_job(src_dir.path().to_path_buf());
        let tracker = engine.execution_tracker(&job.job_id);
        let result = engine.run_backup(&job, &tracker).await;
        let version_id = result.unwrap().version_id.unwrap();

        TestSetup {
            _src_dir: src_dir,
            repo_dir,
            repo,
            version_id,
        }
    }

    fn make_verifier() -> IntegrityVerifier {
        IntegrityVerifier::new(
            Arc::new(ZstdCompressor::default()),
            Arc::new(NoOpEncryptionProvider),
        )
    }

    #[tokio::test]
    async fn test_quick_mode() {
        let setup = setup_backup().await;
        let verifier = make_verifier();
        let report = verifier
            .verify(&setup.version_id, VerifyMode::Quick, &setup.repo)
            .unwrap();

        assert_eq!(report.mode, VerifyMode::Quick);
        assert!(report.total_checked >= 2);
        assert_eq!(report.failed, 0);
        assert!(report.failures.is_empty());
    }

    #[tokio::test]
    async fn test_full_mode() {
        let setup = setup_backup().await;
        let verifier = make_verifier();
        let report = verifier
            .verify(&setup.version_id, VerifyMode::Full, &setup.repo)
            .unwrap();

        assert_eq!(report.mode, VerifyMode::Full);
        assert!(report.total_checked > 2);
        assert_eq!(report.failed, 0);
        assert!(report.failures.is_empty());
    }

    #[tokio::test]
    async fn test_deep_mode() {
        let setup = setup_backup().await;
        let verifier = make_verifier();
        let report = verifier
            .verify(&setup.version_id, VerifyMode::Deep, &setup.repo)
            .unwrap();

        assert_eq!(report.mode, VerifyMode::Deep);
        assert!(report.total_checked > 2);
        assert_eq!(report.failed, 0);
        assert!(report.failures.is_empty());
    }

    #[tokio::test]
    async fn test_random_mode() {
        let setup = setup_backup().await;
        let verifier = make_verifier();
        let report = verifier
            .verify(
                &setup.version_id,
                VerifyMode::Random { ratio: 0.5 },
                &setup.repo,
            )
            .unwrap();

        assert!(report.total_checked >= 2);
        assert_eq!(report.failed, 0);
    }

    #[tokio::test]
    async fn test_detect_corrupted_chunk() {
        let setup = setup_backup().await;
        let verifier = make_verifier();

        let manifest = setup.repo.read_manifest(&setup.version_id).unwrap();
        let first_hash = manifest.chunk_refs[0].hash.clone();
        let location = chunk_location_from_hash(&first_hash);
        let chunk_path = setup
            .repo_dir
            .path()
            .join("chunks")
            .join(&location.bucket)
            .join(&location.path);
        fs::write(&chunk_path, b"corrupted data").unwrap();

        let report = verifier
            .verify(&setup.version_id, VerifyMode::Full, &setup.repo)
            .unwrap();

        assert!(report.failed > 0);
        assert!(!report.failures.is_empty());
        assert_eq!(report.failures[0].item_type, VerifyItemType::Chunk);
    }
}
