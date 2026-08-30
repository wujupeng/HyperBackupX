package common

import (
	"context"


	"github.com/google/uuid"
	"github.com/jackc/pgx/v5/pgxpool"
)

type NotTestedReason struct {
	ReasonID        string `json:"reason_id"`
	SessionID       string `json:"session_id"`
	Item            string `json:"item"`
	Cause           string `json:"cause"`
	RequiredResource string `json:"required_resource"`
	CreatedAt       string `json:"created_at"`
}

type NotTestedReasonRegistry struct {
	pool *pgxpool.Pool
}

func NewNotTestedReasonRegistry(pool *pgxpool.Pool) *NotTestedReasonRegistry {
	return &NotTestedReasonRegistry{pool: pool}
}

func (r *NotTestedReasonRegistry) Register(ctx context.Context, sessionID, item, cause, requiredResource string) error {
	reasonID := "ntr-" + uuid.NewString()
	_, err := r.pool.Exec(ctx, `
		INSERT INTO not_tested_reasons (reason_id, session_id, item, cause, required_resource)
		VALUES ($1, $2, $3, $4, $5)
	`, reasonID, sessionID, item, cause, requiredResource)
	return err
}

func (r *NotTestedReasonRegistry) ListBySession(ctx context.Context, sessionID string) ([]NotTestedReason, error) {
	rows, err := r.pool.Query(ctx, `
		SELECT reason_id, session_id, item, cause, required_resource, created_at::text
		FROM not_tested_reasons
		WHERE session_id = $1
		ORDER BY created_at
	`, sessionID)
	if err != nil {
		return nil, err
	}
	defer rows.Close()

	var result []NotTestedReason
	for rows.Next() {
		var ntr NotTestedReason
		if err := rows.Scan(&ntr.ReasonID, &ntr.SessionID, &ntr.Item, &ntr.Cause, &ntr.RequiredResource, &ntr.CreatedAt); err != nil {
			return nil, err
		}
		result = append(result, ntr)
	}
	return result, rows.Err()
}
