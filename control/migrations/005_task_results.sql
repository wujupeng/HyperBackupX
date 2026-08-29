-- 005_task_results.sql
-- Task execution results from agents

CREATE TABLE IF NOT EXISTS task_results (
    id              UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    task_id         TEXT NOT NULL,
    agent_id        UUID NOT NULL,
    job_id          TEXT NOT NULL,
    status          TEXT NOT NULL DEFAULT 'pending',
    started_at      TIMESTAMPTZ,
    completed_at    TIMESTAMPTZ,
    bytes_processed BIGINT NOT NULL DEFAULT 0,
    bytes_stored    BIGINT NOT NULL DEFAULT 0,
    file_count      INTEGER NOT NULL DEFAULT 0,
    chunk_count     INTEGER NOT NULL DEFAULT 0,
    dedup_ratio     DOUBLE PRECISION NOT NULL DEFAULT 0.0,
    version_id      TEXT,
    error_message   TEXT,
    trace_id        TEXT,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_task_results_task ON task_results(task_id);
CREATE INDEX IF NOT EXISTS idx_task_results_agent ON task_results(agent_id);
CREATE INDEX IF NOT EXISTS idx_task_results_job ON task_results(job_id);
CREATE INDEX IF NOT EXISTS idx_task_results_status ON task_results(status);