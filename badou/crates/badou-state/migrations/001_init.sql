-- 八斗存储桶 PostgreSQL Schema v1
-- 独立 badou schema，不影响现有 hbx_control 表

CREATE SCHEMA IF NOT EXISTS badou;

-- 1. repositories
CREATE TABLE badou.repositories (
    repo_id          UUID PRIMARY KEY,
    name             VARCHAR(255) NOT NULL UNIQUE,
    status           TEXT NOT NULL DEFAULT 'active'
                     CHECK (status IN ('active', 'readonly', 'deleted', 'immutable')),
    immutable_until  TIMESTAMPTZ,
    backend_node     VARCHAR(255),
    version_count    BIGINT NOT NULL DEFAULT 0,
    total_size       BIGINT NOT NULL DEFAULT 0,
    stored_size      BIGINT NOT NULL DEFAULT 0,
    created_at       TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at       TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- 2. versions
CREATE TABLE badou.versions (
    version_id        UUID PRIMARY KEY,
    repo_id           UUID NOT NULL REFERENCES badou.repositories(repo_id) ON DELETE CASCADE,
    snapshot_id       UUID,
    parent_version_id UUID,
    sequence          BIGINT NOT NULL,
    status            TEXT NOT NULL DEFAULT 'created'
                      CHECK (status IN ('created', 'writing', 'verifying', 'committing',
                                        'sealed', 'expired', 'deleted', 'gc_pending', 'purged')),
    sealed_at         TIMESTAMPTZ,
    immutable_until   TIMESTAMPTZ,
    created_at        TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at        TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (repo_id, sequence)
);

-- 3. snapshots
CREATE TABLE badou.snapshots (
    snapshot_id      UUID PRIMARY KEY,
    version_id       UUID NOT NULL REFERENCES badou.versions(version_id) ON DELETE CASCADE,
    repo_id          UUID NOT NULL REFERENCES badou.repositories(repo_id) ON DELETE CASCADE,
    source_machine   JSONB NOT NULL,
    backup_policy    JSONB NOT NULL,
    manifest_id      UUID,
    encryption_info  JSONB,
    compression_info JSONB,
    version_info     JSONB,
    verify_info      JSONB,
    status           TEXT NOT NULL DEFAULT 'created'
                     CHECK (status IN ('created', 'writing', 'sealed', 'corrupt', 'deleted')),
    total_size       BIGINT NOT NULL DEFAULT 0,
    stored_size      BIGINT NOT NULL DEFAULT 0,
    file_count       INTEGER NOT NULL DEFAULT 0,
    chunk_count      INTEGER NOT NULL DEFAULT 0,
    created_at       TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- 4. manifests
CREATE TABLE badou.manifests (
    manifest_id   UUID PRIMARY KEY,
    snapshot_id   UUID NOT NULL REFERENCES badou.snapshots(snapshot_id) ON DELETE CASCADE,
    chunk_count   INTEGER NOT NULL DEFAULT 0,
    file_count    INTEGER NOT NULL DEFAULT 0,
    total_size    BIGINT NOT NULL DEFAULT 0,
    manifest_hash VARCHAR(64) NOT NULL,
    storage_path  TEXT NOT NULL,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- 5. chunk_ref_counts
CREATE TABLE badou.chunk_ref_counts (
    repo_id        UUID NOT NULL REFERENCES badou.repositories(repo_id) ON DELETE CASCADE,
    chunk_hash     VARCHAR(64) NOT NULL,
    ref_count      INTEGER NOT NULL DEFAULT 0,
    size           BIGINT NOT NULL,
    stored_size    BIGINT NOT NULL,
    status         TEXT NOT NULL DEFAULT 'active'
                   CHECK (status IN ('active', 'gc_pending', 'purged')),
    encryption_ref JSONB,
    created_at     TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at     TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (repo_id, chunk_hash)
);

-- 6. journal
CREATE TABLE badou.journal (
    journal_id   UUID PRIMARY KEY,
    repo_id      UUID NOT NULL REFERENCES badou.repositories(repo_id) ON DELETE CASCADE,
    operation    TEXT NOT NULL
                 CHECK (operation IN ('put_chunk', 'delete_chunk', 'commit_snapshot',
                                      'delete_version', 'gc')),
    committed    BOOLEAN NOT NULL DEFAULT false,
    payload_ref  TEXT,
    timestamp    TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- 7. gc_reports
CREATE TABLE badou.gc_reports (
    report_id      UUID PRIMARY KEY,
    repo_id        UUID NOT NULL REFERENCES badou.repositories(repo_id) ON DELETE CASCADE,
    report_data    JSONB NOT NULL,
    chunks_scanned BIGINT NOT NULL DEFAULT 0,
    chunks_collected BIGINT NOT NULL DEFAULT 0,
    bytes_freed    BIGINT NOT NULL DEFAULT 0,
    started_at     TIMESTAMPTZ NOT NULL,
    completed_at   TIMESTAMPTZ,
    created_at     TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- 8. verify_reports
CREATE TABLE badou.verify_reports (
    report_id     UUID PRIMARY KEY,
    repo_id       UUID NOT NULL REFERENCES badou.repositories(repo_id) ON DELETE CASCADE,
    target_id     UUID NOT NULL,
    verify_level  TEXT NOT NULL
                  CHECK (verify_level IN ('repository', 'version', 'chunk')),
    passed        BOOLEAN NOT NULL,
    total_checked BIGINT NOT NULL DEFAULT 0,
    total_failed  BIGINT NOT NULL DEFAULT 0,
    failed_items  JSONB,
    checked_at    TIMESTAMPTZ NOT NULL,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Indexes
CREATE INDEX idx_versions_repo_status ON badou.versions (repo_id, status);
CREATE INDEX idx_versions_repo_sequence ON badou.versions (repo_id, sequence);
CREATE INDEX idx_snapshots_repo ON badou.snapshots (repo_id);
CREATE INDEX idx_snapshots_version ON badou.snapshots (version_id);
CREATE INDEX idx_chunk_refs_repo_status ON badou.chunk_ref_counts (repo_id, status);
CREATE INDEX idx_chunk_refs_ref_count ON badou.chunk_ref_counts (ref_count) WHERE ref_count = 0;
CREATE INDEX idx_journal_repo_committed ON badou.journal (repo_id, committed);