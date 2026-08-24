use std::fs;
use std::path::{Path, PathBuf};

use hbx_core::domain::common::RepositoryId;
use hbx_core::domain::repository::BackendType;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

const FORMAT_VERSION: u32 = 1;

#[derive(Serialize, Deserialize)]
pub struct RepositoryMeta {
    pub format_version: u32,
    pub repository_id: Uuid,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub backend_type: String,
}

#[derive(Serialize, Deserialize)]
pub struct RepoConfig {
    pub bucket_strategy: String,
    pub bucket_count: u32,
}

pub struct RepositoryInitializer {
    root: PathBuf,
}

impl RepositoryInitializer {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn init(
        &self,
        repository_id: RepositoryId,
        backend_type: BackendType,
    ) -> Result<(), std::io::Error> {
        let root = &self.root;
        fs::create_dir_all(root)?;

        let dirs = [
            "config",
            "manifests",
            "manifests/.staging",
            "index",
            "index/.staging",
            "chunks",
            "volumes",
            "locks",
            ".hbx",
        ];
        for dir in &dirs {
            fs::create_dir_all(root.join(dir))?;
        }

        for i in 0u8..=255 {
            let bucket = format!("{:02x}", i);
            fs::create_dir_all(root.join("chunks").join(&bucket))?;
        }

        let meta = RepositoryMeta {
            format_version: FORMAT_VERSION,
            repository_id: repository_id.0,
            created_at: chrono::Utc::now(),
            backend_type: format!("{:?}", backend_type),
        };
        fs::write(
            root.join("repository.json"),
            serde_json::to_vec_pretty(&meta)?,
        )?;

        let config = RepoConfig {
            bucket_strategy: "hash_prefix_2".to_string(),
            bucket_count: 256,
        };
        fs::write(
            root.join("config").join("repo_config.json"),
            serde_json::to_vec_pretty(&config)?,
        )?;

        fs::write(
            root.join(".hbx").join("format_version"),
            FORMAT_VERSION.to_string(),
        )?;

        fs::write(
            root.join("volumes").join("volume_registry.json"),
            serde_json::to_vec_pretty(&serde_json::json!({"volumes": []}))?,
        )?;

        Ok(())
    }

    pub fn format_version(&self) -> Result<u32, std::io::Error> {
        let content = fs::read_to_string(self.root.join(".hbx").join("format_version"))?;
        content.trim().parse().map_err(|e| {
            std::io::Error::new(std::io::ErrorKind::InvalidData, format!("{}", e))
        })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }
}

pub fn bucket_name(hash: &hbx_core::domain::chunk::ChunkHash) -> String {
    format!("{:02x}", hash.0[0])
}

pub fn chunk_filename(hash: &hbx_core::domain::chunk::ChunkHash) -> String {
    hex::encode(hash.0) + ".chunk"
}

pub fn manifest_filename(version_id: &hbx_core::domain::common::VersionId) -> String {
    format!("{}.manifest", version_id.0)
}

pub fn encode_encrypted_chunk(encrypted: &hbx_core::domain::encryption::EncryptedChunk) -> Vec<u8> {
    let mut data = Vec::with_capacity(12 + 16 + encrypted.ciphertext.len());
    data.extend_from_slice(&encrypted.nonce);
    data.extend_from_slice(&encrypted.auth_tag);
    data.extend_from_slice(&encrypted.ciphertext);
    data
}

pub fn decode_encrypted_chunk(data: &[u8]) -> Result<hbx_core::domain::encryption::EncryptedChunk, std::io::Error> {
    if data.len() < 12 + 16 {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "chunk data too short",
        ));
    }
    let mut nonce = [0u8; 12];
    let mut auth_tag = [0u8; 16];
    nonce.copy_from_slice(&data[..12]);
    auth_tag.copy_from_slice(&data[12..28]);
    let ciphertext = data[28..].to_vec();
    Ok(hbx_core::domain::encryption::EncryptedChunk {
        ciphertext,
        nonce,
        auth_tag,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_init_repository() {
        let tmp = tempfile::tempdir().unwrap();
        let init = RepositoryInitializer::new(tmp.path());
        let repo_id = RepositoryId(Uuid::new_v4());
        init.init(repo_id, BackendType::Local).unwrap();

        assert!(tmp.path().join("repository.json").exists());
        assert!(tmp.path().join("config").join("repo_config.json").exists());
        assert!(tmp.path().join("manifests").exists());
        assert!(tmp.path().join("index").exists());
        assert!(tmp.path().join("chunks").exists());
        assert!(tmp.path().join("volumes").join("volume_registry.json").exists());
        assert!(tmp.path().join(".hbx").join("format_version").exists());

        for i in 0u8..=255 {
            let bucket = format!("{:02x}", i);
            assert!(tmp.path().join("chunks").join(&bucket).exists());
        }

        let version = init.format_version().unwrap();
        assert_eq!(version, 1);
    }

    #[test]
    fn test_bucket_name() {
        let hash = hbx_core::domain::chunk::ChunkHash([0xab, 0xcd, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8]);
        assert_eq!(bucket_name(&hash), "ab");
    }

    #[test]
    fn test_encode_decode_chunk() {
        let encrypted = hbx_core::domain::encryption::EncryptedChunk {
            ciphertext: vec![1, 2, 3, 4, 5],
            nonce: [0u8; 12],
            auth_tag: [0xff; 16],
        };
        let encoded = encode_encrypted_chunk(&encrypted);
        let decoded = decode_encrypted_chunk(&encoded).unwrap();
        assert_eq!(decoded.ciphertext, encrypted.ciphertext);
        assert_eq!(decoded.nonce, encrypted.nonce);
        assert_eq!(decoded.auth_tag, encrypted.auth_tag);
    }
}