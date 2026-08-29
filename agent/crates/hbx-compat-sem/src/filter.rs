use hbx_core::domain::common::FilterRule;
use serde::{Deserialize, Serialize};

use super::SemanticError;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DuplicatiFilterType {
    Glob,
    Regex,
    PathPrefix,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DuplicatiFilterRule {
    pub pattern: String,
    pub filter_type: DuplicatiFilterType,
}

pub fn align_filter_rules(
    rules: &[DuplicatiFilterRule],
) -> Result<Vec<FilterRule>, SemanticError> {
    rules.iter().map(align_single_rule).collect()
}

fn align_single_rule(rule: &DuplicatiFilterRule) -> Result<FilterRule, SemanticError> {
    let normalized = normalize_path(&rule.pattern);
    match rule.filter_type {
        DuplicatiFilterType::Glob => {
            validate_glob(&normalized)?;
            Ok(FilterRule::Glob(normalized))
        }
        DuplicatiFilterType::Regex => {
            regex::Regex::new(&normalized)
                .map_err(|e| SemanticError::UnsupportedConfig(format!("invalid regex: {e}")))?;
            Ok(FilterRule::Regex(normalized))
        }
        DuplicatiFilterType::PathPrefix => {
            if normalized.is_empty() {
                return Err(SemanticError::UnsupportedConfig("empty path prefix".to_string()));
            }
            Ok(FilterRule::PathPrefix(normalized))
        }
    }
}

fn normalize_path(p: &str) -> String {
    p.replace('\\', "/")
}

fn validate_glob(pattern: &str) -> Result<(), SemanticError> {
    if pattern.is_empty() {
        return Err(SemanticError::UnsupportedConfig("empty glob pattern".to_string()));
    }
    let invalid = ['|', '<', '>', '"'];
    if pattern.chars().any(|c| invalid.contains(&c)) {
        return Err(SemanticError::UnsupportedConfig(format!(
            "invalid characters in glob: {pattern}"
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_align_glob_rule() {
        let rule = DuplicatiFilterRule {
            pattern: "*.tmp".to_string(),
            filter_type: DuplicatiFilterType::Glob,
        };
        let result = align_filter_rules(&[rule]).unwrap();
        assert_eq!(result.len(), 1);
        match &result[0] {
            FilterRule::Glob(s) => assert_eq!(s, "*.tmp"),
            _ => panic!("expected Glob"),
        }
    }

    #[test]
    fn test_align_regex_rule() {
        let rule = DuplicatiFilterRule {
            pattern: r".*\.log$".to_string(),
            filter_type: DuplicatiFilterType::Regex,
        };
        let result = align_filter_rules(&[rule]).unwrap();
        match &result[0] {
            FilterRule::Regex(_) => {}
            _ => panic!("expected Regex"),
        }
    }

    #[test]
    fn test_align_path_prefix_rule() {
        let rule = DuplicatiFilterRule {
            pattern: "C:\\Users\\test".to_string(),
            filter_type: DuplicatiFilterType::PathPrefix,
        };
        let result = align_filter_rules(&[rule]).unwrap();
        match &result[0] {
            FilterRule::PathPrefix(s) => assert_eq!(s, "C:/Users/test"),
            _ => panic!("expected PathPrefix"),
        }
    }

    #[test]
    fn test_align_invalid_regex() {
        let rule = DuplicatiFilterRule {
            pattern: "[".to_string(),
            filter_type: DuplicatiFilterType::Regex,
        };
        let result = align_filter_rules(&[rule]);
        assert!(result.is_err());
    }

    #[test]
    fn test_align_empty_glob() {
        let rule = DuplicatiFilterRule {
            pattern: "".to_string(),
            filter_type: DuplicatiFilterType::Glob,
        };
        let result = align_filter_rules(&[rule]);
        assert!(result.is_err());
    }

    #[test]
    fn test_align_multiple_rules() {
        let rules = vec![
            DuplicatiFilterRule { pattern: "*.tmp".to_string(), filter_type: DuplicatiFilterType::Glob },
            DuplicatiFilterRule { pattern: "cache/".to_string(), filter_type: DuplicatiFilterType::PathPrefix },
            DuplicatiFilterRule { pattern: r".*\.bak$".to_string(), filter_type: DuplicatiFilterType::Regex },
        ];
        let result = align_filter_rules(&rules).unwrap();
        assert_eq!(result.len(), 3);
    }

    #[test]
    fn test_path_separator_normalization() {
        let rule = DuplicatiFilterRule {
            pattern: "C:\\Temp\\*.log".to_string(),
            filter_type: DuplicatiFilterType::Glob,
        };
        let result = align_filter_rules(&[rule]).unwrap();
        match &result[0] {
            FilterRule::Glob(s) => assert!(s.contains('/'), "path separators should be normalized to /"),
            _ => panic!("expected Glob"),
        }
    }
}