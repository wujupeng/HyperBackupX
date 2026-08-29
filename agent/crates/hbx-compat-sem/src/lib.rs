pub mod filter;
pub mod version;
pub mod compression;
pub mod encryption;
pub mod metadata;
pub mod exception;

pub use filter::{DuplicatiFilterRule, DuplicatiFilterType, align_filter_rules};
pub use version::{DuplicatiVersionStrategy, align_version_strategy};
pub use compression::{DuplicatiCompressionConfig, align_compression};
pub use encryption::{DuplicatiEncryptionConfig, align_encryption};
pub use metadata::{DuplicatiMetadata, align_metadata};
pub use exception::{ExceptionType, ExceptionDecision, align_exception_decision};

use thiserror::Error;

#[derive(Debug, Error)]
pub enum SemanticError {
    #[error("unsupported config: {0}")]
    UnsupportedConfig(String),
    #[error("ambiguous mapping: {0}")]
    AmbiguousMapping(String),
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DuplicatiSemanticConfig {
    pub filters: Vec<DuplicatiFilterRule>,
    pub version_strategy: DuplicatiVersionStrategy,
    pub compression: DuplicatiCompressionConfig,
    pub encryption: DuplicatiEncryptionConfig,
    pub metadata: DuplicatiMetadata,
}

pub trait ISemanticAligner: Send + Sync {
    fn align_filter_rules(
        &self,
        rules: &[DuplicatiFilterRule],
    ) -> Result<Vec<hbx_core::domain::common::FilterRule>, SemanticError>;

    fn align_version_strategy(
        &self,
        strategy: &DuplicatiVersionStrategy,
    ) -> Result<(hbx_core::domain::backup::BackupType, Option<String>), SemanticError>;

    fn align_compression(
        &self,
        config: &DuplicatiCompressionConfig,
    ) -> Result<hbx_core::domain::common::CompressionProfile, SemanticError>;

    fn align_encryption(
        &self,
        config: &DuplicatiEncryptionConfig,
    ) -> Result<hbx_core::domain::encryption::EncryptionProfile, SemanticError>;

    fn align_metadata(
        &self,
        metadata: &DuplicatiMetadata,
    ) -> Result<hbx_core::domain::repository::FileEntry, SemanticError>;

    fn align_exception_decision(
        &self,
        exception: ExceptionType,
    ) -> Result<ExceptionDecision, SemanticError>;
}

pub struct DefaultSemanticAligner;

impl ISemanticAligner for DefaultSemanticAligner {
    fn align_filter_rules(
        &self,
        rules: &[DuplicatiFilterRule],
    ) -> Result<Vec<hbx_core::domain::common::FilterRule>, SemanticError> {
        filter::align_filter_rules(rules)
    }

    fn align_version_strategy(
        &self,
        strategy: &DuplicatiVersionStrategy,
    ) -> Result<(hbx_core::domain::backup::BackupType, Option<String>), SemanticError> {
        version::align_version_strategy(strategy)
    }

    fn align_compression(
        &self,
        config: &DuplicatiCompressionConfig,
    ) -> Result<hbx_core::domain::common::CompressionProfile, SemanticError> {
        compression::align_compression(config)
    }

    fn align_encryption(
        &self,
        config: &DuplicatiEncryptionConfig,
    ) -> Result<hbx_core::domain::encryption::EncryptionProfile, SemanticError> {
        encryption::align_encryption(config)
    }

    fn align_metadata(
        &self,
        metadata: &DuplicatiMetadata,
    ) -> Result<hbx_core::domain::repository::FileEntry, SemanticError> {
        metadata::align_metadata(metadata)
    }

    fn align_exception_decision(
        &self,
        exception: ExceptionType,
    ) -> Result<ExceptionDecision, SemanticError> {
        exception::align_exception_decision(exception)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_aligner_filter() {
        let aligner = DefaultSemanticAligner;
        let rules = vec![DuplicatiFilterRule {
            pattern: "*.tmp".to_string(),
            filter_type: DuplicatiFilterType::Glob,
        }];
        let result = aligner.align_filter_rules(&rules).unwrap();
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn test_default_aligner_version_full() {
        let aligner = DefaultSemanticAligner;
        let strategy = DuplicatiVersionStrategy {
            incremental: false,
            full_backup_interval: 0,
        };
        let (bt, baseline) = aligner.align_version_strategy(&strategy).unwrap();
        assert_eq!(bt, hbx_core::domain::backup::BackupType::Full);
        assert!(baseline.is_none());
    }

    #[test]
    fn test_default_aligner_version_incremental() {
        let aligner = DefaultSemanticAligner;
        let strategy = DuplicatiVersionStrategy {
            incremental: true,
            full_backup_interval: 7,
        };
        let (bt, _baseline) = aligner.align_version_strategy(&strategy).unwrap();
        assert_eq!(bt, hbx_core::domain::backup::BackupType::Incremental);
    }

    #[test]
    fn test_default_aligner_compression() {
        let aligner = DefaultSemanticAligner;
        let config = DuplicatiCompressionConfig {
            algorithm: "gzip".to_string(),
            level: 5,
        };
        let result = aligner.align_compression(&config).unwrap();
        assert_eq!(result.algorithm, hbx_core::domain::common::CompressionAlgorithm::Zstd);
    }

    #[test]
    fn test_default_aligner_exception_network() {
        let aligner = DefaultSemanticAligner;
        let decision = aligner.align_exception_decision(ExceptionType::NetworkBreak).unwrap();
        assert_eq!(decision, ExceptionDecision::Retry);
    }

    #[test]
    fn test_default_aligner_exception_disk_full() {
        let aligner = DefaultSemanticAligner;
        let decision = aligner.align_exception_decision(ExceptionType::DiskFull).unwrap();
        assert_eq!(decision, ExceptionDecision::Abort);
    }

    #[test]
    fn test_semantic_error_display() {
        let e = SemanticError::UnsupportedConfig("unknown algorithm".to_string());
        assert!(format!("{e}").contains("unknown algorithm"));
    }
}