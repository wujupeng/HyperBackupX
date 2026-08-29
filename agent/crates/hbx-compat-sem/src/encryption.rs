use hbx_core::domain::encryption::{EncryptionAlgorithm, EncryptionProfile, KdfAlgorithm, KeySource};
use hbx_core::domain::common::{Base64Bytes, ProfileId};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::SemanticError;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DuplicatiEncryptionConfig {
    pub algorithm: String,
    pub passphrase: String,
}

pub fn align_encryption(
    config: &DuplicatiEncryptionConfig,
) -> Result<EncryptionProfile, SemanticError> {
    if config.passphrase.is_empty() {
        return Err(SemanticError::UnsupportedConfig("empty passphrase".to_string()));
    }

    let _algorithm = match config.algorithm.to_lowercase().as_str() {
        "aes-256" | "aes-256-gcm" | "aes-gcm" => EncryptionAlgorithm::Aes256Gcm,
        "aes-128" | "aes-128-cbc" | "aes-cbc" => EncryptionAlgorithm::Aes256Gcm,
        "none" => {
            return Err(SemanticError::UnsupportedConfig(
                "encryption disabled in Duplicati config, cannot map to HBX (encryption mandatory)".to_string(),
            ));
        }
        other => {
            return Err(SemanticError::UnsupportedConfig(format!(
                "unknown encryption algorithm: {other}, mapping to AES-256-GCM"
            )))
        }
    };

    let salt = vec![0u8; 16];

    Ok(EncryptionProfile {
        profile_id: ProfileId(Uuid::new_v4()),
        algorithm: EncryptionAlgorithm::Aes256Gcm,
        kdf: KdfAlgorithm::Argon2id,
        key_source: KeySource::Password,
        key_reference: config.passphrase.clone(),
        salt: Base64Bytes(salt),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_aes256_mapping() {
        let config = DuplicatiEncryptionConfig {
            algorithm: "aes-256".to_string(),
            passphrase: "secret".to_string(),
        };
        let result = align_encryption(&config).unwrap();
        assert_eq!(result.algorithm, EncryptionAlgorithm::Aes256Gcm);
        assert_eq!(result.kdf, KdfAlgorithm::Argon2id);
        assert_eq!(result.key_source, KeySource::Password);
    }

    #[test]
    fn test_aes128_upgraded_to_aes256() {
        let config = DuplicatiEncryptionConfig {
            algorithm: "aes-128".to_string(),
            passphrase: "secret".to_string(),
        };
        let result = align_encryption(&config).unwrap();
        assert_eq!(result.algorithm, EncryptionAlgorithm::Aes256Gcm);
    }

    #[test]
    fn test_empty_passphrase_rejected() {
        let config = DuplicatiEncryptionConfig {
            algorithm: "aes-256".to_string(),
            passphrase: "".to_string(),
        };
        let result = align_encryption(&config);
        assert!(result.is_err());
    }

    #[test]
    fn test_none_encryption_rejected() {
        let config = DuplicatiEncryptionConfig {
            algorithm: "none".to_string(),
            passphrase: "secret".to_string(),
        };
        let result = align_encryption(&config);
        assert!(result.is_err());
    }

    #[test]
    fn test_unknown_algorithm_rejected() {
        let config = DuplicatiEncryptionConfig {
            algorithm: "blowfish".to_string(),
            passphrase: "secret".to_string(),
        };
        let result = align_encryption(&config);
        assert!(result.is_err());
    }

    #[test]
    fn test_key_reference_preserves_passphrase() {
        let config = DuplicatiEncryptionConfig {
            algorithm: "aes-256-gcm".to_string(),
            passphrase: "my_secret_pass".to_string(),
        };
        let result = align_encryption(&config).unwrap();
        assert_eq!(result.key_reference, "my_secret_pass");
    }
}