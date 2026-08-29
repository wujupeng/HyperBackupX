
use hbx_cli::args::Args;

#[test]
fn test_parse_positional_args() {
    let args = Args::parse(&[
        "backup".to_string(),
        "job-123".to_string(),
        "--repo".to_string(),
        "repo-456".to_string(),
    ]);
    assert_eq!(args.positional, vec!["backup", "job-123"]);
    assert_eq!(args.get("repo"), Some("repo-456"));
}

#[test]
fn test_parse_bool_flag() {
    let args = Args::parse(&[
        "delete".to_string(),
        "ver-789".to_string(),
        "--force".to_string(),
    ]);
    assert_eq!(args.positional, vec!["delete", "ver-789"]);
    assert!(args.has("force"));
}

#[test]
fn test_parse_multiple_flags() {
    let args = Args::parse(&[
        "restore".to_string(),
        "ver-1".to_string(),
        "--target".to_string(),
        "/tmp/restore".to_string(),
        "--mode".to_string(),
        "overwrite".to_string(),
        "--selection".to_string(),
        "all".to_string(),
    ]);
    assert_eq!(args.get("target"), Some("/tmp/restore"));
    assert_eq!(args.get("mode"), Some("overwrite"));
    assert_eq!(args.get("selection"), Some("all"));
}

#[test]
fn test_parse_short_flag() {
    let args = Args::parse(&[
        "list".to_string(),
        "repo-1".to_string(),
        "-v".to_string(),
    ]);
    assert!(args.has("v"));
}

#[test]
fn test_get_or_default() {
    let args = Args::parse(&["backup".to_string()]);
    assert_eq!(args.get_or("mode", "Quick"), "Quick");
}

#[test]
fn test_empty_args() {
    let args = Args::parse(&[]);
    assert!(args.positional.is_empty());
    assert!(args.flags.is_empty());
    assert!(args.bool_flags.is_empty());
}

#[test]
fn test_has_flag_not_set() {
    let args = Args::parse(&["backup".to_string()]);
    assert!(!args.has("force"));
}

#[test]
fn test_flag_at_end() {
    let args = Args::parse(&[
        "import".to_string(),
        "config.json".to_string(),
        "--dry-run".to_string(),
    ]);
    assert_eq!(args.positional, vec!["import", "config.json"]);
    assert!(args.has("dry-run"));
}

#[test]
fn test_mixed_positional_and_flags() {
    let args = Args::parse(&[
        "verify".to_string(),
        "repo-1".to_string(),
        "--mode".to_string(),
        "Full".to_string(),
        "--deep".to_string(),
    ]);
    assert_eq!(args.positional, vec!["verify", "repo-1"]);
    assert_eq!(args.get("mode"), Some("Full"));
    assert!(args.has("deep"));
}