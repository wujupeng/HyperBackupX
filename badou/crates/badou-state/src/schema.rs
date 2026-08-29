pub const SCHEMA_NAME: &str = "badou";
pub const FORMAT_VERSION: u32 = 1;

pub const TABLE_REPOSITORIES: &str = "badou.repositories";
pub const TABLE_VERSIONS: &str = "badou.versions";
pub const TABLE_SNAPSHOTS: &str = "badou.snapshots";
pub const TABLE_MANIFESTS: &str = "badou.manifests";
pub const TABLE_CHUNK_REF_COUNTS: &str = "badou.chunk_ref_counts";
pub const TABLE_JOURNAL: &str = "badou.journal";
pub const TABLE_GC_REPORTS: &str = "badou.gc_reports";
pub const TABLE_VERIFY_REPORTS: &str = "badou.verify_reports";

pub const MIGRATION_001: &str = include_str!("../migrations/001_init.sql");