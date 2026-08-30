-- G17-G20 DB-01.1: Certification session table
CREATE TABLE IF NOT EXISTS cert_sessions (
    session_id VARCHAR(64) PRIMARY KEY,
    gate VARCHAR(16) NOT NULL,
    status VARCHAR(16) NOT NULL,
    operator VARCHAR(64) NOT NULL,
    started_at TIMESTAMPTZ NOT NULL,
    completed_at TIMESTAMPTZ,
    detail JSONB,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_cert_sessions_gate_status
    ON cert_sessions (gate, status);