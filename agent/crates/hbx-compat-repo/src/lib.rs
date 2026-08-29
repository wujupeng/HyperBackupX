pub mod repository;
pub mod self_check;

pub use repository::{
    CompatibleRepository, CompatibilityVersion, CompatibilityVersionSummary,
    CompatibilityManifest, CompatFileEntry, CompatChunkLocation, CompatibilityHashes,
    CompatRepoConfig, CompatRepoError,
};
pub use self_check::{
    CompatRepoIntegrityReport, DamageType, DamageLocation,
    ICompatibilityRepository,
};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompatRepoMetadata {
    pub repository_id: String,
    pub format_version: u32,
    pub duplicati_semantic_version: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

pub const COMPAT_FORMAT_VERSION: u32 = 1;
pub const DUPLICATI_SEMANTIC_VERSION: &str = "2.0-compatible";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compat_repo_metadata_serde() {
        let meta = CompatRepoMetadata {
            repository_id: "test-repo".to_string(),
            format_version: COMPAT_FORMAT_VERSION,
            duplicati_semantic_version: DUPLICATI_SEMANTIC_VERSION.to_string(),
            created_at: chrono::Utc::now(),
        };
        let json = serde_json::to_string(&meta).unwrap();
        let de: CompatRepoMetadata = serde_json::from_str(&json).unwrap();
        assert_eq!(de.repository_id, "test-repo");
        assert_eq!(de.format_version, COMPAT_FORMAT_VERSION);
    }

    #[test]
    fn test_format_version_is_independent_from_native() {
        assert_eq!(COMPAT_FORMAT_VERSION, 1);
    }
}