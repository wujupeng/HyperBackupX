package common

import (
	"context"
	"fmt"
	"time"

	"github.com/jackc/pgx/v5/pgxpool"
)

type StabilityThresholds struct {
	MemoryLeakRateUpper    float64 `json:"memory_leak_rate_upper"`
	HandleLeakRateUpper    float64 `json:"handle_leak_rate_upper"`
	CPUDriftUpper          float64 `json:"cpu_drift_upper"`
	DiskGrowthRateUpper    float64 `json:"disk_growth_rate_upper"`
	ConnectionUpper        float64 `json:"connection_upper"`
	ZombieProcessUpper     float64 `json:"zombie_process_upper"`
	HeartbeatJitterUpper   float64 `json:"heartbeat_jitter_upper"`
	HeartbeatLossRateUpper float64 `json:"heartbeat_loss_rate_upper"`
}

type FrozenTargetFreezer struct {
	pool *pgxpool.Pool
}

func NewFrozenTargetFreezer(pool *pgxpool.Pool) *FrozenTargetFreezer {
	return &FrozenTargetFreezer{pool: pool}
}

func (f *FrozenTargetFreezer) FreezeStabilityThresholds(ctx context.Context, soakSessionID string, thresholds StabilityThresholds, operator string) error {
	var sessionStatus string
	var sessionGate string
	err := f.pool.QueryRow(ctx, `SELECT status, gate FROM cert_sessions WHERE session_id = $1`, soakSessionID).Scan(&sessionStatus, &sessionGate)
	if err != nil {
		return fmt.Errorf("%w: session %s", ErrSessionNotFound, soakSessionID)
	}
	if sessionStatus != "completed" {
		return fmt.Errorf("%w: session status=%s", ErrSoakNotPassed, sessionStatus)
	}
	if sessionGate != "G17_SOAK" {
		return fmt.Errorf("%w: gate=%s", ErrNotG17Context, sessionGate)
	}

	items := []struct {
		metric string
		value  float64
	}{
		{"memory_leak_rate_upper", thresholds.MemoryLeakRateUpper},
		{"handle_leak_rate_upper", thresholds.HandleLeakRateUpper},
		{"cpu_drift_upper", thresholds.CPUDriftUpper},
		{"disk_growth_rate_upper", thresholds.DiskGrowthRateUpper},
		{"connection_upper", thresholds.ConnectionUpper},
		{"zombie_process_upper", thresholds.ZombieProcessUpper},
		{"heartbeat_jitter_upper", thresholds.HeartbeatJitterUpper},
		{"heartbeat_loss_rate_upper", thresholds.HeartbeatLossRateUpper},
	}

	now := time.Now().UTC()
	for _, item := range items {
		targetID := "stab-" + item.metric
		_, err := f.pool.Exec(ctx, `
			INSERT INTO frozen_targets (target_id, category, metric, scenario, value, unit, frozen_at, frozen_by)
			VALUES ($1, 'Stability', $2, '', $3, '', $4, $5)
			ON CONFLICT (category, metric, scenario) DO NOTHING
		`, targetID, item.metric, item.value, now, operator)
		if err != nil {
			return err
		}
	}
	return nil
}
