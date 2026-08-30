-- G17-G20 DB-01.3: NOT_TESTED reasons table
CREATE TABLE IF NOT EXISTS not_tested_reasons (
    reason_id VARCHAR(64) PRIMARY KEY,
    session_id VARCHAR(64) NOT NULL REFERENCES cert_sessions(session_id),
    item VARCHAR(128) NOT NULL,
    cause TEXT NOT NULL,
    required_resource TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_not_tested_reasons_session
    ON not_tested_reasons (session_id);