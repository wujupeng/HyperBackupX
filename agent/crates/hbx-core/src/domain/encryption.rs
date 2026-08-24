use serde::{Deserialize, Serialize};

use super::common::{Base64Bytes, ProfileId};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptionProfile {
    pub profile_id: ProfileId,
    pub algorithm: EncryptionAlgorithm,
    pub kdf: KdfAlgorithm,
    pub key_source: KeySource,
    pub key_reference: String,
    pub salt: Base64Bytes,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EncryptionAlgorithm {
    Aes256Gcm,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum KdfAlgorithm {
    Argon2id,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum KeySource {
    Password,
    KeyFile,
    EnterpriseKms,
    HuaweiCloudKms,
    AzureKeyVault,
    AwsKms,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptedChunk {
    pub ciphertext: Vec<u8>,
    pub nonce: [u8; 12],
    pub auth_tag: [u8; 16],
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DerivedKey(pub Vec<u8>);