use hbx_core::domain::common::{CompressionAlgorithm, CompressionProfile};
use serde::{Deserialize, Serialize};

use super::SemanticError;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DuplicatiCompressionConfig {
    pub algorithm: String,
    pub level: u32,
}

pub fn align_compression(
    config: &DuplicatiCompressionConfig,
) -> Result<CompressionProfile, SemanticError> {
    let algorithm = match config.algorithm.to_lowercase().as_str() {
        "zip" | "gzip" | "deflate" => CompressionAlgorithm::Zstd,
        "lz4" | "lzma" | "brotli" => CompressionAlgorithm::Lz4,
        "none" | "no-compression" => CompressionAlgorithm::None,
        other => {
            return Err(SemanticError::UnsupportedConfig(format!(
                "unknown compression algorithm: {other}"
            )))
        }
    };

    let level = if config.level > 22 { 22 } else { config.level };

    Ok(CompressionProfile { algorithm, level })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gzip_to_zstd() {
        let config = DuplicatiCompressionConfig {
            algorithm: "gzip".to_string(),
            level: 5,
        };
        let result = align_compression(&config).unwrap();
        assert_eq!(result.algorithm, CompressionAlgorithm::Zstd);
        assert_eq!(result.level, 5);
    }

    #[test]
    fn test_zip_to_zstd() {
        let config = DuplicatiCompressionConfig {
            algorithm: "zip".to_string(),
            level: 3,
        };
        let result = align_compression(&config).unwrap();
        assert_eq!(result.algorithm, CompressionAlgorithm::Zstd);
    }

    #[test]
    fn test_lz4_mapping() {
        let config = DuplicatiCompressionConfig {
            algorithm: "lz4".to_string(),
            level: 1,
        };
        let result = align_compression(&config).unwrap();
        assert_eq!(result.algorithm, CompressionAlgorithm::Lz4);
    }

    #[test]
    fn test_none_compression() {
        let config = DuplicatiCompressionConfig {
            algorithm: "none".to_string(),
            level: 0,
        };
        let result = align_compression(&config).unwrap();
        assert_eq!(result.algorithm, CompressionAlgorithm::None);
    }

    #[test]
    fn test_unknown_algorithm() {
        let config = DuplicatiCompressionConfig {
            algorithm: "rar".to_string(),
            level: 5,
        };
        let result = align_compression(&config);
        assert!(result.is_err());
    }

    #[test]
    fn test_level_clamped() {
        let config = DuplicatiCompressionConfig {
            algorithm: "gzip".to_string(),
            level: 100,
        };
        let result = align_compression(&config).unwrap();
        assert_eq!(result.level, 22);
    }

    #[test]
    fn test_case_insensitive() {
        let config = DuplicatiCompressionConfig {
            algorithm: "GZIP".to_string(),
            level: 5,
        };
        let result = align_compression(&config).unwrap();
        assert_eq!(result.algorithm, CompressionAlgorithm::Zstd);
    }
}