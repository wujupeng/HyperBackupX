use std::path::PathBuf;

use hbx_core::domain::encryption::EncryptionProfile;
use hbx_core::pipeline::{IKeySource, KeyError, ZeroizingKey};

pub struct KeyFileSource {
    key_file_path: PathBuf,
}

impl KeyFileSource {
    pub fn new(path: PathBuf) -> Self {
        Self {
            key_file_path: path,
        }
    }
}

impl IKeySource for KeyFileSource {
    fn acquire_key(&self, _profile: &EncryptionProfile) -> Result<ZeroizingKey, KeyError> {
        let mut file = std::fs::File::open(&self.key_file_path)
            .map_err(|e| KeyError::Unavailable(format!("failed to open key file: {e}")))?;

        let metadata = file
            .metadata()
            .map_err(|e| KeyError::Unavailable(format!("failed to read key file metadata: {e}")))?;

        let mut buf = vec![0u8; metadata.len() as usize];
        use std::io::Read;
        file.read_exact(&mut buf)
            .map_err(|e| KeyError::Unavailable(format!("failed to read key file: {e}")))?;

        Ok(ZeroizingKey(buf))
    }

    fn release_key(&self, _key: ZeroizingKey) {}
}

#[cfg(test)]
mod tests {
    use super::*;
    use hbx_core::domain::common::{Base64Bytes, ProfileId};
    use hbx_core::domain::encryption::{
        EncryptionAlgorithm, EncryptionProfile, KdfAlgorithm, KeySource,
    };
    use uuid::Uuid;

    fn make_profile() -> EncryptionProfile {
        EncryptionProfile {
            profile_id: ProfileId(Uuid::new_v4()),
            algorithm: EncryptionAlgorithm::Aes256Gcm,
            kdf: KdfAlgorithm::Argon2id,
            key_source: KeySource::KeyFile,
            key_reference: "test".to_string(),
            salt: Base64Bytes(b"test_salt_16bytes".to_vec()),
        }
    }

    #[test]
    fn test_key_file_source_acquire() {
        let dir = tempfile::tempdir().unwrap();
        let key_path = dir.path().join("test.key");
        let key_data = vec![0xABu8; 32];
        std::fs::write(&key_path, &key_data).unwrap();

        let source = KeyFileSource::new(key_path);
        let profile = make_profile();
        let key = source.acquire_key(&profile).unwrap();
        assert_eq!(key.0, key_data);
    }

    #[test]
    fn test_key_file_source_missing_file() {
        let source = KeyFileSource::new(PathBuf::from("/nonexistent/path/key.file"));
        let profile = make_profile();
        let result = source.acquire_key(&profile);
        assert!(result.is_err());
    }
}
