-- 006_pending_tasks.sql
-- Pending task queue for agent dispatch

CREATE TABLE IF NOT EXISTS pending_tasks (
    task_id         UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    agent_id        UUID,
    job_id          TEXT NOT NULL,
    repo_id         TEXT NOT NULL,
    task_type       TEXT NOT NULL DEFAULT 'backup',
    source_path     TEXT NOT NULL DEFAULT '',
    target_path     TEXT NOT NULL DEFAULT '',
    badou_grpc_endpoint TEXT NOT NULL DEFAULT '',
    spec_json       JSONB NOT NULL DEFAULT '{}',
    status          TEXT NOT NULL DEFAULT 'pending',
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    dispatched_at   TIMESTAMPTZ,
    CONSTRAINT chk_pending_status CHECK (status IN ('pending', 'dispatched', 'completed', 'failed'))
);

CREATE INDEX IF NOT EXISTS idx_pending_tasks_agent ON pending_tasks(agent_id);
CREATE INDEX IF NOT EXISTS idx_pending_tasks_status ON pending_tasks(status);
CREATE INDEX IF NOT EXISTS idx_pending_tasks_created ON pending_tasks(created_at);