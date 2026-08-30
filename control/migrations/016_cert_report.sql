-- G17-G20 DB-01.3: Certification report archive table
CREATE TABLE IF NOT EXISTS cert_reports (
    report_id VARCHAR(64) PRIMARY KEY,
    session_id VARCHAR(64) NOT NULL REFERENCES cert_sessions(session_id),
    gate VARCHAR(16) NOT NULL,
    verdict VARCHAR(16) NOT NULL,
    content JSONB NOT NULL,
    evidence_package_ref TEXT,
    archived_at TIMESTAMPTZ NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_cert_reports_session
    ON cert_reports (session_id);

CREATE INDEX IF NOT EXISTS idx_cert_reports_gate
    ON cert_reports (gate, archived_at DESC);