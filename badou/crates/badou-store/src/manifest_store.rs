//! Manifest 持久化：长度前缀帧流式编码 + 原子写入。

use std::path::PathBuf;
use hbx_core::domain::common::RepositoryId;
use badou_engine::format::BadouDataLayout;
use badou_engine::domain::manifest::Manifest;
use badou_engine::domain::snapshot::FileEntry;
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum ManifestStoreError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("serde error: {0}")]
    Serde(#[from] serde_json::Error),
    #[error("manifest not found: {0}")]
    NotFound(Uuid),
    #[error("consistency check failed: chunk_list and file_tree mismatch")]
    Inconsistent,
    #[error("truncated frame")]
    Truncated,
}

pub struct ManifestStore {
    layout: BadouDataLayout,
}

impl ManifestStore {
    pub fn new(layout: BadouDataLayout) -> Self {
        Self { layout }
    }

    pub fn manifest_path(&self, repo_id: &RepositoryId, manifest_id: Uuid) -> PathBuf {
        self.layout.manifests_dir(repo_id).join(format!("{}.manifest", manifest_id))
    }

    pub fn write_manifest(
        &self,
        repo_id: &RepositoryId,
        manifest: &Manifest,
    ) -> Result<Uuid, ManifestStoreError> {
        let manifest_id = manifest.manifest_id;
        let final_path = self.manifest_path(repo_id, manifest_id);
        let staging_path = final_path.with_extension("manifest.staging");

        let encoded = encode_manifest(manifest)?;
        std::fs::write(&staging_path, &encoded)?;
        std::fs::rename(&staging_path, &final_path)?;

        Ok(manifest_id)
    }

    pub fn read_manifest(
        &self,
        repo_id: &RepositoryId,
        manifest_id: Uuid,
    ) -> Result<Manifest, ManifestStoreError> {
        let path = self.manifest_path(repo_id, manifest_id);
        if !path.exists() {
            return Err(ManifestStoreError::NotFound(manifest_id));
        }
        let data = std::fs::read(&path)?;
        let manifest = decode_manifest(&data)?;
        Ok(manifest)
    }

    pub fn manifest_exists(&self, repo_id: &RepositoryId, manifest_id: Uuid) -> bool {
        self.manifest_path(repo_id, manifest_id).exists()
    }

    pub fn compute_hash(&self, manifest: &Manifest) -> String {
        let encoded = encode_manifest(manifest).unwrap_or_default();
        let h = blake3::hash(&encoded);
        hex::encode(h.as_bytes())
    }

    pub fn verify_consistency(
        &self,
        manifest: &Manifest,
        file_entries: &[FileEntry],
    ) -> Result<(), ManifestStoreError> {
        let chunk_set: std::collections::HashSet<String> = manifest.chunk_refs.iter()
            .map(|r| hex::encode(r.chunk_hash.0))
            .collect();
        let file_chunk_set: std::collections::HashSet<String> = file_entries.iter()
            .flat_map(|e| e.chunk_hashes.iter())
            .map(|h| hex::encode(h.0))
            .collect();
        if chunk_set != file_chunk_set {
            return Err(ManifestStoreError::Inconsistent);
        }
        Ok(())
    }
}

fn encode_manifest(manifest: &Manifest) -> Result<Vec<u8>, ManifestStoreError> {
    let json = serde_json::to_vec(manifest)?;
    let mut buf = Vec::with_capacity(4 + json.len());
    let len = json.len() as u32;
    buf.extend_from_slice(&len.to_le_bytes());
    buf.extend_from_slice(&json);
    Ok(buf)
}

fn decode_manifest(data: &[u8]) -> Result<Manifest, ManifestStoreError> {
    if data.len() < 4 {
        return Err(ManifestStoreError::Truncated);
    }
    let len = u32::from_le_bytes([data[0], data[1], data[2], data[3]]) as usize;
    if data.len() < 4 + len {
        return Err(ManifestStoreError::Truncated);
    }
    let manifest: Manifest = serde_json::from_slice(&data[4..4 + len])?;
    Ok(manifest)
}

#[cfg(test)]
mod tests {
    use super::*;
    use badou_engine::domain::manifest::ChunkRef;
    use hbx_core::domain::chunk::ChunkHash;

    fn make_layout() -> (BadouDataLayout, RepositoryId) {
        let tmp = tempfile::tempdir().unwrap();
        let layout = BadouDataLayout::new(tmp.path());
        let repo_id = RepositoryId(Uuid::new_v4());
        std::fs::create_dir_all(layout.manifests_dir(&repo_id)).unwrap();
        std::mem::forget(tmp);
        (layout, repo_id)
    }

    fn make_manifest() -> Manifest {
        let chunk_ref = ChunkRef {
            chunk_hash: ChunkHash([0xab; 32]),
            offset: 0,
            size: 1024,
        };
        Manifest::new(Uuid::new_v4(), vec![1, 2, 3], vec![chunk_ref])
    }

    #[test]
    fn write_and_read_manifest() {
        let (layout, repo_id) = make_layout();
        let store = ManifestStore::new(layout);
        let manifest = make_manifest();
        let id = store.write_manifest(&repo_id, &manifest).unwrap();
        let read_back = store.read_manifest(&repo_id, id).unwrap();
        assert_eq!(read_back.manifest_id, manifest.manifest_id);
        assert_eq!(read_back.chunk_refs.len(), 1);
    }

    #[test]
    fn manifest_exists_after_write() {
        let (layout, repo_id) = make_layout();
        let store = ManifestStore::new(layout);
        let manifest = make_manifest();
        let id = store.write_manifest(&repo_id, &manifest).unwrap();
        assert!(store.manifest_exists(&repo_id, id));
    }

    #[test]
    fn read_nonexistent_fails() {
        let (layout, repo_id) = make_layout();
        let store = ManifestStore::new(layout);
        let result = store.read_manifest(&repo_id, Uuid::new_v4());
        assert!(result.is_err());
    }

    #[test]
    fn consistency_check_passes() {
        let (layout, _repo_id) = make_layout();
        let store = ManifestStore::new(layout);
        let hash = ChunkHash([0xab; 32]);
        let chunk_ref = ChunkRef { chunk_hash: hash.clone(), offset: 0, size: 1024 };
        let manifest = Manifest::new(Uuid::new_v4(), vec![], vec![chunk_ref]);
        let file_entries = vec![FileEntry {
            path: "/test/file".to_string(),
            size: 1024,
            is_directory: false,
            chunk_hashes: vec![hash],
        }];
        assert!(store.verify_consistency(&manifest, &file_entries).is_ok());
    }

    #[test]
    fn consistency_check_fails_on_mismatch() {
        let (layout, _repo_id) = make_layout();
        let store = ManifestStore::new(layout);
        let hash1 = ChunkHash([0xab; 32]);
        let hash2 = ChunkHash([0xcd; 32]);
        let chunk_ref = ChunkRef { chunk_hash: hash1, offset: 0, size: 1024 };
        let manifest = Manifest::new(Uuid::new_v4(), vec![], vec![chunk_ref]);
        let file_entries = vec![FileEntry {
            path: "/test/file".to_string(),
            size: 1024,
            is_directory: false,
            chunk_hashes: vec![hash2],
        }];
        assert!(store.verify_consistency(&manifest, &file_entries).is_err());
    }

    #[test]
    fn compute_hash_deterministic() {
        let (layout, _repo_id) = make_layout();
        let store = ManifestStore::new(layout);
        let manifest = make_manifest();
        let h1 = store.compute_hash(&manifest);
        let h2 = store.compute_hash(&manifest);
        assert_eq!(h1, h2);
    }
}