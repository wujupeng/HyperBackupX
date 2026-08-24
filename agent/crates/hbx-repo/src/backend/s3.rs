use std::time::Duration;

use hbx_core::domain::chunk::{ChunkHash, ChunkLocation};
use hbx_core::domain::common::{LockOperation, RepoLock, VersionId, VersionSummary};
use hbx_core::domain::encryption::EncryptedChunk;
use hbx_core::domain::repository::Manifest;
use hbx_core::pipeline::{IBackupRepository, RepoError};
use parking_lot::Mutex;
use sha2::{Digest, Sha256};
use uuid::Uuid;

use super::config::S3Config;
use crate::format::{decode_encrypted_chunk, encode_encrypted_chunk};

#[derive(Debug, Clone)]
pub struct S3Credentials {
    pub access_key: String,
    pub secret_key: String,
}

pub struct S3Repository {
    config: S3Config,
    credentials: S3Credentials,
    _lock: Mutex<()>,
}

impl S3Repository {
    pub fn new(config: S3Config, credentials: S3Credentials) -> Self {
        Self {
            config,
            credentials,
            _lock: Mutex::new(()),
        }
    }

    fn scheme(&self) -> &str {
        if self.config.use_tls {
            "https"
        } else {
            "http"
        }
    }

    fn object_url(&self, key: &str) -> String {
        if self.config.path_style {
            format!(
                "{}://{}/{}/{}",
                self.scheme(),
                self.config.endpoint,
                self.config.bucket,
                key
            )
        } else {
            format!("{}://{}/{}", self.scheme(), self.config.endpoint, key)
        }
    }

    #[allow(dead_code)]
    fn bucket_url(&self) -> String {
        if self.config.path_style {
            format!("{}://{}/{}", self.scheme(), self.config.endpoint, self.config.bucket)
        } else {
            format!("{}://{}.{}", self.scheme(), self.config.bucket, self.config.endpoint)
        }
    }

    fn chunk_key(hash: &ChunkHash) -> String {
        let bucket = format!("{:02x}", hash.0[0]);
        let filename = hex::encode(hash.0) + ".chunk";
        format!("chunks/{}/{}", bucket, filename)
    }

    fn manifest_key(version_id: &VersionId) -> String {
        format!("manifests/{}.manifest", version_id.0)
    }

    fn lock_key(lock_id: &Uuid) -> String {
        format!("locks/{}.lock", lock_id)
    }

    fn sign_request(
        &self,
        method: &str,
        url: &str,
        body: &[u8],
        content_type: &str,
    ) -> Result<ureq::Request, RepoError> {
        let now = chrono::Utc::now();
        let amz_date = now.format("%Y%m%dT%H%M%SZ").to_string();
        let date_stamp = now.format("%Y%m%d").to_string();

        let payload_hash = hex::encode(Sha256::digest(body));

        let parsed_url = url::Url::parse(url)
            .map_err(|e| RepoError::Failed(format!("invalid URL: {e}")))?;
        let host = parsed_url.host_str().unwrap_or("").to_string();

        let canonical_uri = parsed_url.path().to_string();

        let canonical_headers = format!(
            "host:{host}\nx-amz-content-sha256:{payload_hash}\nx-amz-date:{amz_date}\n"
        );
        let signed_headers = "host;x-amz-content-sha256;x-amz-date";

        let canonical_request = format!(
            "{method}\n{canonical_uri}\n\n{canonical_headers}\n{signed_headers}\n{payload_hash}"
        );

        let canonical_request_hash = hex::encode(Sha256::digest(canonical_request.as_bytes()));

        let credential_scope = format!(
            "{date_stamp}/{}/s3/aws4_request",
            self.config.region
        );

        let string_to_sign = format!(
            "AWS4-HMAC-SHA256\n{amz_date}\n{credential_scope}\n{canonical_request_hash}"
        );

        let signing_key = compute_signing_key(
            &self.credentials.secret_key,
            &date_stamp,
            &self.config.region,
            "s3",
        );

        let signature = hex::encode(hmac_sha256(&signing_key, string_to_sign.as_bytes()));

        let authorization = format!(
            "AWS4-HMAC-SHA256 Credential={}/{credential_scope}, SignedHeaders={signed_headers}, Signature={signature}",
            self.credentials.access_key
        );

        let request = match method {
            "PUT" => ureq::put(url),
            "GET" => ureq::get(url),
            "HEAD" => ureq::head(url),
            "DELETE" => ureq::delete(url),
            _ => return Err(RepoError::Failed(format!("unsupported method: {method}"))),
        };

        Ok(request
            .set("Authorization", &authorization)
            .set("x-amz-date", &amz_date)
            .set("x-amz-content-sha256", &payload_hash)
            .set("Content-Type", content_type)
            .set("Host", &host))
    }

    fn put_object(&self, key: &str, data: &[u8]) -> Result<(), RepoError> {
        let url = self.object_url(key);
        let req = self.sign_request("PUT", &url, data, "application/octet-stream")?;
        let resp = req.send_bytes(data);
        match resp {
            Ok(_) => Ok(()),
            Err(ureq::Error::Status(code, resp)) => {
                let body = resp.into_string().unwrap_or_default();
                Err(RepoError::Failed(format!("S3 PUT failed: {code}: {body}")))
            }
            Err(e) => Err(RepoError::Failed(format!("S3 PUT error: {e}"))),
        }
    }

    fn get_object(&self, key: &str) -> Result<Vec<u8>, RepoError> {
        let url = self.object_url(key);
        let req = self.sign_request("GET", &url, &[], "application/octet-stream")?;
        let resp = req.call();
        match resp {
            Ok(resp) => {
                let mut buf = Vec::new();
                resp.into_reader()
                    .read_to_end(&mut buf)
                    .map_err(|e| RepoError::Failed(format!("read error: {e}")))?;
                Ok(buf)
            }
            Err(ureq::Error::Status(404, _)) => {
                Err(RepoError::NotFound(format!("S3 object not found: {key}")))
            }
            Err(ureq::Error::Status(code, resp)) => {
                let body = resp.into_string().unwrap_or_default();
                Err(RepoError::Failed(format!("S3 GET failed: {code}: {body}")))
            }
            Err(e) => Err(RepoError::Failed(format!("S3 GET error: {e}"))),
        }
    }

    fn head_object(&self, key: &str) -> Result<bool, RepoError> {
        let url = self.object_url(key);
        let req = self.sign_request("HEAD", &url, &[], "application/octet-stream")?;
        let resp = req.call();
        match resp {
            Ok(_) => Ok(true),
            Err(ureq::Error::Status(404, _)) => Ok(false),
            Err(ureq::Error::Status(code, resp)) => {
                let body = resp.into_string().unwrap_or_default();
                Err(RepoError::Failed(format!("S3 HEAD failed: {code}: {body}")))
            }
            Err(e) => Err(RepoError::Failed(format!("S3 HEAD error: {e}"))),
        }
    }

    fn delete_object(&self, key: &str) -> Result<(), RepoError> {
        let url = self.object_url(key);
        let req = self.sign_request("DELETE", &url, &[], "application/octet-stream")?;
        let resp = req.call();
        match resp {
            Ok(_) => Ok(()),
            Err(ureq::Error::Status(404, _)) => Ok(()),
            Err(ureq::Error::Status(code, resp)) => {
                let body = resp.into_string().unwrap_or_default();
                Err(RepoError::Failed(format!("S3 DELETE failed: {code}: {body}")))
            }
            Err(e) => Err(RepoError::Failed(format!("S3 DELETE error: {e}"))),
        }
    }
}

fn hmac_sha256(key: &[u8], data: &[u8]) -> Vec<u8> {
    use hmac::{Hmac, Mac};
    type HmacSha256 = Hmac<Sha256>;
    let mut mac = HmacSha256::new_from_slice(key).expect("HMAC key error");
    mac.update(data);
    mac.finalize().into_bytes().to_vec()
}

fn compute_signing_key(secret_key: &str, date: &str, region: &str, service: &str) -> Vec<u8> {
    let k_date = hmac_sha256(format!("AWS4{secret_key}").as_bytes(), date.as_bytes());
    let k_region = hmac_sha256(&k_date, region.as_bytes());
    let k_service = hmac_sha256(&k_region, service.as_bytes());
    hmac_sha256(&k_service, b"aws4_request")
}

impl IBackupRepository for S3Repository {
    fn write_chunk(
        &self,
        hash: &ChunkHash,
        encrypted: &EncryptedChunk,
    ) -> Result<ChunkLocation, RepoError> {
        let key = Self::chunk_key(hash);
        let data = encode_encrypted_chunk(encrypted);
        self.put_object(&key, &data)?;

        Ok(ChunkLocation {
            bucket: format!("{:02x}", hash.0[0]),
            path: hex::encode(hash.0) + ".chunk",
        })
    }

    fn read_chunk(&self, location: &ChunkLocation) -> Result<EncryptedChunk, RepoError> {
        let key = format!("chunks/{}/{}", location.bucket, location.path);
        let data = self.get_object(&key)?;
        decode_encrypted_chunk(&data).map_err(RepoError::Io)
    }

    fn chunk_exists(&self, hash: &ChunkHash) -> Result<bool, RepoError> {
        let key = Self::chunk_key(hash);
        self.head_object(&key)
    }

    fn delete_chunk(&self, location: &ChunkLocation) -> Result<(), RepoError> {
        let key = format!("chunks/{}/{}", location.bucket, location.path);
        self.delete_object(&key)
    }

    fn write_manifest(
        &self,
        version_id: &VersionId,
        manifest: &Manifest,
    ) -> Result<(), RepoError> {
        let key = Self::manifest_key(version_id);
        let data = serde_json::to_vec(manifest)?;
        self.put_object(&key, &data)
    }

    fn read_manifest(&self, version_id: &VersionId) -> Result<Manifest, RepoError> {
        let key = Self::manifest_key(version_id);
        let data = self.get_object(&key)?;
        let manifest: Manifest = serde_json::from_slice(&data)?;
        Ok(manifest)
    }

    fn list_versions(&self) -> Result<Vec<VersionSummary>, RepoError> {
        Err(RepoError::Failed(
            "list_versions requires S3 ListObjectsV2 API, not yet implemented".to_string(),
        ))
    }

    fn acquire_lock(
        &self,
        operation: LockOperation,
        _timeout: Duration,
    ) -> Result<RepoLock, RepoError> {
        let lock_id = Uuid::new_v4();
        let key = Self::lock_key(&lock_id);
        let lock = RepoLock {
            lock_id,
            holder: format!("{:?}", operation),
            acquired_at: chrono::Utc::now(),
            ttl: Duration::from_secs(1800),
        };

        let data = serde_json::to_vec(&serde_json::json!({
            "lock_id": lock_id,
            "holder": lock.holder,
            "acquired_at": lock.acquired_at,
            "ttl_secs": lock.ttl.as_secs(),
        }))?;

        self.put_object(&key, &data)?;
        Ok(lock)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hbx_core::domain::repository::BackendType;

    #[test]
    fn test_s3_config_creation() {
        let config = S3Config {
            endpoint: "s3.amazonaws.com".to_string(),
            region: "us-east-1".to_string(),
            bucket: "test-bucket".to_string(),
            use_tls: true,
            path_style: false,
        };
        let creds = S3Credentials {
            access_key: "AKIAIOSFODNN7EXAMPLE".to_string(),
            secret_key: "wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY".to_string(),
        };
        let repo = S3Repository::new(config, creds);
        assert_eq!(repo.config.bucket, "test-bucket");
    }

    #[test]
    fn test_chunk_key_format() {
        let hash = ChunkHash([0xab; 32]);
        let key = S3Repository::chunk_key(&hash);
        assert!(key.starts_with("chunks/ab/"));
        assert!(key.ends_with(".chunk"));
    }

    #[test]
    fn test_signing_key_computation() {
        let key = compute_signing_key("secret", "20240101", "us-east-1", "s3");
        assert_eq!(key.len(), 32);
    }

    #[test]
    fn test_hmac_sha256() {
        let result = hmac_sha256(b"key", b"data");
        assert_eq!(result.len(), 32);
    }

    #[test]
    fn test_backend_type_s3() {
        let bt = BackendType::S3;
        assert_eq!(format!("{:?}", bt), "S3");
    }
}