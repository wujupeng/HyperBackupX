-- G17-G20 DB-01.2: Frozen target values table (Stability/Performance/DisasterRecovery)
CREATE TABLE IF NOT EXISTS frozen_targets (
    target_id VARCHAR(64) PRIMARY KEY,
    category VARCHAR(16) NOT NULL,
    metric VARCHAR(64) NOT NULL,
    scenario VARCHAR(64) NOT NULL DEFAULT '',
    value DOUBLE PRECISION NOT NULL,
    unit VARCHAR(32) NOT NULL DEFAULT '',
    frozen_at TIMESTAMPTZ NOT NULL,
    frozen_by VARCHAR(64) NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_frozen_targets_unique
    ON frozen_targets (category, metric, scenario);