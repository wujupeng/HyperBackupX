package audit

import (
	"context"
	"log/slog"
	"time"

	"github.com/google/uuid"
	"github.com/jackc/pgx/v5/pgxpool"
)

type ActorType string

const (
	ActorTypeUser   ActorType = "user"
	ActorTypeSystem ActorType = "system"
)

type Entry struct {
	ActorID    string
	ActorType  ActorType
	Action     string
	TargetType string
	TargetID   string
	Result     string
	TraceID    string
	Detail     map[string]interface{}
}

type Logger struct {
	pool *pgxpool.Pool
}

func NewLogger(pool *pgxpool.Pool) *Logger {
	return &Logger{pool: pool}
}

func (l *Logger) Record(ctx context.Context, entry Entry) {
	if l.pool == nil {
		slog.Info("audit log",
			"actor", entry.ActorID,
			"action", entry.Action,
			"target", entry.TargetType+"/"+entry.TargetID,
			"result", entry.Result,
		)
		return
	}

	logID := uuid.New()
	_, err := l.pool.Exec(ctx, `
		INSERT INTO audit_logs (log_id, actor_id, actor_type, action, target_type, target_id, result, timestamp, trace_id, detail)
		VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
	`, logID, entry.ActorID, string(entry.ActorType), entry.Action,
		entry.TargetType, entry.TargetID, entry.Result, time.Now().UTC(),
		entry.TraceID, entry.Detail,
	)
	if err != nil {
		slog.Error("failed to write audit log", "error", err, "action", entry.Action)
	}
}