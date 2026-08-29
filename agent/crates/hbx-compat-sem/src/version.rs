use hbx_core::domain::backup::BackupType;
use serde::{Deserialize, Serialize};

use super::SemanticError;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DuplicatiVersionStrategy {
    pub incremental: bool,
    pub full_backup_interval: u32,
}

pub fn align_version_strategy(
    strategy: &DuplicatiVersionStrategy,
) -> Result<(BackupType, Option<String>), SemanticError> {
    if strategy.incremental {
        Ok((BackupType::Incremental, None))
    } else {
        Ok((BackupType::Full, None))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_full_backup_strategy() {
        let strategy = DuplicatiVersionStrategy {
            incremental: false,
            full_backup_interval: 0,
        };
        let (bt, baseline) = align_version_strategy(&strategy).unwrap();
        assert_eq!(bt, BackupType::Full);
        assert!(baseline.is_none());
    }

    #[test]
    fn test_incremental_backup_strategy() {
        let strategy = DuplicatiVersionStrategy {
            incremental: true,
            full_backup_interval: 7,
        };
        let (bt, baseline) = align_version_strategy(&strategy).unwrap();
        assert_eq!(bt, BackupType::Incremental);
        assert!(baseline.is_none());
    }

    #[test]
    fn test_incremental_with_interval() {
        let strategy = DuplicatiVersionStrategy {
            incremental: true,
            full_backup_interval: 30,
        };
        let (bt, _) = align_version_strategy(&strategy).unwrap();
        assert_eq!(bt, BackupType::Incremental);
    }
}