use hbx_core::domain::encryption::EncryptionProfile;
use hbx_core::pipeline::{IKeySource, KeyError, ZeroizingKey};

pub struct AzureKeyVaultSource {
    endpoint: Option<String>,
    tenant_id: zeroize::Zeroizing<Option<std::string::String>>,
    client_id: zeroize::Zeroizing<Option<std::string::String>>,
    client_secret: zeroize::Zeroizing<Option<std::string::String>>,
}

impl AzureKeyVaultSource {
    pub fn new(
        endpoint: String,
        tenant_id: String,
        client_id: String,
        client_secret: String,
    ) -> Self {
        Self {
            endpoint: Some(endpoint),
            tenant_id: zeroize::Zeroizing::new(Some(tenant_id)),
            client_id: zeroize::Zeroizing::new(Some(client_id)),
            client_secret: zeroize::Zeroizing::new(Some(client_secret)),
        }
    }

    pub fn unconfigured() -> Self {
        Self {
            endpoint: None,
            tenant_id: zeroize::Zeroizing::new(None),
            client_id: zeroize::Zeroizing::new(None),
            client_secret: zeroize::Zeroizing::new(None),
        }
    }
}

impl IKeySource for AzureKeyVaultSource {
    fn acquire_key(&self, profile: &EncryptionProfile) -> Result<ZeroizingKey, KeyError> {
        let endpoint = self
            .endpoint
            .as_ref()
            .ok_or(KeyError::Unavailable("Key Vault endpoint not configured".to_string()))?;

        let _ = (
            endpoint,
            profile,
            self.tenant_id.as_ref(),
            self.client_id.as_ref(),
            self.client_secret.as_ref(),
        );
        Err(KeyError::Unavailable(
            "Azure Key Vault TLS handshake not yet implemented".to_string(),
        ))
    }

    fn release_key(&self, _key: ZeroizingKey) {}
}
