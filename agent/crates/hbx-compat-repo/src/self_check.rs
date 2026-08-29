use std::collections::HashSet;
use std::fs;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::repository::{CompatRepoError, CompatibleRepository};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DamageType {
    ChunkMissing,
    ChunkTampered,
    RepoInconsistent,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DamageLocation {
    pub bucket: Option<String>,
    pub path: String,
    pub version_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DamageFinding {
    pub damage_type: DamageType,
    pub location: DamageLocation,
    pub description: String,
    pub affected_versions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompatRepoIntegrityReport {
    pub repository_id: String,
    pub checked_at: DateTime<Utc>,
    pub total_chunks_scanned: u64,
    pub total_manifests_scanned: u64,
    pub findings: Vec<DamageFinding>,
}

impl CompatRepoIntegrityReport {
    pub fn is_clean(&self) -> bool {
        self.findings.is_empty()
    }

    pub fn finding_count(&self, dtype: &DamageType) -> usize {
        self.findings
            .iter()
            .filter(|f| &f.damage_type == dtype)
            .count()
    }
}

pub trait ICompatibilityRepository {
    fn self_check(&self) -> Result<CompatRepoIntegrityReport, CompatRepoError>;
}

impl ICompatibilityRepository for CompatibleRepository {
    fn self_check(&self) -> Result<CompatRepoIntegrityReport, CompatRepoError> {
        let metadata_path = self.root().join("compat_repository.json");
        let metadata_data = fs::read(&metadata_path)?;
        let metadata: crate::CompatRepoMetadata = serde_json::from_slice(&metadata_data)?;
        let repository_id = metadata.repository_id.clone();

        let mut findings = Vec::new();
        let mut total_chunks_scanned: u64 = 0;
        let mut total_manifests_scanned: u64 = 0;

        let dlists_dir = self.root().join("dlists");
        let mut manifest_hashes: HashSet<[u8; 32]> = HashSet::new();
        let mut all_referenced_chunks: HashSet<[u8; 32]> = HashSet::new();
        let mut versions_by_chunk: std::collections::HashMap<[u8; 32], Vec<String>> =
            std::collections::HashMap::new();

        if dlists_dir.exists() {
            for entry in fs::read_dir(&dlists_dir)? {
                let entry = entry?;
                let path = entry.path();
                if !path.is_file() {
                    continue;
                }
                let filename = path
                    .file_name()
                    .unwrap()
                    .to_string_lossy()
                    .to_string();
                if !filename.ends_with(".dlist") {
                    continue;
                }

                let vid = &filename[..filename.len() - ".dlist".len()];
                let data = fs::read(&path)?;
                let manifest: crate::repository::CompatibilityManifest =
                    match serde_json::from_slice(&data) {
                        Ok(m) => m,
                        Err(_) => {
                            findings.push(DamageFinding {
                                damage_type: DamageType::RepoInconsistent,
                                location: DamageLocation {
                                    bucket: None,
                                    path: filename.clone(),
                                    version_id: Some(vid.to_string()),
                                },
                                description: "manifest is not valid JSON".to_string(),
                                affected_versions: vec![vid.to_string()],
                            });
                            total_manifests_scanned += 1;
                            continue;
                        }
                    };

                total_manifests_scanned += 1;

                let mut manifest_chunk_set: HashSet<[u8; 32]> = HashSet::new();
                for file_entry in &manifest.files {
                    for chunk_hash in &file_entry.chunks {
                        manifest_chunk_set.insert(chunk_hash.0);
                        all_referenced_chunks.insert(chunk_hash.0);
                        versions_by_chunk
                            .entry(chunk_hash.0)
                            .or_default()
                            .push(vid.to_string());
                    }
                }

                let mut ref_chunk_set: HashSet<[u8; 32]> = HashSet::new();
                for chunk_ref in &manifest.chunk_refs {
                    ref_chunk_set.insert(chunk_ref.hash.0);
                    all_referenced_chunks.insert(chunk_ref.hash.0);
                    versions_by_chunk
                        .entry(chunk_ref.hash.0)
                        .or_default()
                        .push(vid.to_string());
                }

                if manifest_chunk_set != ref_chunk_set {
                    let only_in_files: Vec<_> = manifest_chunk_set
                        .difference(&ref_chunk_set)
                        .collect();
                    let only_in_refs: Vec<_> = ref_chunk_set
                        .difference(&manifest_chunk_set)
                        .collect();
                    findings.push(DamageFinding {
                        damage_type: DamageType::RepoInconsistent,
                        location: DamageLocation {
                            bucket: None,
                            path: filename.clone(),
                            version_id: Some(vid.to_string()),
                        },
                        description: format!(
                            "chunk set mismatch: {} in files only, {} in refs only",
                            only_in_files.len(),
                            only_in_refs.len()
                        ),
                        affected_versions: vec![vid.to_string()],
                    });
                }

                for hash_bytes in &manifest_chunk_set {
                    manifest_hashes.insert(*hash_bytes);
                }
            }
        }

        for hash_bytes in &all_referenced_chunks {
            total_chunks_scanned += 1;
            let bucket = format!("{:02x}", hash_bytes[0]);
            let chunk_filename = format!("{}.dblock", hex::encode(hash_bytes));
            let chunk_path = self
                .root()
                .join("dblocks")
                .join(&bucket)
                .join(&chunk_filename);

            if !chunk_path.exists() {
                let affected = versions_by_chunk.get(hash_bytes).cloned().unwrap_or_default();
                findings.push(DamageFinding {
                    damage_type: DamageType::ChunkMissing,
                    location: DamageLocation {
                        bucket: Some(bucket),
                        path: chunk_filename,
                        version_id: None,
                    },
                    description: "referenced chunk file not found on disk".to_string(),
                    affected_versions: affected,
                });
                continue;
            }

            let chunk_data = fs::read(&chunk_path)?;
            let mut hasher = Sha256::new();
            hasher.update(&chunk_data);
            let computed = hasher.finalize();
            if computed.as_slice() != hash_bytes.as_slice() {
                let affected = versions_by_chunk.get(hash_bytes).cloned().unwrap_or_default();
                findings.push(DamageFinding {
                    damage_type: DamageType::ChunkTampered,
                    location: DamageLocation {
                        bucket: Some(bucket),
                        path: chunk_filename,
                        version_id: None,
                    },
                    description: format!(
                        "chunk content hash mismatch: expected {}, got {}",
                        hex::encode(hash_bytes),
                        hex::encode(computed)
                    ),
                    affected_versions: affected,
                });
            }
        }

        let dblocks_dir = self.root().join("dblocks");
        if dblocks_dir.exists() {
            for entry in fs::read_dir(&dblocks_dir)? {
                let entry = entry?;
                if !entry.path().is_dir() {
                    continue;
                }
                let bucket = entry
                    .path()
                    .file_name()
                    .unwrap()
                    .to_string_lossy()
                    .to_string();
                for chunk_entry in fs::read_dir(entry.path())? {
                    let chunk_entry = chunk_entry?;
                    let chunk_filename = chunk_entry
                        .path()
                        .file_name()
                        .unwrap()
                        .to_string_lossy()
                        .to_string();
                    if !chunk_filename.ends_with(".dblock") {
                        continue;
                    }
                    let hash_hex = &chunk_filename[..chunk_filename.len() - ".dblock".len()];
                    let hash_bytes = match hex::decode(hash_hex) {
                        Ok(bytes) if bytes.len() == 32 => {
                            let mut arr = [0u8; 32];
                            arr.copy_from_slice(&bytes);
                            arr
                        }
                        _ => {
                            findings.push(DamageFinding {
                                damage_type: DamageType::RepoInconsistent,
                                location: DamageLocation {
                                    bucket: Some(bucket.clone()),
                                    path: chunk_filename,
                                    version_id: None,
                                },
                                description: "chunk filename is not a valid hex-encoded SHA-256"
                                    .to_string(),
                                affected_versions: vec![],
                            });
                            continue;
                        }
                    };
                    if !all_referenced_chunks.contains(&hash_bytes) {
                        findings.push(DamageFinding {
                            damage_type: DamageType::RepoInconsistent,
                            location: DamageLocation {
                                bucket: Some(bucket.clone()),
                                path: chunk_filename,
                                version_id: None,
                            },
                            description: "orphaned chunk not referenced by any manifest".to_string(),
                            affected_versions: vec![],
                        });
                    }
                }
            }
        }

        Ok(CompatRepoIntegrityReport {
            repository_id,
            checked_at: Utc::now(),
            total_chunks_scanned,
            total_manifests_scanned,
            findings,
        })
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use crate::repository::{
        CompatChunkLocation, CompatFileEntry, CompatibilityHashes, CompatibilityManifest,
    };
    use hbx_core::domain::chunk::{ChunkHash, ChunkReference};
    use hbx_core::domain::common::VersionId;
    use uuid::Uuid;

    fn setup_repo_with_data() -> (
        tempfile::TempDir,
        CompatibleRepository,
        ChunkHash,
        Vec<u8>,
        String,
    ) {
        let tmp = tempfile::tempdir().unwrap();
        let repo = CompatibleRepository::init(tmp.path(), "test-repo".to_string()).unwrap();

        let chunk_data = b"hello world chunk data for testing";
        let mut hasher = Sha256::new();
        hasher.update(chunk_data);
        let hash = ChunkHash(hasher.finalize().into());

        repo.write_compat_chunk(&hash, chunk_data).unwrap();

        let version_id = "v1".to_string();
        let manifest = CompatibilityManifest {
            version_id: version_id.clone(),
            timestamp: Utc::now(),
            parent_version_id: None,
            version_number: 1,
            backup_type: "full".to_string(),
            files: vec![CompatFileEntry {
                path: "/test/file.txt".to_string(),
                size: chunk_data.len() as u64,
                modified_at: Utc::now(),
                chunks: vec![hash.clone()],
                file_hash: [0u8; 32],
            }],
            chunk_refs: vec![ChunkReference {
                hash: hash.clone(),
                version_id: VersionId(Uuid::new_v4()),
                file_path: "/test/file.txt".to_string(),
                offset: 0,
            }],
            hashes: CompatibilityHashes {
                manifest_hash: [0u8; 32],
                file_index_hash: [0u8; 32],
                chunk_index_hash: [0u8; 32],
            },
        };
        repo.write_compat_manifest(&version_id, &manifest)
            .unwrap();

        (tmp, repo, hash, chunk_data.to_vec(), version_id)
    }

    #[test]
    fn test_self_check_clean_repo() {
        let (_tmp, repo, _hash, _data, _vid) = setup_repo_with_data();
        let report = repo.self_check().unwrap();
        assert!(report.is_clean(), "clean repo should have no findings");
        assert_eq!(report.total_manifests_scanned, 1);
        assert_eq!(report.total_chunks_scanned, 1);
    }

    #[test]
    fn test_self_check_detects_chunk_missing() {
        let (tmp, repo, hash, _data, _vid) = setup_repo_with_data();

        let loc = CompatChunkLocation {
            bucket: format!("{:02x}", hash.0[0]),
            path: format!("{}.dblock", hex::encode(hash.0)),
        };
        repo.delete_compat_chunk(&loc).unwrap();

        let report = repo.self_check().unwrap();
        assert!(!report.is_clean());
        assert_eq!(
            report.finding_count(&DamageType::ChunkMissing),
            1,
            "should detect 1 missing chunk"
        );
        let _ = tmp;
    }

    #[test]
    fn test_self_check_detects_chunk_tampered() {
        let (tmp, repo, hash, _data, _vid) = setup_repo_with_data();

        let chunk_path = tmp
            .path()
            .join("dblocks")
            .join(format!("{:02x}", hash.0[0]))
            .join(format!("{}.dblock", hex::encode(hash.0)));
        fs::write(&chunk_path, b"tampered content that changes the hash").unwrap();

        let report = repo.self_check().unwrap();
        assert!(!report.is_clean());
        assert_eq!(
            report.finding_count(&DamageType::ChunkTampered),
            1,
            "should detect 1 tampered chunk"
        );
        let _ = tmp;
    }

    #[test]
    fn test_self_check_detects_repo_inconsistent_orphaned_chunk() {
        let (tmp, repo, _hash, _data, _vid) = setup_repo_with_data();

        let orphan_hash = ChunkHash([0xaa; 32]);
        let orphan_path = tmp
            .path()
            .join("dblocks")
            .join("aa")
            .join(format!("{}.dblock", hex::encode(orphan_hash.0)));
        fs::write(&orphan_path, b"orphaned chunk data").unwrap();

        let report = repo.self_check().unwrap();
        assert!(!report.is_clean());
        assert!(
            report.finding_count(&DamageType::RepoInconsistent) >= 1,
            "should detect orphaned chunk as RepoInconsistent"
        );
        let _ = tmp;
    }

    #[test]
    fn test_self_check_detects_corrupt_manifest() {
        let (tmp, repo, _hash, _data, _vid) = setup_repo_with_data();

        let manifest_path = tmp.path().join("dlists").join("v1.dlist");
        fs::write(&manifest_path, b"{ this is not valid json }").unwrap();

        let report = repo.self_check().unwrap();
        assert!(!report.is_clean());
        assert!(
            report.finding_count(&DamageType::RepoInconsistent) >= 1,
            "should detect corrupt manifest as RepoInconsistent"
        );
        let _ = tmp;
    }

    #[test]
    fn test_self_check_detects_chunk_set_mismatch() {
        let tmp = tempfile::tempdir().unwrap();
        let repo = CompatibleRepository::init(tmp.path(), "test-repo".to_string()).unwrap();

        let chunk_data = b"test data";
        let mut hasher = Sha256::new();
        hasher.update(chunk_data);
        let hash = ChunkHash(hasher.finalize().into());
        repo.write_compat_chunk(&hash, chunk_data).unwrap();

        let extra_hash = ChunkHash([0xbb; 32]);

        let manifest = CompatibilityManifest {
            version_id: "v1".to_string(),
            timestamp: Utc::now(),
            parent_version_id: None,
            version_number: 1,
            backup_type: "full".to_string(),
            files: vec![CompatFileEntry {
                path: "/test/file.txt".to_string(),
                size: chunk_data.len() as u64,
                modified_at: Utc::now(),
                chunks: vec![hash],
                file_hash: [0u8; 32],
            }],
            chunk_refs: vec![ChunkReference {
                hash: extra_hash,
                version_id: VersionId(Uuid::new_v4()),
                file_path: "/test/file.txt".to_string(),
                offset: 0,
            }],
            hashes: CompatibilityHashes {
                manifest_hash: [0u8; 32],
                file_index_hash: [0u8; 32],
                chunk_index_hash: [0u8; 32],
            },
        };
        repo.write_compat_manifest("v1", &manifest).unwrap();

        let report = repo.self_check().unwrap();
        assert!(!report.is_clean());
        assert!(
            report.finding_count(&DamageType::RepoInconsistent) >= 1,
            "should detect chunk set mismatch"
        );
        assert!(
            report.finding_count(&DamageType::ChunkMissing) >= 1,
            "should detect extra_hash as missing chunk"
        );
    }

    #[test]
    fn test_self_check_empty_repo() {
        let tmp = tempfile::tempdir().unwrap();
        let repo = CompatibleRepository::init(tmp.path(), "empty-repo".to_string()).unwrap();
        let report = repo.self_check().unwrap();
        assert!(report.is_clean());
        assert_eq!(report.total_manifests_scanned, 0);
        assert_eq!(report.total_chunks_scanned, 0);
    }

    #[test]
    fn test_integrity_report_serde() {
        let report = CompatRepoIntegrityReport {
            repository_id: "test".to_string(),
            checked_at: Utc::now(),
            total_chunks_scanned: 10,
            total_manifests_scanned: 2,
            findings: vec![DamageFinding {
                damage_type: DamageType::ChunkMissing,
                location: DamageLocation {
                    bucket: Some("ab".to_string()),
                    path: "test.dblock".to_string(),
                    version_id: Some("v1".to_string()),
                },
                description: "test finding".to_string(),
                affected_versions: vec!["v1".to_string()],
            }],
        };
        let json = serde_json::to_string(&report).unwrap();
        let de: CompatRepoIntegrityReport = serde_json::from_str(&json).unwrap();
        assert_eq!(de.repository_id, "test");
        assert_eq!(de.findings.len(), 1);
        assert!(!de.is_clean());
    }
}