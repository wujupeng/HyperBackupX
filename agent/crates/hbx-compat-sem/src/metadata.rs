use hbx_core::domain::common::FileAttributes;
use hbx_core::domain::repository::FileEntry;
use serde::{Deserialize, Serialize};

use super::SemanticError;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DuplicatiMetadata {
    pub path: String,
    pub size: u64,
    pub modified_at: chrono::DateTime<chrono::Utc>,
    pub is_directory: bool,
    pub is_hidden: bool,
    pub is_read_only: bool,
    pub is_system: bool,
}

pub fn align_metadata(
    metadata: &DuplicatiMetadata,
) -> Result<FileEntry, SemanticError> {
    let path = metadata.path.replace('\\', "/");

    if path.is_empty() {
        return Err(SemanticError::UnsupportedConfig("empty file path".to_string()));
    }

    let attributes = FileAttributes {
        is_directory: metadata.is_directory,
        is_hidden: metadata.is_hidden,
        is_system: metadata.is_system,
        is_read_only: metadata.is_read_only,
        windows_acl: None,
    };

    Ok(FileEntry {
        path,
        size: metadata.size,
        modified_at: metadata.modified_at,
        attributes,
        chunks: vec![],
        file_hash: [0u8; 32],
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_test_metadata() -> DuplicatiMetadata {
        DuplicatiMetadata {
            path: "C:\\Users\\test\\file.txt".to_string(),
            size: 1024,
            modified_at: chrono::Utc::now(),
            is_directory: false,
            is_hidden: false,
            is_read_only: false,
            is_system: false,
        }
    }

    #[test]
    fn test_align_basic_metadata() {
        let meta = make_test_metadata();
        let result = align_metadata(&meta).unwrap();
        assert_eq!(result.size, 1024);
        assert!(!result.attributes.is_directory);
    }

    #[test]
    fn test_path_separator_normalization() {
        let meta = DuplicatiMetadata {
            path: "C:\\Users\\test\\file.txt".to_string(),
            size: 0,
            modified_at: chrono::Utc::now(),
            is_directory: false,
            is_hidden: false,
            is_read_only: false,
            is_system: false,
        };
        let result = align_metadata(&meta).unwrap();
        assert_eq!(result.path, "C:/Users/test/file.txt");
    }

    #[test]
    fn test_empty_path_rejected() {
        let meta = DuplicatiMetadata {
            path: "".to_string(),
            size: 0,
            modified_at: chrono::Utc::now(),
            is_directory: false,
            is_hidden: false,
            is_read_only: false,
            is_system: false,
        };
        let result = align_metadata(&meta);
        assert!(result.is_err());
    }

    #[test]
    fn test_directory_attribute() {
        let meta = DuplicatiMetadata {
            path: "/home/user/docs".to_string(),
            size: 0,
            modified_at: chrono::Utc::now(),
            is_directory: true,
            is_hidden: false,
            is_read_only: false,
            is_system: false,
        };
        let result = align_metadata(&meta).unwrap();
        assert!(result.attributes.is_directory);
    }

    #[test]
    fn test_hidden_and_readonly_attributes() {
        let meta = DuplicatiMetadata {
            path: "/home/user/secret".to_string(),
            size: 100,
            modified_at: chrono::Utc::now(),
            is_directory: false,
            is_hidden: true,
            is_read_only: true,
            is_system: false,
        };
        let result = align_metadata(&meta).unwrap();
        assert!(result.attributes.is_hidden);
        assert!(result.attributes.is_read_only);
    }
}