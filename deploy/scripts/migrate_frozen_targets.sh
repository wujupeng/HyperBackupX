#!/usr/bin/env bash
set -euo pipefail

PG_HOST="${PG_HOST:-127.0.0.1}"
PG_PORT="${PG_PORT:-5432}"
PG_DB="${PG_DB:-hbx}"
PG_USER="${PG_USER:-postgres}"

echo "[migrate_frozen_targets] Migrating G15 resource_targets and G14 chaos_targets into frozen_targets..."

psql -h "$PG_HOST" -p "$PG_PORT" -d "$PG_DB" -U "$PG_USER" <<'SQL'
-- Migrate G15 device_resource_targets (frozen=TRUE) into frozen_targets as Performance category
INSERT INTO frozen_targets (target_id, category, metric, scenario, value, unit, frozen_at, frozen_by)
SELECT
    'perf-' || target_name,
    'Performance',
    target_name,
    '',
    target_value,
    unit,
    COALESCE(frozen_at, NOW()),
    'system'
FROM device_resource_targets
WHERE frozen = TRUE
ON CONFLICT (category, metric, scenario) DO NOTHING;

-- Migrate G14 chaos_targets into frozen_targets as DisasterRecovery category
INSERT INTO frozen_targets (target_id, category, metric, scenario, value, unit, frozen_at, frozen_by)
SELECT
    'dr-' || COALESCE(scenario, 'default') || '-' || metric,
    'DisasterRecovery',
    metric,
    COALESCE(scenario, 'default'),
    value,
    COALESCE(unit, ''),
    COALESCE(frozen_at, NOW()),
    COALESCE(frozen_by, 'system')
FROM chaos_targets
WHERE frozen = TRUE
ON CONFLICT (category, metric, scenario) DO NOTHING;

SELECT category, metric, scenario, value, unit FROM frozen_targets ORDER BY category, metric;
SQL

echo "[migrate_frozen_targets] Done."