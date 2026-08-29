-- 003_testorch_tables.sql
-- BC-TEST: Test Orchestration tables
-- Maps to spec.md §6.1-6.5, design.md §2.4.1/§2.4.2/§2.4.3

-- =========================================================================
-- compatibility_matrices: Matrix definitions
-- =========================================================================
CREATE TABLE IF NOT EXISTS compatibility_matrices (
    matrix_id     UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    name          TEXT NOT NULL,
    version       INTEGER NOT NULL DEFAULT 1,
    total_entries INTEGER NOT NULL DEFAULT 0,
    passed_count  INTEGER NOT NULL DEFAULT 0,
    failed_count  INTEGER NOT NULL DEFAULT 0,
    status        TEXT NOT NULL DEFAULT 'idle',
    created_at    TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at    TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT chk_matrix_status CHECK (status IN ('idle', 'running', 'completed', 'failed'))
);

-- =========================================================================
-- matrix_entries: Individual matrix entries (L1-L5)
-- =========================================================================
CREATE TABLE IF NOT EXISTS matrix_entries (
    entry_id      UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    matrix_id     UUID NOT NULL REFERENCES compatibility_matrices(matrix_id) ON DELETE CASCADE,
    layer         TEXT NOT NULL,
    backend       TEXT NOT NULL,
    feature       TEXT NOT NULL,
    category      TEXT NOT NULL,
    status        TEXT NOT NULL DEFAULT 'pending',
    error_message TEXT,
    execution_time_ms BIGINT,
    evidence      JSONB,
    executed_at   TIMESTAMPTZ,
    CONSTRAINT chk_entry_layer CHECK (layer IN ('L1', 'L2', 'L3', 'L4', 'L5')),
    CONSTRAINT chk_entry_status CHECK (status IN ('pending', 'pass', 'fail', 'missing', 'not_applicable'))
);

CREATE INDEX IF NOT EXISTS idx_matrix_entries_matrix ON matrix_entries(matrix_id);
CREATE INDEX IF NOT EXISTS idx_matrix_entries_layer ON matrix_entries(layer);
CREATE INDEX IF NOT EXISTS idx_matrix_entries_status ON matrix_entries(status);

-- =========================================================================
-- compatibility_test_cases: Test cases for golden set
-- =========================================================================
CREATE TABLE IF NOT EXISTS compatibility_test_cases (
    case_id       UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    name          TEXT NOT NULL,
    description   TEXT,
    layer         TEXT NOT NULL,
    input_config  JSONB NOT NULL DEFAULT '{}',
    expected_behavior JSONB NOT NULL DEFAULT '{}',
    judgment_criteria TEXT NOT NULL DEFAULT 'sha256',
    status        TEXT NOT NULL DEFAULT 'pending',
    result_detail JSONB,
    matrix_entry_id UUID REFERENCES matrix_entries(entry_id),
    executed_at   TIMESTAMPTZ,
    CONSTRAINT chk_case_layer CHECK (layer IN ('L1', 'L2', 'L3', 'L4', 'L5')),
    CONSTRAINT chk_case_status CHECK (status IN ('pending', 'pass', 'fail', 'skipped')),
    CONSTRAINT chk_judgment CHECK (judgment_criteria IN ('semantic', 'sha256', 'directory_tree', 'file_size', 'metadata', 'exception_decision'))
);

CREATE INDEX IF NOT EXISTS idx_test_cases_layer ON compatibility_test_cases(layer);
CREATE INDEX IF NOT EXISTS idx_test_cases_status ON compatibility_test_cases(status);

-- =========================================================================
-- dual_run_results: Dual-run comparison results
-- =========================================================================
CREATE TABLE IF NOT EXISTS dual_run_results (
    run_id          UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    input_summary   JSONB NOT NULL DEFAULT '{}',
    duplicati_result JSONB NOT NULL DEFAULT '{}',
    hbx_result      JSONB NOT NULL DEFAULT '{}',
    comparison      JSONB NOT NULL DEFAULT '{}',
    consistency_rate DOUBLE PRECISION NOT NULL DEFAULT 0.0,
    deviation_count INTEGER NOT NULL DEFAULT 0,
    status          TEXT NOT NULL DEFAULT 'pending',
    started_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    completed_at    TIMESTAMPTZ,
    CONSTRAINT chk_dual_run_status CHECK (status IN ('pending', 'running', 'completed', 'failed'))
);

CREATE INDEX IF NOT EXISTS idx_dual_run_status ON dual_run_results(status);

-- =========================================================================
-- fuzz_scenarios: Fuzz testing scenarios
-- =========================================================================
CREATE TABLE IF NOT EXISTS fuzz_scenarios (
    scenario_id   UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    name          TEXT NOT NULL,
    description   TEXT,
    input_generator TEXT NOT NULL,
    iterations    INTEGER NOT NULL DEFAULT 1000,
    seed          BIGINT,
    status        TEXT NOT NULL DEFAULT 'pending',
    corruption_found BOOLEAN NOT NULL DEFAULT false,
    result_detail JSONB,
    started_at    TIMESTAMPTZ,
    completed_at  TIMESTAMPTZ,
    CONSTRAINT chk_fuzz_status CHECK (status IN ('pending', 'running', 'completed', 'failed'))
);

-- =========================================================================
-- chaos_scenarios: Chaos testing scenarios
-- =========================================================================
CREATE TABLE IF NOT EXISTS chaos_scenarios (
    scenario_id   UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    name          TEXT NOT NULL,
    description   TEXT,
    fault_type    TEXT NOT NULL,
    target        TEXT NOT NULL,
    duration_sec  INTEGER NOT NULL DEFAULT 60,
    status        TEXT NOT NULL DEFAULT 'pending',
    recovered     BOOLEAN NOT NULL DEFAULT false,
    result_detail JSONB,
    started_at    TIMESTAMPTZ,
    completed_at  TIMESTAMPTZ,
    CONSTRAINT chk_chaos_status CHECK (status IN ('pending', 'running', 'completed', 'failed')),
    CONSTRAINT chk_fault_type CHECK (fault_type IN ('network_partition', 'disk_full', 'permission_denied', 'file_lock', 'source_deleted', 'repo_unavailable', 'process_kill', 'power_loss'))
);

-- =========================================================================
-- compatibility_reports: Generated reports
-- =========================================================================
CREATE TABLE IF NOT EXISTS compatibility_reports (
    report_id     UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    report_type   TEXT NOT NULL,
    matrix_id     UUID REFERENCES compatibility_matrices(matrix_id),
    summary       JSONB NOT NULL DEFAULT '{}',
    details       JSONB NOT NULL DEFAULT '{}',
    generated_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT chk_report_type CHECK (report_type IN ('matrix', 'golden', 'dual_run', 'fuzz', 'chaos', 'acceptance'))
);

CREATE INDEX IF NOT EXISTS idx_reports_type ON compatibility_reports(report_type);
CREATE INDEX IF NOT EXISTS idx_reports_time ON compatibility_reports(generated_at DESC);