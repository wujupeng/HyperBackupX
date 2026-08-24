use hbx_core::domain::encryption::EncryptionProfile;
use hbx_core::pipeline::{IKeySource, KeyError, ZeroizingKey};

pub struct HuaweiCloudKmsSource {
    endpoint: Option<String>,
    ak: zeroize::Zeroizing<Option<std::string::String>>,
    sk: zeroize::Zeroizing<Option<std::string::String>>,
}

impl HuaweiCloudKmsSource {
    pub fn new(endpoint: String, ak: String, sk: String) -> Self {
        Self {
            endpoint: Some(endpoint),
            ak: zeroize::Zeroizing::new(Some(ak)),
            sk: zeroize::Zeroizing::new(Some(sk)),
        }
    }

    pub fn unconfigured() -> Self {
        Self {
            endpoint: None,
            ak: zeroize::Zeroizing::new(None),
            sk: zeroize::Zeroizing::new(None),
        }
    }
}

impl IKeySource for HuaweiCloudKmsSource {
    fn acquire_key(&self, profile: &EncryptionProfile) -> Result<ZeroizingKey, KeyError> {
        let endpoint = self
            .endpoint
            .as_ref()
            .ok_or(KeyError::Unavailable("KMS endpoint not configured".to_string()))?;

        let _ = (endpoint, profile, self.ak.as_ref(), self.sk.as_ref());
        Err(KeyError::Unavailable(
            "Huawei Cloud KMS TLS handshake not yet implemented".to_string(),
        ))
    }

    fn release_key(&self, _key: ZeroizingKey) {}
}
