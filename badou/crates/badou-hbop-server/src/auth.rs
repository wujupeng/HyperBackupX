//! mTLS + JWT 鉴权 + RBAC + 版本协商。
//!
//! - mTLS 强制：tonic Server 加载服务端证书 + 校验客户端证书，禁止明文（C-SEC-BD-001）。
//! - JWT 鉴权：从 gRPC metadata 提取 `authorization: Bearer <jwt>`，校验 HMAC-SHA256 签名与过期。
//! - RBAC：校验角色权限，无权限返回 FORBIDDEN。
//! - 版本协商：从 metadata 提取 `x-hbop-version`，校验版本范围（C-COMP-BD-003）。

use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use hmac::{Hmac, Mac};
use sha2::Sha256;
use serde::{Deserialize, Serialize};
use tonic::{Request, Status};

type HmacSha256 = Hmac<Sha256>;

/// HBOP 协议支持的主版本号。
pub const HBOP_VERSION_MAJOR: u32 = 1;

/// 支持的版本范围（向后兼容）。
pub const SUPPORTED_VERSIONS: &[u32] = &[1];

/// gRPC metadata key: 协议版本。
pub const METADATA_VERSION: &str = "x-hbop-version";

/// gRPC metadata key: 授权。
pub const METADATA_AUTH: &str = "authorization";

/// RBAC 角色。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Role {
    Admin,
    Operator,
    Viewer,
}

impl Role {
    pub fn from_role_str(s: &str) -> Option<Self> {
        match s {
            "admin" => Some(Self::Admin),
            "operator" => Some(Self::Operator),
            "viewer" => Some(Self::Viewer),
            _ => None,
        }
    }

    /// Admin 可执行所有操作；Operator 可执行除管理类以外的操作；Viewer 只读。
    pub fn can_write(&self) -> bool {
        matches!(self, Self::Admin | Self::Operator)
    }

    pub fn can_admin(&self) -> bool {
        matches!(self, Self::Admin)
    }
}

/// JWT Claims。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JwtClaims {
    /// Subject (用户 ID)。
    pub sub: String,
    /// Role。
    pub role: String,
    /// Expiration (UNIX timestamp)。
    pub exp: u64,
    /// Issued at。
    pub iat: u64,
}

/// 鉴权配置。
#[derive(Clone)]
pub struct AuthConfig {
    jwt_secret: Vec<u8>,
}

impl AuthConfig {
    /// 从 HMAC secret 创建鉴权配置。
    pub fn from_secret(secret: &[u8]) -> Self {
        Self {
            jwt_secret: secret.to_vec(),
        }
    }

    /// 校验 JWT 并返回 Claims（HMAC-SHA256 签名验证 + 过期检查）。
    #[allow(clippy::result_large_err)]
    pub fn validate_jwt(&self, token: &str) -> Result<JwtClaims, Status> {
        let parts: Vec<&str> = token.split('.').collect();
        if parts.len() != 3 {
            return Err(Status::unauthenticated("AUTH_FAILED: invalid JWT format"));
        }

        let header_b64 = parts[0];
        let payload_b64 = parts[1];
        let signature_b64 = parts[2];

        let signing_input = format!("{}.{}", header_b64, payload_b64);
        let expected_sig = URL_SAFE_NO_PAD.decode(signature_b64)
            .map_err(|_| Status::unauthenticated("AUTH_FAILED: invalid JWT signature encoding"))?;

        let mut mac = HmacSha256::new_from_slice(&self.jwt_secret)
            .map_err(|_| Status::internal("AUTH_FAILED: invalid HMAC key"))?;
        mac.update(signing_input.as_bytes());
        mac.verify_slice(&expected_sig)
            .map_err(|_| Status::unauthenticated("AUTH_FAILED: JWT signature verification failed"))?;

        let payload_bytes = URL_SAFE_NO_PAD.decode(payload_b64)
            .map_err(|_| Status::unauthenticated("AUTH_FAILED: invalid JWT payload encoding"))?;

        let claims: JwtClaims = serde_json::from_slice(&payload_bytes)
            .map_err(|e| Status::unauthenticated(format!("AUTH_FAILED: invalid JWT claims: {}", e)))?;

        let now = chrono::Utc::now().timestamp() as u64;
        if claims.exp < now {
            return Err(Status::unauthenticated("AUTH_FAILED: JWT expired"));
        }

        Ok(claims)
    }
}

/// 生成 JWT token（用于测试）。
pub fn generate_jwt(secret: &[u8], claims: &JwtClaims) -> Result<String, String> {
    let header = serde_json::json!({"alg": "HS256", "typ": "JWT"});
    let header_b64 = URL_SAFE_NO_PAD.encode(serde_json::to_vec(&header).map_err(|e| e.to_string())?);
    let payload_b64 = URL_SAFE_NO_PAD.encode(serde_json::to_vec(claims).map_err(|e| e.to_string())?);

    let signing_input = format!("{}.{}", header_b64, payload_b64);
    let mut mac = HmacSha256::new_from_slice(secret).map_err(|e| e.to_string())?;
    mac.update(signing_input.as_bytes());
    let signature = mac.finalize().into_bytes();
    let sig_b64 = URL_SAFE_NO_PAD.encode(signature);

    Ok(format!("{}.{}.{}", header_b64, payload_b64, sig_b64))
}

/// 从 gRPC metadata 提取并校验 JWT。
#[allow(clippy::result_large_err)]
pub fn extract_auth<T>(request: &Request<T>, config: &AuthConfig) -> Result<JwtClaims, Status> {
    let metadata = request.metadata();
    let auth_header = metadata
        .get(METADATA_AUTH)
        .ok_or_else(|| Status::unauthenticated("AUTH_FAILED: missing authorization header"))?
        .to_str()
        .map_err(|_| Status::unauthenticated("AUTH_FAILED: invalid authorization header encoding"))?;

    let token = auth_header
        .strip_prefix("Bearer ")
        .ok_or_else(|| Status::unauthenticated("AUTH_FAILED: expected Bearer token"))?;

    config.validate_jwt(token)
}

/// 从 gRPC metadata 提取并校验协议版本。
#[allow(clippy::result_large_err)]
pub fn extract_version<T>(request: &Request<T>) -> Result<u32, Status> {
    let metadata = request.metadata();
    let version_str = metadata
        .get(METADATA_VERSION)
        .ok_or_else(|| {
            Status::invalid_argument("VERSION_MISMATCH: missing x-hbop-version header")
        })?
        .to_str()
        .map_err(|_| Status::invalid_argument("VERSION_MISMATCH: invalid version header encoding"))?;

    let version: u32 = version_str
        .parse()
        .map_err(|_| {
            Status::invalid_argument(format!("VERSION_MISMATCH: invalid version: {}", version_str))
        })?;

    if !SUPPORTED_VERSIONS.contains(&version) {
        return Err(Status::invalid_argument(format!(
            "VERSION_MISMATCH: unsupported version {}, supported: {:?}",
            version, SUPPORTED_VERSIONS
        )));
    }

    Ok(version)
}

/// 从请求中提取鉴权信息 + 版本号，并校验写权限。
#[allow(clippy::result_large_err)]
pub fn require_write<T>(request: &Request<T>, config: &AuthConfig) -> Result<(JwtClaims, u32), Status> {
    let claims = extract_auth(request, config)?;
    let version = extract_version(request)?;
    let role = Role::from_role_str(&claims.role)
        .ok_or_else(|| Status::permission_denied("FORBIDDEN: unknown role"))?;
    if !role.can_write() {
        return Err(Status::permission_denied("FORBIDDEN: write permission required"));
    }
    Ok((claims, version))
}

/// 从请求中提取鉴权信息 + 版本号，并校验管理员权限。
#[allow(clippy::result_large_err)]
pub fn require_admin<T>(request: &Request<T>, config: &AuthConfig) -> Result<(JwtClaims, u32), Status> {
    let claims = extract_auth(request, config)?;
    let version = extract_version(request)?;
    let role = Role::from_role_str(&claims.role)
        .ok_or_else(|| Status::permission_denied("FORBIDDEN: unknown role"))?;
    if !role.can_admin() {
        return Err(Status::permission_denied("FORBIDDEN: admin permission required"));
    }
    Ok((claims, version))
}

/// 从请求中提取鉴权信息 + 版本号（只读，任何已鉴权用户）。
#[allow(clippy::result_large_err)]
pub fn require_auth<T>(request: &Request<T>, config: &AuthConfig) -> Result<(JwtClaims, u32), Status> {
    let claims = extract_auth(request, config)?;
    let version = extract_version(request)?;
    Ok((claims, version))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn make_token(secret: &[u8], role: &str, exp_offset: i64) -> String {
        let claims = JwtClaims {
            sub: "test-user".to_string(),
            role: role.to_string(),
            exp: (Utc::now().timestamp() + exp_offset) as u64,
            iat: Utc::now().timestamp() as u64,
        };
        generate_jwt(secret, &claims).unwrap()
    }

    #[test]
    fn validate_valid_jwt() {
        let secret = b"test-secret";
        let config = AuthConfig::from_secret(secret);
        let token = make_token(secret, "admin", 3600);
        let claims = config.validate_jwt(&token).unwrap();
        assert_eq!(claims.sub, "test-user");
        assert_eq!(claims.role, "admin");
    }

    #[test]
    fn validate_expired_jwt_fails() {
        let secret = b"test-secret";
        let config = AuthConfig::from_secret(secret);
        let token = make_token(secret, "admin", -3600);
        assert!(config.validate_jwt(&token).is_err());
    }

    #[test]
    fn validate_wrong_secret_fails() {
        let config = AuthConfig::from_secret(b"correct-secret");
        let token = make_token(b"wrong-secret", "admin", 3600);
        assert!(config.validate_jwt(&token).is_err());
    }

    #[test]
    fn validate_tampered_payload_fails() {
        let secret = b"test-secret";
        let config = AuthConfig::from_secret(secret);
        let token = make_token(secret, "admin", 3600);
        let parts: Vec<&str> = token.split('.').collect();
        let tampered = format!("{}.{}.{}", parts[0], "eyJzdWIiOiJ0ZXN0In0", parts[2]);
        assert!(config.validate_jwt(&tampered).is_err());
    }

    #[test]
    fn role_permissions() {
        assert!(Role::Admin.can_write());
        assert!(Role::Admin.can_admin());
        assert!(Role::Operator.can_write());
        assert!(!Role::Operator.can_admin());
        assert!(!Role::Viewer.can_write());
        assert!(!Role::Viewer.can_admin());
    }

    #[test]
    fn version_negotiation() {
        assert!(SUPPORTED_VERSIONS.contains(&1));
        assert!(!SUPPORTED_VERSIONS.contains(&2));
    }
}
