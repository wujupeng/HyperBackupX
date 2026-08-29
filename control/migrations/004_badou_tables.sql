-- 004_badou_tables.sql
-- Gate-BD-17: Control Plane 八斗管理表
-- Maps to spec.md §5.1.1/§5.8, design.md §2.2.2.9
-- 这些表在 Control Plane 侧管理，与八斗 badou schema 分离

-- =========================================================================
-- badou_repositories: 八斗 Repository 注册
-- =========================================================================
CREATE TABLE IF NOT EXISTS badou_repositories (
    repo_id          UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    name             TEXT NOT NULL UNIQUE,
    description      TEXT NOT NULL DEFAULT '',
    -- 八斗节点连接信息
    node_address     TEXT NOT NULL,
    node_port        INTEGER NOT NULL DEFAULT 50051,
    -- mTLS 证书引用（路径或 secret 引用）
    tls_cert_path    TEXT NOT NULL DEFAULT '',
    tls_key_path     TEXT NOT NULL DEFAULT '',
    tls_ca_path      TEXT NOT NULL DEFAULT '',
    -- JWT 凭据引用
    jwt_subject      TEXT NOT NULL DEFAULT '',
    jwt_secret_ref   TEXT NOT NULL DEFAULT '',
    -- 不可变保留
    immutable_retention_days INTEGER NOT NULL DEFAULT 0,
    -- 状态
    status           TEXT NOT NULL DEFAULT 'active',
    -- 元数据
    created_at       TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at       TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT chk_badou_repo_status CHECK (status IN ('active', 'disabled', 'error', 'maintenance'))
);

CREATE INDEX IF NOT EXISTS idx_badou_repos_status ON badou_repositories(status);

-- =========================================================================
-- badou_nodes: 八斗集群节点
-- =========================================================================
CREATE TABLE IF NOT EXISTS badou_nodes (
    node_id          UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    node_address     TEXT NOT NULL,
    node_port        INTEGER NOT NULL DEFAULT 50051,
    node_role        TEXT NOT NULL DEFAULT 'follower',
    status           TEXT NOT NULL DEFAULT 'online',
    -- 容量信息
    disk_capacity_bytes BIGINT NOT NULL DEFAULT 0,
    disk_used_bytes     BIGINT NOT NULL DEFAULT 0,
    -- 元数据
    joined_at        TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    last_heartbeat_at  TIMESTAMPTZ,
    metadata         JSONB NOT NULL DEFAULT '{}',
    CONSTRAINT chk_badou_node_role CHECK (node_role IN ('leader', 'follower', 'learner')),
    CONSTRAINT chk_badou_node_status CHECK (status IN ('online', 'offline', 'draining', 'failed'))
);

CREATE INDEX IF NOT EXISTS idx_badou_nodes_status ON badou_nodes(status);

-- =========================================================================
-- badou_cluster_topology: 集群拓扑关系
-- =========================================================================
CREATE TABLE IF NOT EXISTS badou_cluster_topology (
    topology_id      UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    cluster_name     TEXT NOT NULL,
    node_id          UUID NOT NULL REFERENCES badou_nodes(node_id) ON DELETE CASCADE,
    shard_id         INTEGER NOT NULL DEFAULT 0,
    replica_role     TEXT NOT NULL DEFAULT 'primary',
    configured_at    TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT chk_badou_replica_role CHECK (replica_role IN ('primary', 'secondary', 'witness'))
);

CREATE INDEX IF NOT EXISTS idx_badou_topology_cluster ON badou_cluster_topology(cluster_name);
CREATE INDEX IF NOT EXISTS idx_badou_topology_node ON badou_cluster_topology(node_id);

-- =========================================================================
-- badou_gc_reports: GC 报告记录
-- =========================================================================
CREATE TABLE IF NOT EXISTS badou_gc_reports (
    report_id        UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    repo_id          UUID NOT NULL REFERENCES badou_repositories(repo_id) ON DELETE CASCADE,
    triggered_by     TEXT NOT NULL DEFAULT '',
    chunks_scanned   BIGINT NOT NULL DEFAULT 0,
    chunks_deleted   BIGINT NOT NULL DEFAULT 0,
    bytes_freed      BIGINT NOT NULL DEFAULT 0,
    duration_ms      BIGINT NOT NULL DEFAULT 0,
    status           TEXT NOT NULL DEFAULT 'running',
    started_at       TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    completed_at     TIMESTAMPTZ,
    CONSTRAINT chk_badou_gc_status CHECK (status IN ('running', 'completed', 'failed'))
);

CREATE INDEX IF NOT EXISTS idx_badou_gc_repo ON badou_gc_reports(repo_id);