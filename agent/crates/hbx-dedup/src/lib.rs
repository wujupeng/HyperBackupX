use std::collections::HashMap;

use hbx_core::domain::chunk::{ChunkHash, ChunkLocation, ChunkReference};
use hbx_core::pipeline::{DedupLookupResult, IDedupIndex, IndexError};
use parking_lot::RwLock;
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HashAlgorithm {
    Blake3,
    Sha256,
}

pub fn compute_hash(data: &[u8], algorithm: HashAlgorithm) -> ChunkHash {
    match algorithm {
        HashAlgorithm::Blake3 => {
            let hash = blake3::hash(data);
            ChunkHash(*hash.as_bytes())
        }
        HashAlgorithm::Sha256 => {
            let mut hasher = Sha256::new();
            hasher.update(data);
            let result = hasher.finalize();
            let mut arr = [0u8; 32];
            arr.copy_from_slice(&result);
            ChunkHash(arr)
        }
    }
}

struct IndexEntry {
    location: ChunkLocation,
    reference_count: u64,
}

pub struct LocalDedupIndex {
    index: RwLock<HashMap<ChunkHash, IndexEntry>>,
    default_algorithm: HashAlgorithm,
}

impl LocalDedupIndex {
    pub fn new() -> Self {
        Self {
            index: RwLock::new(HashMap::new()),
            default_algorithm: HashAlgorithm::Blake3,
        }
    }

    pub fn with_algorithm(algorithm: HashAlgorithm) -> Self {
        Self {
            index: RwLock::new(HashMap::new()),
            default_algorithm: algorithm,
        }
    }

    pub fn algorithm(&self) -> HashAlgorithm {
        self.default_algorithm
    }

    pub fn compute_hash(&self, data: &[u8]) -> ChunkHash {
        compute_hash(data, self.default_algorithm)
    }
}

impl Default for LocalDedupIndex {
    fn default() -> Self {
        Self::new()
    }
}

impl IDedupIndex for LocalDedupIndex {
    fn batch_lookup(
        &self,
        hashes: &[ChunkHash],
    ) -> Result<Vec<DedupLookupResult>, IndexError> {
        let index = self.index.read();
        let results = hashes
            .iter()
            .map(|hash| {
                match index.get(hash) {
                    Some(entry) => DedupLookupResult {
                        hash: hash.clone(),
                        exists: true,
                        reference_count: entry.reference_count,
                        location: Some(entry.location.clone()),
                    },
                    None => DedupLookupResult {
                        hash: hash.clone(),
                        exists: false,
                        reference_count: 0,
                        location: None,
                    },
                }
            })
            .collect();
        Ok(results)
    }

    fn register_new(
        &self,
        hash: &ChunkHash,
        location: &ChunkLocation,
    ) -> Result<(), IndexError> {
        let mut index = self.index.write();
        index.insert(
            hash.clone(),
            IndexEntry {
                location: location.clone(),
                reference_count: 0,
            },
        );
        Ok(())
    }

    fn add_references(
        &self,
        references: &[ChunkReference],
    ) -> Result<(), IndexError> {
        let mut index = self.index.write();
        for reference in references {
            if let Some(entry) = index.get_mut(&reference.hash) {
                entry.reference_count += 1;
            } else {
                tracing::warn!(
                    "add_references: chunk not registered: {:?}",
                    reference.hash
                );
            }
        }
        Ok(())
    }

    fn remove_references(
        &self,
        references: &[ChunkReference],
    ) -> Result<Vec<ChunkHash>, IndexError> {
        let mut index = self.index.write();
        let mut orphaned = Vec::new();

        for reference in references {
            if let Some(entry) = index.get_mut(&reference.hash) {
                if entry.reference_count > 0 {
                    entry.reference_count -= 1;
                }
                if entry.reference_count == 0 {
                    orphaned.push(reference.hash.clone());
                }
            }
        }

        Ok(orphaned)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hbx_core::pipeline::IDedupIndex;

    #[test]
    fn test_blake3_hash_consistency() {
        let data = b"hello world";
        let h1 = compute_hash(data, HashAlgorithm::Blake3);
        let h2 = compute_hash(data, HashAlgorithm::Blake3);
        assert_eq!(h1, h2);
    }

    #[test]
    fn test_sha256_hash_consistency() {
        let data = b"hello world";
        let h1 = compute_hash(data, HashAlgorithm::Sha256);
        let h2 = compute_hash(data, HashAlgorithm::Sha256);
        assert_eq!(h1, h2);
    }

    #[test]
    fn test_different_data_different_hash() {
        let h1 = compute_hash(b"data1", HashAlgorithm::Blake3);
        let h2 = compute_hash(b"data2", HashAlgorithm::Blake3);
        assert_ne!(h1, h2);
    }

    #[test]
    fn test_batch_lookup_empty() {
        let index = LocalDedupIndex::new();
        let results = index.batch_lookup(&[]).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn test_register_and_lookup() {
        let index = LocalDedupIndex::new();
        let hash = compute_hash(b"test data", HashAlgorithm::Blake3);
        let location = ChunkLocation {
            bucket: "ab".to_string(),
            path: "ab/test.chunk".to_string(),
        };

        index.register_new(&hash, &location).unwrap();

        let results = index.batch_lookup(std::slice::from_ref(&hash)).unwrap();
        assert!(results[0].exists);
        assert_eq!(results[0].reference_count, 0);
        assert_eq!(results[0].location.as_ref().unwrap(), &location);
    }

    #[test]
    fn test_reference_counting() {
        let index = LocalDedupIndex::new();
        let hash = compute_hash(b"test data", HashAlgorithm::Blake3);
        let location = ChunkLocation {
            bucket: "ab".to_string(),
            path: "ab/test.chunk".to_string(),
        };

        index.register_new(&hash, &location).unwrap();

        let version_id = hbx_core::domain::common::VersionId(uuid::Uuid::new_v4());
        let refs = vec![ChunkReference {
            hash: hash.clone(),
            version_id,
            file_path: "/test/file.txt".to_string(),
            offset: 0,
        }];

        index.add_references(&refs).unwrap();

        let results = index.batch_lookup(std::slice::from_ref(&hash)).unwrap();
        assert_eq!(results[0].reference_count, 1);

        index.add_references(&refs).unwrap();
        let results = index.batch_lookup(std::slice::from_ref(&hash)).unwrap();
        assert_eq!(results[0].reference_count, 2);

        let orphaned = index.remove_references(&refs).unwrap();
        assert!(orphaned.is_empty());

        let results = index.batch_lookup(std::slice::from_ref(&hash)).unwrap();
        assert_eq!(results[0].reference_count, 1);

        let orphaned = index.remove_references(&refs).unwrap();
        assert_eq!(orphaned.len(), 1);
        assert_eq!(orphaned[0], hash);
    }
}

#[cfg(test)]
mod proptests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn prop_hash_deterministic_blake3(
            data in proptest::collection::vec(any::<u8>(), 0..8192)
        ) {
            let h1 = compute_hash(&data, HashAlgorithm::Blake3);
            let h2 = compute_hash(&data, HashAlgorithm::Blake3);
            prop_assert_eq!(h1, h2);
        }

        #[test]
        fn prop_hash_deterministic_sha256(
            data in proptest::collection::vec(any::<u8>(), 0..8192)
        ) {
            let h1 = compute_hash(&data, HashAlgorithm::Sha256);
            let h2 = compute_hash(&data, HashAlgorithm::Sha256);
            prop_assert_eq!(h1, h2);
        }

        #[test]
        fn prop_hash_different_data_different_hash(
            data1 in proptest::collection::vec(any::<u8>(), 1..1024),
            data2 in proptest::collection::vec(any::<u8>(), 1..1024)
        ) {
            if data1 != data2 {
                let h1 = compute_hash(&data1, HashAlgorithm::Blake3);
                let h2 = compute_hash(&data2, HashAlgorithm::Blake3);
                prop_assert_ne!(h1, h2);
            }
        }
    }
}
