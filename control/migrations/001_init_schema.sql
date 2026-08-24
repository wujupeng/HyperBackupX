-- HBX Control Plane: Initial Schema Migration
-- Based on design.md §2.8.2 PostgreSQL Schema Design
-- All tables organized by Bounded Context

-- Enable extensions
CREATE EXTENSION IF NOT EXISTS "uuid-ossp";
CREATE EXTENSION IF NOT EXISTS "pgcrypto";

-- BC-9: Organizations (referenced by users and devices)
CREATE TABLE IF NOT EXISTS organizations (
    organization_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    name VARCHAR(128) NOT NULL,
    parent_id UUID REFERENCES organizations,
    path VARCHAR(1024) NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- BC-9: Users and Permissions
CREATE TABLE IF NOT EXISTS users (
    user_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    username VARCHAR(128) UNIQUE NOT NULL,
    display_name VARCHAR(255) NOT NULL,
    email VARCHAR(255) UNIQUE NOT NULL,
    password_hash VARCHAR(255),
    auth_source VARCHAR(32) NOT NULL DEFAULT 'local',
    organization_id UUID REFERENCES organizations,
    status VARCHAR(16) NOT NULL DEFAULT 'active',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS roles (
    role_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    name VARCHAR(64) UNIQUE NOT NULL,
    is_builtin BOOLEAN NOT NULL DEFAULT FALSE,
    permissions JSONB NOT NULL DEFAULT '[]',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS user_roles (
    user_id UUID REFERENCES users ON DELETE CASCADE,
    role_id UUID REFERENCES roles ON DELETE CASCADE,
    PRIMARY KEY (user_id, role_id)
);

-- BC-7: Device Management
CREATE TABLE IF NOT EXISTS devices (
    device_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    hostname VARCHAR(255) NOT NULL,
    os_type VARCHAR(16) NOT NULL,
    hardware_profile JSONB NOT NULL DEFAULT '{}',
    agent_version VARCHAR(32) NOT NULL,
    status VARCHAR(16) NOT NULL DEFAULT 'offline',
    organization_id UUID REFERENCES organizations,
    agent_credential_hash VARCHAR(255),
    last_heartbeat_at TIMESTAMPTZ,
    registered_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS idx_devices_org ON devices(organization_id);
CREATE INDEX IF NOT EXISTS idx_devices_status ON devices(status);

-- BC-9: Policy Management
CREATE TABLE IF NOT EXISTS policies (
    policy_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    name VARCHAR(128) NOT NULL,
    template JSONB NOT NULL DEFAULT '{}',
    version INT NOT NULL DEFAULT 1,
    scope_type VARCHAR(16) NOT NULL,
    scope_id UUID NOT NULL,
    status VARCHAR(16) NOT NULL DEFAULT 'active',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS idx_policies_scope ON policies(scope_type, scope_id);

CREATE TABLE IF NOT EXISTS policy_versions (
    policy_id UUID REFERENCES policies ON DELETE CASCADE,
    version INT NOT NULL,
    template JSONB NOT NULL DEFAULT '{}',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    created_by UUID REFERENCES users,
    PRIMARY KEY (policy_id, version)
);

-- BC-1: Backup Jobs (control plane metadata)
CREATE TABLE IF NOT EXISTS backup_jobs (
    job_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    device_id UUID REFERENCES devices ON DELETE CASCADE,
    name VARCHAR(128) NOT NULL,
    source_config JSONB NOT NULL DEFAULT '{}',
    destination_config JSONB NOT NULL DEFAULT '{}',
    schedule_id UUID,
    retention_policy_id UUID,
    encryption_profile_id UUID,
    status VARCHAR(16) NOT NULL DEFAULT 'active',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS idx_backup_jobs_device ON backup_jobs(device_id);

CREATE TABLE IF NOT EXISTS backup_versions (
    version_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    job_id UUID REFERENCES backup_jobs ON DELETE CASCADE,
    version_number BIGINT NOT NULL,
    timestamp TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    backup_type VARCHAR(16) NOT NULL,
    parent_version_id UUID,
    status VARCHAR(16) NOT NULL,
    file_count BIGINT NOT NULL DEFAULT 0,
    total_size BIGINT NOT NULL DEFAULT 0,
    stored_size BIGINT NOT NULL DEFAULT 0,
    manifest_hash VARCHAR(64) NOT NULL,
    UNIQUE (job_id, version_number)
);
CREATE INDEX IF NOT EXISTS idx_versions_job ON backup_versions(job_id, timestamp DESC);

-- BC-5: Restore Jobs
CREATE TABLE IF NOT EXISTS restore_jobs (
    restore_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    source_version_id UUID REFERENCES backup_versions,
    device_id UUID REFERENCES devices,
    file_selection JSONB NOT NULL DEFAULT '{}',
    restore_mode VARCHAR(16) NOT NULL,
    target_location TEXT NOT NULL,
    status VARCHAR(16) NOT NULL DEFAULT 'pending',
    started_at TIMESTAMPTZ,
    completed_at TIMESTAMPTZ,
    failed_files JSONB
);

-- BC-4: Repository Registration
CREATE TABLE IF NOT EXISTS repositories (
    repository_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    name VARCHAR(128) NOT NULL,
    backend_type VARCHAR(32) NOT NULL,
    connection_config JSONB NOT NULL DEFAULT '{}',
    credential_id UUID,
    format_version INT NOT NULL DEFAULT 1,
    status VARCHAR(16) NOT NULL DEFAULT 'active',
    total_capacity BIGINT,
    used_capacity BIGINT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS credentials (
    credential_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    name VARCHAR(128) NOT NULL,
    type VARCHAR(32) NOT NULL,
    encrypted_secret BYTEA NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- BC-9: Audit Logs (partitioned by month)
CREATE TABLE IF NOT EXISTS audit_logs (
    log_id UUID DEFAULT uuid_generate_v4(),
    actor_id VARCHAR(255) NOT NULL,
    actor_type VARCHAR(16) NOT NULL,
    action VARCHAR(32) NOT NULL,
    target_type VARCHAR(64) NOT NULL,
    target_id VARCHAR(255) NOT NULL,
    result VARCHAR(16) NOT NULL,
    timestamp TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    trace_id VARCHAR(64),
    detail JSONB,
    PRIMARY KEY (log_id, timestamp)
) PARTITION BY RANGE (timestamp);

-- Create initial monthly partitions for audit_logs (current + next 12 months)
DO $$
DECLARE
    i INT;
    start_date TIMESTAMPTZ;
    end_date TIMESTAMPTZ;
    partition_name TEXT;
BEGIN
    FOR i IN 0..12 LOOP
        start_date := date_trunc('month', NOW() + (i || ' month')::INTERVAL);
        end_date := start_date + INTERVAL '1 month';
        partition_name := 'audit_logs_' || to_char(start_date, 'YYYY_MM');
        EXECUTE format(
            'CREATE TABLE IF NOT EXISTS %I PARTITION OF audit_logs FOR VALUES FROM (%L) TO (%L)',
            partition_name, start_date, end_date
        );
    END LOOP;
END $$;

-- BC-9: Alerts
CREATE TABLE IF NOT EXISTS alerts (
    alert_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    rule_id VARCHAR(64) NOT NULL,
    severity VARCHAR(16) NOT NULL,
    device_id UUID REFERENCES devices,
    message TEXT NOT NULL,
    triggered_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    suppressed BOOLEAN NOT NULL DEFAULT FALSE,
    acknowledged BOOLEAN NOT NULL DEFAULT FALSE,
    acknowledged_by UUID REFERENCES users,
    acknowledged_at TIMESTAMPTZ
);

-- BC-9: Centralized Agent Logs (partitioned by day)
CREATE TABLE IF NOT EXISTS agent_logs (
    log_id BIGSERIAL,
    device_id UUID NOT NULL,
    timestamp TIMESTAMPTZ NOT NULL,
    level VARCHAR(16) NOT NULL,
    component VARCHAR(64) NOT NULL,
    trace_id VARCHAR(64),
    message TEXT NOT NULL,
    fields JSONB,
    PRIMARY KEY (log_id, timestamp)
) PARTITION BY RANGE (timestamp);

-- Create initial daily partitions for agent_logs (current + next 30 days)
DO $$
DECLARE
    i INT;
    start_date TIMESTAMPTZ;
    end_date TIMESTAMPTZ;
    partition_name TEXT;
BEGIN
    FOR i IN 0..30 LOOP
        start_date := date_trunc('day', NOW() + (i || ' day')::INTERVAL);
        end_date := start_date + INTERVAL '1 day';
        partition_name := 'agent_logs_' || to_char(start_date, 'YYYY_MM_DD');
        EXECUTE format(
            'CREATE TABLE IF NOT EXISTS %I PARTITION OF agent_logs FOR VALUES FROM (%L) TO (%L)',
            partition_name, start_date, end_date
        );
    END LOOP;
END $$;

-- Composite index for log retrieval by device + time
CREATE INDEX IF NOT EXISTS idx_agent_logs_device_time ON agent_logs(device_id, timestamp DESC);
CREATE INDEX IF NOT EXISTS idx_audit_logs_time ON audit_logs(timestamp DESC);

-- Seed builtin roles
INSERT INTO roles (name, is_builtin, permissions) VALUES
    ('admin', TRUE, '["*"]'::jsonb),
    ('operator', TRUE, '["devices:read","jobs:*","versions:read","restores:*","verify:*","monitoring:read","logs:read"]'::jsonb),
    ('auditor', TRUE, '["audit:read","logs:read"]'::jsonb),
    ('viewer', TRUE, '["devices:read","versions:read","monitoring:read"]'::jsonb)
ON CONFLICT (name) DO NOTHING;