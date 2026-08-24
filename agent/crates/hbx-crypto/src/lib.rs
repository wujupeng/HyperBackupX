use aes_gcm::aead::{AeadInPlace, KeyInit};
use aes_gcm::{Aes256Gcm, Key, Nonce};
use argon2::password_hash::SaltString;
use argon2::{Algorithm, Argon2, Params, PasswordHasher, Version};
use hbx_core::domain::chunk::ChunkId;
use hbx_core::domain::device::HardwareTier;
use hbx_core::domain::encryption::{DerivedKey, EncryptedChunk, EncryptionProfile};
use hbx_core::pipeline::{EncryptError, IEncryptionProvider, ZeroizingKey};

const KEY_LEN: usize = 32;
const NONCE_LEN: usize = 12;
const AUTH_TAG_LEN: usize = 16;

#[derive(Debug, Clone, Copy)]
pub struct Argon2Params {
    m_cost: u32,
    t_cost: u32,
    p_cost: u32,
}

impl Argon2Params {
    pub fn from_tier(tier: HardwareTier) -> Self {
        match tier {
            HardwareTier::Legacy => Self {
                m_cost: 65536,
                t_cost: 2,
                p_cost: 1,
            },
            HardwareTier::Standard => Self {
                m_cost: 262144,
                t_cost: 3,
                p_cost: 2,
            },
            HardwareTier::Modern => Self {
                m_cost: 1048576,
                t_cost: 3,
                p_cost: 4,
            },
        }
    }

    pub fn test_params() -> Self {
        Self {
            m_cost: 8,
            t_cost: 1,
            p_cost: 1,
        }
    }
}

impl Default for Argon2Params {
    fn default() -> Self {
        Self::from_tier(HardwareTier::Standard)
    }
}

fn build_argon2(params: Argon2Params) -> Argon2<'static> {
    let p = Params::new(params.m_cost, params.t_cost, params.p_cost, None)
        .expect("invalid Argon2 params");
    Argon2::new(Algorithm::Argon2id, Version::V0x13, p)
}

fn derive_nonce(chunk_id: &ChunkId) -> [u8; NONCE_LEN] {
    let uuid_bytes = chunk_id.0.as_bytes();
    let mut nonce = [0u8; NONCE_LEN];
    nonce.copy_from_slice(&uuid_bytes[..NONCE_LEN]);
    nonce
}

pub struct AesGcmEncryptionProvider {
    key: ZeroizingKey,
    argon2_params: Argon2Params,
}

impl AesGcmEncryptionProvider {
    pub fn new(key: Vec<u8>, tier: HardwareTier) -> Result<Self, EncryptError> {
        if key.len() != KEY_LEN {
            return Err(EncryptError::Failed(format!(
                "invalid key length: expected {KEY_LEN}, got {}",
                key.len()
            )));
        }
        Ok(Self {
            key: ZeroizingKey(key),
            argon2_params: Argon2Params::from_tier(tier),
        })
    }

    pub fn from_password(
        password: &str,
        salt: &[u8],
        tier: HardwareTier,
    ) -> Result<Self, EncryptError> {
        let params = Argon2Params::from_tier(tier);
        let key = derive_key_with_params(password, salt, params)?;
        Ok(Self {
            key: ZeroizingKey(key),
            argon2_params: params,
        })
    }

    pub fn new_test(key: Vec<u8>) -> Result<Self, EncryptError> {
        if key.len() != KEY_LEN {
            return Err(EncryptError::Failed(format!(
                "invalid key length: expected {KEY_LEN}, got {}",
                key.len()
            )));
        }
        Ok(Self {
            key: ZeroizingKey(key),
            argon2_params: Argon2Params::test_params(),
        })
    }

    pub fn from_password_test(password: &str, salt: &[u8]) -> Result<Self, EncryptError> {
        let params = Argon2Params::test_params();
        let key = derive_key_with_params(password, salt, params)?;
        Ok(Self {
            key: ZeroizingKey(key),
            argon2_params: params,
        })
    }

    fn aes_cipher(&self) -> Aes256Gcm {
        let key = Key::<Aes256Gcm>::from_slice(&self.key.0);
        Aes256Gcm::new(key)
    }
}

fn derive_key_with_params(
    password: &str,
    salt: &[u8],
    params: Argon2Params,
) -> Result<Vec<u8>, EncryptError> {
    let argon2 = build_argon2(params);
    let salt = SaltString::encode_b64(salt).map_err(|e| EncryptError::Failed(e.to_string()))?;
    let hash = argon2
        .hash_password(password.as_bytes(), &salt)
        .map_err(|_| EncryptError::Failed("key derivation failed".to_string()))?;
    let raw = hash
        .hash
        .ok_or(EncryptError::Failed("no hash output".to_string()))?;
    Ok(raw.as_bytes().to_vec())
}

impl IEncryptionProvider for AesGcmEncryptionProvider {
    fn encrypt_chunk(
        &self,
        plain: &[u8],
        chunk_id: &ChunkId,
    ) -> Result<EncryptedChunk, EncryptError> {
        let cipher = self.aes_cipher();
        let nonce_bytes = derive_nonce(chunk_id);
        let nonce = Nonce::from_slice(&nonce_bytes);

        let mut buf = plain.to_vec();
        let tag = cipher
            .encrypt_in_place_detached(nonce, b"", &mut buf)
            .map_err(|_| EncryptError::Failed("encryption failed".to_string()))?;

        let mut auth_tag = [0u8; AUTH_TAG_LEN];
        auth_tag.copy_from_slice(tag.as_slice());

        Ok(EncryptedChunk {
            ciphertext: buf,
            nonce: nonce_bytes,
            auth_tag,
        })
    }

    fn decrypt_chunk(&self, encrypted: &EncryptedChunk) -> Result<Vec<u8>, EncryptError> {
        let cipher = self.aes_cipher();
        let nonce = Nonce::from_slice(&encrypted.nonce);
        let tag = aes_gcm::Tag::from_slice(&encrypted.auth_tag);

        let mut buf = encrypted.ciphertext.clone();
        cipher
            .decrypt_in_place_detached(nonce, b"", &mut buf, tag)
            .map_err(|_| EncryptError::AuthFailed)?;

        Ok(buf)
    }

    fn derive_key(
        &self,
        password: &str,
        salt: &[u8],
        _profile: &EncryptionProfile,
    ) -> Result<DerivedKey, EncryptError> {
        let key = derive_key_with_params(password, salt, self.argon2_params)?;
        Ok(DerivedKey(key))
    }
}

pub fn generate_salt() -> Vec<u8> {
    use rand::RngCore;
    let mut salt = vec![0u8; 16];
    rand::thread_rng().fill_bytes(&mut salt);
    salt
}

pub fn generate_random_key() -> Vec<u8> {
    use rand::RngCore;
    let mut key = vec![0u8; KEY_LEN];
    rand::thread_rng().fill_bytes(&mut key);
    key
}

pub struct KeySourceRegistry {
    sources:
        std::collections::HashMap<hbx_core::domain::encryption::KeySource, Box<dyn hbx_core::pipeline::IKeySource>>,
}

impl KeySourceRegistry {
    pub fn new() -> Self {
        Self {
            sources: std::collections::HashMap::new(),
        }
    }

    pub fn register(
        &mut self,
        source_type: hbx_core::domain::encryption::KeySource,
        source: Box<dyn hbx_core::pipeline::IKeySource>,
    ) {
        self.sources.insert(source_type, source);
    }

    pub fn get(
        &self,
        source_type: &hbx_core::domain::encryption::KeySource,
    ) -> Option<&dyn hbx_core::pipeline::IKeySource> {
        self.sources.get(source_type).map(|s| s.as_ref())
    }
}

impl Default for KeySourceRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hbx_core::domain::chunk::ChunkId;
    use uuid::Uuid;

    fn make_provider() -> AesGcmEncryptionProvider {
        AesGcmEncryptionProvider::new_test(vec![0x42u8; 32]).unwrap()
    }

    #[test]
    fn test_encrypt_decrypt_roundtrip() {
        let provider = make_provider();
        let chunk_id = ChunkId(Uuid::new_v4());
        let data = b"hello world, this is a test chunk";

        let encrypted = provider.encrypt_chunk(data, &chunk_id).unwrap();
        let decrypted = provider.decrypt_chunk(&encrypted).unwrap();
        assert_eq!(decrypted, data);
    }

    #[test]
    fn test_encrypt_decrypt_empty() {
        let provider = make_provider();
        let chunk_id = ChunkId(Uuid::new_v4());

        let encrypted = provider.encrypt_chunk(b"", &chunk_id).unwrap();
        let decrypted = provider.decrypt_chunk(&encrypted).unwrap();
        assert!(decrypted.is_empty());
    }

    #[test]
    fn test_encrypt_decrypt_large() {
        let provider = make_provider();
        let chunk_id = ChunkId(Uuid::new_v4());
        let data = vec![0xABu8; 1024 * 1024];

        let encrypted = provider.encrypt_chunk(&data, &chunk_id).unwrap();
        let decrypted = provider.decrypt_chunk(&encrypted).unwrap();
        assert_eq!(decrypted, data);
    }

    #[test]
    fn test_tamper_ciphertext_fails() {
        let provider = make_provider();
        let chunk_id = ChunkId(Uuid::new_v4());
        let data = b"sensitive backup data";

        let mut encrypted = provider.encrypt_chunk(data, &chunk_id).unwrap();
        encrypted.ciphertext[0] ^= 0xFF;

        let result = provider.decrypt_chunk(&encrypted);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), EncryptError::AuthFailed));
    }

    #[test]
    fn test_tamper_auth_tag_fails() {
        let provider = make_provider();
        let chunk_id = ChunkId(Uuid::new_v4());
        let data = b"sensitive backup data";

        let mut encrypted = provider.encrypt_chunk(data, &chunk_id).unwrap();
        encrypted.auth_tag[0] ^= 0xFF;

        let result = provider.decrypt_chunk(&encrypted);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), EncryptError::AuthFailed));
    }

    #[test]
    fn test_tamper_nonce_fails() {
        let provider = make_provider();
        let chunk_id = ChunkId(Uuid::new_v4());
        let data = b"sensitive backup data";

        let mut encrypted = provider.encrypt_chunk(data, &chunk_id).unwrap();
        encrypted.nonce[0] ^= 0xFF;

        let result = provider.decrypt_chunk(&encrypted);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), EncryptError::AuthFailed));
    }

    #[test]
    fn test_wrong_key_fails() {
        let provider1 = AesGcmEncryptionProvider::new_test(vec![0x42u8; 32]).unwrap();
        let provider2 = AesGcmEncryptionProvider::new_test(vec![0x00u8; 32]).unwrap();

        let chunk_id = ChunkId(Uuid::new_v4());
        let data = b"encrypted with key 1";

        let encrypted = provider1.encrypt_chunk(data, &chunk_id).unwrap();
        let result = provider2.decrypt_chunk(&encrypted);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), EncryptError::AuthFailed));
    }

    #[test]
    fn test_ciphertext_differs_from_plaintext() {
        let provider = make_provider();
        let chunk_id = ChunkId(Uuid::new_v4());
        let data = b"plaintext data that should be encrypted";

        let encrypted = provider.encrypt_chunk(data, &chunk_id).unwrap();
        assert_ne!(encrypted.ciphertext, data);
    }

    #[test]
    fn test_nonce_is_12_bytes() {
        let provider = make_provider();
        let chunk_id = ChunkId(Uuid::new_v4());

        let encrypted = provider.encrypt_chunk(b"data", &chunk_id).unwrap();
        assert_eq!(encrypted.nonce.len(), NONCE_LEN);
    }

    #[test]
    fn test_auth_tag_is_16_bytes() {
        let provider = make_provider();
        let chunk_id = ChunkId(Uuid::new_v4());

        let encrypted = provider.encrypt_chunk(b"data", &chunk_id).unwrap();
        assert_eq!(encrypted.auth_tag.len(), AUTH_TAG_LEN);
    }

    #[test]
    fn test_derive_key_consistency() {
        let provider =
            AesGcmEncryptionProvider::from_password_test("mypassword", b"mysalt1234567890").unwrap();
        let profile = EncryptionProfile {
            profile_id: hbx_core::domain::common::ProfileId(Uuid::new_v4()),
            algorithm: hbx_core::domain::encryption::EncryptionAlgorithm::Aes256Gcm,
            kdf: hbx_core::domain::encryption::KdfAlgorithm::Argon2id,
            key_source: hbx_core::domain::encryption::KeySource::Password,
            key_reference: "test".to_string(),
            salt: hbx_core::domain::common::Base64Bytes(b"mysalt1234567890".to_vec()),
        };

        let key1 = provider
            .derive_key("mypassword", b"mysalt1234567890", &profile)
            .unwrap();
        let key2 = provider
            .derive_key("mypassword", b"mysalt1234567890", &profile)
            .unwrap();
        assert_eq!(key1.0, key2.0);
    }

    #[test]
    fn test_derive_key_different_passwords() {
        let provider =
            AesGcmEncryptionProvider::from_password_test("password1", b"fixedsalt123456").unwrap();
        let profile = EncryptionProfile {
            profile_id: hbx_core::domain::common::ProfileId(Uuid::new_v4()),
            algorithm: hbx_core::domain::encryption::EncryptionAlgorithm::Aes256Gcm,
            kdf: hbx_core::domain::encryption::KdfAlgorithm::Argon2id,
            key_source: hbx_core::domain::encryption::KeySource::Password,
            key_reference: "test".to_string(),
            salt: hbx_core::domain::common::Base64Bytes(b"fixedsalt123456".to_vec()),
        };

        let key1 = provider
            .derive_key("password1", b"fixedsalt123456", &profile)
            .unwrap();
        let key2 = provider
            .derive_key("password2", b"fixedsalt123456", &profile)
            .unwrap();
        assert_ne!(key1.0, key2.0);
    }

    #[test]
    fn test_from_password_encrypt_decrypt() {
        let provider =
            AesGcmEncryptionProvider::from_password_test("strong_password", b"salt_16_bytes_!!")
                .unwrap();
        let chunk_id = ChunkId(Uuid::new_v4());
        let data = b"data encrypted with password-derived key";

        let encrypted = provider.encrypt_chunk(data, &chunk_id).unwrap();
        let decrypted = provider.decrypt_chunk(&encrypted).unwrap();
        assert_eq!(decrypted, data);
    }

    #[test]
    fn test_argon2_params_from_tier() {
        assert_eq!(Argon2Params::from_tier(HardwareTier::Legacy).m_cost, 65536);
        assert_eq!(
            Argon2Params::from_tier(HardwareTier::Standard).m_cost,
            262144
        );
        assert_eq!(
            Argon2Params::from_tier(HardwareTier::Modern).m_cost,
            1048576
        );
    }

    #[test]
    fn test_generate_salt_length() {
        let salt = generate_salt();
        assert_eq!(salt.len(), 16);
    }

    #[test]
    fn test_generate_random_key_length() {
        let key = generate_random_key();
        assert_eq!(key.len(), KEY_LEN);
    }
}

#[cfg(test)]
mod proptests {
    use super::*;
    use hbx_core::domain::chunk::ChunkId;
    use proptest::prelude::*;
    use uuid::Uuid;

    proptest! {
        #[test]
        fn prop_encrypt_decrypt_roundtrip(data in proptest::collection::vec(any::<u8>(), 0..4096)) {
            let provider = AesGcmEncryptionProvider::new_test(vec![0x42u8; 32]).unwrap();
            let chunk_id = ChunkId(Uuid::new_v4());
            let encrypted = provider.encrypt_chunk(&data, &chunk_id).unwrap();
            let decrypted = provider.decrypt_chunk(&encrypted).unwrap();
            prop_assert_eq!(decrypted, data);
        }

        #[test]
        fn prop_tamper_detection(
            data in proptest::collection::vec(any::<u8>(), 1..4096),
            flip_pos in 0usize..4096,
        ) {
            let provider = AesGcmEncryptionProvider::new_test(vec![0x42u8; 32]).unwrap();
            let chunk_id = ChunkId(Uuid::new_v4());
            let mut encrypted = provider.encrypt_chunk(&data, &chunk_id).unwrap();

            if flip_pos < encrypted.ciphertext.len() {
                encrypted.ciphertext[flip_pos] ^= 0x01;
                let result = provider.decrypt_chunk(&encrypted);
                prop_assert!(result.is_err());
            }
        }

        #[test]
        fn prop_ciphertext_ne_plaintext(
            data in proptest::collection::vec(any::<u8>(), 1..4096),
        ) {
            let provider = AesGcmEncryptionProvider::new_test(vec![0x42u8; 32]).unwrap();
            let chunk_id = ChunkId(Uuid::new_v4());
            let encrypted = provider.encrypt_chunk(&data, &chunk_id).unwrap();
            prop_assert_ne!(encrypted.ciphertext.as_slice(), data.as_slice());
        }
    }
}
