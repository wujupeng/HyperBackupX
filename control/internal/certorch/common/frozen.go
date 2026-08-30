package common

import (
	"context"
	"fmt"

	"github.com/jackc/pgx/v5/pgxpool"
)

type FrozenCategory string

const (
	CatStability        FrozenCategory = "Stability"
	CatPerformance      FrozenCategory = "Performance"
	CatDisasterRecovery FrozenCategory = "DisasterRecovery"
)

type FrozenTarget struct {
	TargetID string         `json:"target_id"`
	Category FrozenCategory `json:"category"`
	Metric   string         `json:"metric"`
	Scenario string         `json:"scenario"`
	Value    float64        `json:"value"`
	Unit     string         `json:"unit"`
	FrozenAt string         `json:"frozen_at"`
	FrozenBy string         `json:"frozen_by"`
}

type FrozenTargetStore struct {
	pool *pgxpool.Pool
}

func NewFrozenTargetStore(pool *pgxpool.Pool) *FrozenTargetStore {
	return &FrozenTargetStore{pool: pool}
}

func (s *FrozenTargetStore) GetFrozenTarget(ctx context.Context, category FrozenCategory, metric, scenario string) (FrozenTarget, error) {
	row := s.pool.QueryRow(ctx, `
		SELECT target_id, category, metric, scenario, value, unit, frozen_at::text, frozen_by
		FROM frozen_targets
		WHERE category = $1 AND metric = $2 AND scenario = $3
	`, string(category), metric, scenario)

	var ft FrozenTarget
	err := row.Scan(&ft.TargetID, &ft.Category, &ft.Metric, &ft.Scenario, &ft.Value, &ft.Unit, &ft.FrozenAt, &ft.FrozenBy)
	if err != nil {
		return FrozenTarget{}, fmt.Errorf("%w: category=%s metric=%s scenario=%s", ErrNotFrozen, category, metric, scenario)
	}
	return ft, nil
}

func (s *FrozenTargetStore) ListFrozenTargets(ctx context.Context, category FrozenCategory) ([]FrozenTarget, error) {
	rows, err := s.pool.Query(ctx, `
		SELECT target_id, category, metric, scenario, value, unit, frozen_at::text, frozen_by
		FROM frozen_targets
		WHERE category = $1
		ORDER BY metric, scenario
	`, string(category))
	if err != nil {
		return nil, err
	}
	defer rows.Close()

	var result []FrozenTarget
	for rows.Next() {
		var ft FrozenTarget
		if err := rows.Scan(&ft.TargetID, &ft.Category, &ft.Metric, &ft.Scenario, &ft.Value, &ft.Unit, &ft.FrozenAt, &ft.FrozenBy); err != nil {
			return nil, err
		}
		result = append(result, ft)
	}
	return result, rows.Err()
}
