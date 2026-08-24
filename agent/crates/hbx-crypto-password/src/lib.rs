use argon2::password_hash::SaltString;
use argon2::{Algorithm, Argon2, Params, PasswordHasher, Version};
use hbx_core::domain::encryption::EncryptionProfile;
use hbx_core::pipeline::{IKeySource, KeyError, ZeroizingKey};

pub struct PasswordKeySource {
    password: zeroize::Zeroizing<std::string::String>,
}

impl PasswordKeySource {
    pub fn new(password: String) -> Self {
        Self {
            password: zeroize::Zeroizing::new(password),
        }
    }
}

impl IKeySource for PasswordKeySource {
    fn acquire_key(&self, profile: &EncryptionProfile) -> Result<ZeroizingKey, KeyError> {
        let salt = &profile.salt.0;
        let salt_str =
            SaltString::encode_b64(salt).map_err(|_| KeyError::Unavailable("invalid salt".to_string()))?;

        let params = Params::new(8, 1, 1, None)
            .map_err(|_| KeyError::Unavailable("invalid Argon2 params".to_string()))?;
        let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);

        let hash = argon2
            .hash_password(self.password.as_bytes(), &salt_str)
            .map_err(|_| KeyError::InvalidCredential)?;

        let raw = hash
            .hash
            .ok_or(KeyError::Unavailable("no hash output".to_string()))?;

        Ok(ZeroizingKey(raw.as_bytes().to_vec()))
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
            key_source: KeySource::Password,
            key_reference: "test".to_string(),
            salt: Base64Bytes(b"test_salt_16bytes".to_vec()),
        }
    }

    #[test]
    fn test_password_key_source_acquire() {
        let source = PasswordKeySource::new("mypassword".to_string());
        let profile = make_profile();
        let key = source.acquire_key(&profile).unwrap();
        assert!(!key.0.is_empty());
    }

    #[test]
    fn test_password_key_source_consistency() {
        let source = PasswordKeySource::new("mypassword".to_string());
        let profile = make_profile();
        let key1 = source.acquire_key(&profile).unwrap();
        let key2 = source.acquire_key(&profile).unwrap();
        assert_eq!(key1.0, key2.0);
    }

    #[test]
    fn test_different_passwords_different_keys() {
        let source1 = PasswordKeySource::new("password1".to_string());
        let source2 = PasswordKeySource::new("password2".to_string());
        let profile = make_profile();
        let key1 = source1.acquire_key(&profile).unwrap();
        let key2 = source2.acquire_key(&profile).unwrap();
        assert_ne!(key1.0, key2.0);
    }
}
