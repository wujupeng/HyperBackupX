use hbx_core::domain::encryption::EncryptionProfile;
use hbx_core::pipeline::{IKeySource, KeyError, ZeroizingKey};

pub struct EnterpriseKmsSource {
    endpoint: Option<String>,
    api_key: zeroize::Zeroizing<Option<std::string::String>>,
}

impl EnterpriseKmsSource {
    pub fn new(endpoint: String, api_key: String) -> Self {
        Self {
            endpoint: Some(endpoint),
            api_key: zeroize::Zeroizing::new(Some(api_key)),
        }
    }

    pub fn unconfigured() -> Self {
        Self {
            endpoint: None,
            api_key: zeroize::Zeroizing::new(None),
        }
    }
}

impl IKeySource for EnterpriseKmsSource {
    fn acquire_key(&self, profile: &EncryptionProfile) -> Result<ZeroizingKey, KeyError> {
        let endpoint = self
            .endpoint
            .as_ref()
            .ok_or(KeyError::Unavailable("KMS endpoint not configured".to_string()))?;

        let _ = (endpoint, profile, self.api_key.as_ref());
        Err(KeyError::Unavailable(
            "enterprise KMS TLS handshake not yet implemented".to_string(),
        ))
    }

    fn release_key(&self, _key: ZeroizingKey) {}
}
