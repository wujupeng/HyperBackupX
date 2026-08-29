-- 002_compat_tables.sql
-- BC-COMP: Compatibility Engineering tables
-- Maps to spec.md §5.6/§5.7, design.md §2.1.2.2/§2.3.2.2

-- =========================================================================
-- compat_repositories: Duplicati-compatible repository registration
-- =========================================================================
CREATE TABLE IF NOT EXISTS compat_repositories (
    repo_id          UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    name             TEXT NOT NULL,
    root_path        TEXT NOT NULL,
    storage_backend  TEXT NOT NULL DEFAULT 'local',
    backend_config   JSONB NOT NULL DEFAULT '{}',
    format_version   INTEGER NOT NULL DEFAULT 1,
    duplicati_semver TEXT NOT NULL DEFAULT '2.0-compatible',
    status           TEXT NOT NULL DEFAULT 'active',
    created_at       TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at       TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT chk_compat_repo_status CHECK (status IN ('active', 'disabled', 'error'))
);

CREATE INDEX IF NOT EXISTS idx_compat_repos_status ON compat_repositories(status);

-- =========================================================================
-- dual_repo_configs: Dual Repository consistency configurations
-- (must precede compat_jobs due to FK reference)
-- =========================================================================
CREATE TABLE IF NOT EXISTS dual_repo_configs (
    config_id        UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    name             TEXT NOT NULL,
    native_repo_id   UUID NOT NULL,
    compat_repo_id   UUID NOT NULL REFERENCES compat_repositories(repo_id) ON DELETE CASCADE,
    consistency_mode TEXT NOT NULL DEFAULT 'sha256',
    auto_repair      BOOLEAN NOT NULL DEFAULT false,
    alert_on_inconsistency BOOLEAN NOT NULL DEFAULT true,
    created_at       TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at       TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT chk_consistency_mode CHECK (consistency_mode IN ('sha256', 'size_only', 'metadata'))
);

-- =========================================================================
-- compat_jobs: Compatibility backup/restore jobs
-- =========================================================================
CREATE TABLE IF NOT EXISTS compat_jobs (
    job_id           UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    name             TEXT NOT NULL,
    repo_id          UUID NOT NULL REFERENCES compat_repositories(repo_id) ON DELETE CASCADE,
    source_config    JSONB NOT NULL DEFAULT '{}',
    backup_type      TEXT NOT NULL DEFAULT 'full',
    schedule_config  JSONB NOT NULL DEFAULT '{}',
    retention_config JSONB NOT NULL DEFAULT '{}',
    encryption_config JSONB NOT NULL DEFAULT '{}',
    compression_config JSONB NOT NULL DEFAULT '{}',
    dual_repo_mode   TEXT NOT NULL DEFAULT 'compatible_only',
    dual_repo_config_id UUID REFERENCES dual_repo_configs(config_id),
    status           TEXT NOT NULL DEFAULT 'active',
    created_at       TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at       TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT chk_compat_job_status CHECK (status IN ('active', 'paused', 'disabled')),
    CONSTRAINT chk_compat_backup_type CHECK (backup_type IN ('full', 'incremental')),
    CONSTRAINT chk_dual_repo_mode CHECK (dual_repo_mode IN ('native_only', 'compatible_only', 'dual_with_consistency'))
);

CREATE INDEX IF NOT EXISTS idx_compat_jobs_repo ON compat_jobs(repo_id);
CREATE INDEX IF NOT EXISTS idx_compat_jobs_status ON compat_jobs(status);

-- =========================================================================
-- compat_executions: Compatibility job execution records
-- =========================================================================
CREATE TABLE IF NOT EXISTS compat_executions (
    execution_id     UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    job_id           UUID NOT NULL REFERENCES compat_jobs(job_id) ON DELETE CASCADE,
    version_id       UUID,
    state            TEXT NOT NULL DEFAULT 'pending',
    progress         DOUBLE PRECISION NOT NULL DEFAULT 0.0,
    files_processed  BIGINT NOT NULL DEFAULT 0,
    bytes_processed  BIGINT NOT NULL DEFAULT 0,
    duration_ms      BIGINT,
    error_message    TEXT,
    checkpoint_data  JSONB,
    started_at       TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    completed_at     TIMESTAMPTZ,
    CONSTRAINT chk_compat_exec_state CHECK (state IN (
        'pending', 'aligning', 'scanning', 'chunking', 'encrypting',
        'uploading', 'comp_committing', 'verifying', 'success', 'failed', 'paused'
    ))
);

CREATE INDEX IF NOT EXISTS idx_compat_exec_job ON compat_executions(job_id);
CREATE INDEX IF NOT EXISTS idx_compat_exec_state ON compat_executions(state);
CREATE INDEX IF NOT EXISTS idx_compat_exec_started ON compat_executions(started_at DESC);

-- =========================================================================
-- duplicati_config_imports: Duplicati configuration import records
-- =========================================================================
CREATE TABLE IF NOT EXISTS duplicati_config_imports (
    import_id        UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    source_config_hash TEXT NOT NULL UNIQUE,
    source_format    TEXT NOT NULL DEFAULT 'json',
    source_config    JSONB NOT NULL,
    resulting_job_id UUID REFERENCES compat_jobs(job_id) ON DELETE SET NULL,
    field_mappings   JSONB NOT NULL DEFAULT '{}',
    unsupported_items JSONB NOT NULL DEFAULT '[]',
    import_status    TEXT NOT NULL DEFAULT 'success',
    imported_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT chk_import_status CHECK (import_status IN ('success', 'partial', 'failed')),
    CONSTRAINT chk_source_format CHECK (source_format IN ('json', 'sqlite', 'xml'))
);

CREATE INDEX IF NOT EXISTS idx_imports_hash ON duplicati_config_imports(source_config_hash);

-- =========================================================================
-- compat_metrics: Compatibility metrics (success rate, consistency rate)
-- =========================================================================
CREATE TABLE IF NOT EXISTS compat_metrics (
    metric_id        UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    metric_name      TEXT NOT NULL,
    metric_value     DOUBLE PRECISION NOT NULL,
    labels           JSONB NOT NULL DEFAULT '{}',
    recorded_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_compat_metrics_name ON compat_metrics(metric_name);
CREATE INDEX IF NOT EXISTS idx_compat_metrics_time ON compat_metrics(recorded_at DESC);

-- =========================================================================
-- Add compat engineer role and permissions
-- =========================================================================
INSERT INTO roles (name, is_builtin, permissions)
VALUES (
    'compat_engineer',
    true,
    '["compat:read", "compat:write", "compat:trigger", "compat:import", "compat:check", "devices:read", "jobs:read"]'::jsonb
)
ON CONFLICT (name) DO NOTHING;
