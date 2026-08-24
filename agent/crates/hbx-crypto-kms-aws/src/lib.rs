use hbx_core::domain::encryption::EncryptionProfile;
use hbx_core::pipeline::{IKeySource, KeyError, ZeroizingKey};

pub struct AwsKmsSource {
    endpoint: Option<String>,
    access_key_id: zeroize::Zeroizing<Option<std::string::String>>,
    secret_access_key: zeroize::Zeroizing<Option<std::string::String>>,
    region: Option<String>,
}

impl AwsKmsSource {
    pub fn new(
        endpoint: String,
        access_key_id: String,
        secret_access_key: String,
        region: String,
    ) -> Self {
        Self {
            endpoint: Some(endpoint),
            access_key_id: zeroize::Zeroizing::new(Some(access_key_id)),
            secret_access_key: zeroize::Zeroizing::new(Some(secret_access_key)),
            region: Some(region),
        }
    }

    pub fn unconfigured() -> Self {
        Self {
            endpoint: None,
            access_key_id: zeroize::Zeroizing::new(None),
            secret_access_key: zeroize::Zeroizing::new(None),
            region: None,
        }
    }
}

impl IKeySource for AwsKmsSource {
    fn acquire_key(&self, profile: &EncryptionProfile) -> Result<ZeroizingKey, KeyError> {
        let endpoint = self
            .endpoint
            .as_ref()
            .ok_or(KeyError::Unavailable("KMS endpoint not configured".to_string()))?;

        let _ = (
            endpoint,
            profile,
            self.access_key_id.as_ref(),
            self.secret_access_key.as_ref(),
            self.region.as_ref(),
        );
        Err(KeyError::Unavailable(
            "AWS KMS TLS handshake not yet implemented".to_string(),
        ))
    }

    fn release_key(&self, _key: ZeroizingKey) {}
}
