use hbx_core::domain::chunk::ChunkId;
use hbx_core::domain::encryption::{DerivedKey, EncryptedChunk, EncryptionProfile};
use hbx_core::pipeline::{EncryptError, IEncryptionProvider};

pub struct NoOpEncryptionProvider;

impl IEncryptionProvider for NoOpEncryptionProvider {
    fn encrypt_chunk(
        &self,
        plain: &[u8],
        _chunk_id: &ChunkId,
    ) -> Result<EncryptedChunk, EncryptError> {
        Ok(EncryptedChunk {
            ciphertext: plain.to_vec(),
            nonce: [0u8; 12],
            auth_tag: [0u8; 16],
        })
    }

    fn decrypt_chunk(&self, encrypted: &EncryptedChunk) -> Result<Vec<u8>, EncryptError> {
        Ok(encrypted.ciphertext.clone())
    }

    fn derive_key(
        &self,
        _password: &str,
        _salt: &[u8],
        _profile: &EncryptionProfile,
    ) -> Result<DerivedKey, EncryptError> {
        Ok(DerivedKey(vec![0u8; 32]))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hbx_core::domain::chunk::ChunkId;
    use uuid::Uuid;

    #[test]
    fn test_noop_roundtrip() {
        let provider = NoOpEncryptionProvider;
        let chunk_id = ChunkId(Uuid::new_v4());
        let data = b"hello world";
        let encrypted = provider.encrypt_chunk(data, &chunk_id).unwrap();
        let decrypted = provider.decrypt_chunk(&encrypted).unwrap();
        assert_eq!(decrypted, data);
    }
}