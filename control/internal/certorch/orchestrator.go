package certorch

import (
	"context"
	"encoding/json"
	"fmt"
	"sync"
	"time"

	"github.com/google/uuid"
	"github.com/jackc/pgx/v5/pgxpool"

	"hbx-control/internal/certorch/common"
)

type CertSession struct {
	SessionID   string          `json:"session_id"`
	Gate        common.CertGate `json:"gate"`
	Status      string          `json:"status"`
	Operator    string          `json:"operator"`
	StartedAt   time.Time       `json:"started_at"`
	CompletedAt *time.Time      `json:"completed_at,omitempty"`
	Detail      json.RawMessage `json:"detail,omitempty"`
}

type GateRunner interface {
	Run(ctx context.Context, sessionID string, req json.RawMessage) error
}

type CertOrchestrator struct {
	pool           *pgxpool.Pool
	runners        map[common.CertGate]GateRunner
	mu             sync.Mutex
	activeSessions map[common.CertGate]string
}

func NewCertOrchestrator(pool *pgxpool.Pool) *CertOrchestrator {
	return &CertOrchestrator{
		pool:           pool,
		runners:        make(map[common.CertGate]GateRunner),
		activeSessions: make(map[common.CertGate]string),
	}
}

func (o *CertOrchestrator) RegisterRunner(gate common.CertGate, runner GateRunner) {
	o.mu.Lock()
	defer o.mu.Unlock()
	o.runners[gate] = runner
}

func (o *CertOrchestrator) StartGate(ctx context.Context, gate common.CertGate, operator string, req json.RawMessage) (string, error) {
	o.mu.Lock()
	runner, ok := o.runners[gate]
	if !ok {
		o.mu.Unlock()
		return "", common.ErrGateNotFound
	}
	if _, active := o.activeSessions[gate]; active {
		o.mu.Unlock()
		return "", common.ErrSessionAlreadyActive
	}
	o.mu.Unlock()

	sessionID := "cert-" + uuid.NewString()
	now := time.Now().UTC()

	_, err := o.pool.Exec(ctx, `
		INSERT INTO cert_sessions (session_id, gate, status, operator, started_at)
		VALUES ($1, $2, 'running', $3, $4)
	`, sessionID, string(gate), operator, now)
	if err != nil {
		return "", err
	}

	o.mu.Lock()
	o.activeSessions[gate] = sessionID
	o.mu.Unlock()

	go func() {
		bgCtx := context.Background()
		err := runner.Run(bgCtx, sessionID, req)
		o.CompleteSession(bgCtx, sessionID, err)
	}()

	return sessionID, nil
}

func (o *CertOrchestrator) CompleteSession(ctx context.Context, sessionID string, runErr error) {
	now := time.Now().UTC()
	status := "completed"
	var detail json.RawMessage
	if runErr != nil {
		status = "failed"
		detail, _ = json.Marshal(map[string]string{"error": runErr.Error()})
	}

	_, _ = o.pool.Exec(ctx, `
		UPDATE cert_sessions SET status = $1, completed_at = $2, detail = COALESCE($3, detail)
		WHERE session_id = $4
	`, status, now, detail, sessionID)

	o.mu.Lock()
	for gate, sid := range o.activeSessions {
		if sid == sessionID {
			delete(o.activeSessions, gate)
		}
	}
	o.mu.Unlock()
}

func (o *CertOrchestrator) QuerySession(ctx context.Context, sessionID string) (CertSession, error) {
	var s CertSession
	var gateStr, statusStr string
	var detail []byte
	err := o.pool.QueryRow(ctx, `
		SELECT session_id, gate, status, operator, started_at, completed_at, detail
		FROM cert_sessions WHERE session_id = $1
	`, sessionID).Scan(&s.SessionID, &gateStr, &statusStr, &s.Operator, &s.StartedAt, &s.CompletedAt, &detail)
	if err != nil {
		return CertSession{}, fmt.Errorf("%w: %s", common.ErrSessionNotFound, sessionID)
	}
	s.Gate = common.CertGate(gateStr)
	s.Status = statusStr
	if detail != nil {
		s.Detail = json.RawMessage(detail)
	}
	return s, nil
}

func (o *CertOrchestrator) ListSessions(ctx context.Context) ([]CertSession, error) {
	rows, err := o.pool.Query(ctx, `
		SELECT session_id, gate, status, operator, started_at, completed_at, detail
		FROM cert_sessions ORDER BY started_at DESC
	`)
	if err != nil {
		return nil, err
	}
	defer rows.Close()

	var result []CertSession
	for rows.Next() {
		var s CertSession
		var gateStr, statusStr string
		var detail []byte
		if err := rows.Scan(&s.SessionID, &gateStr, &statusStr, &s.Operator, &s.StartedAt, &s.CompletedAt, &detail); err != nil {
			return nil, err
		}
		s.Gate = common.CertGate(gateStr)
		s.Status = statusStr
		if detail != nil {
			s.Detail = json.RawMessage(detail)
		}
		result = append(result, s)
	}
	return result, rows.Err()
}

func (o *CertOrchestrator) DownloadReport(ctx context.Context, sessionID string) (common.CertReport, error) {
	var rpt common.CertReport
	var gateStr, verdictStr string
	var content []byte
	err := o.pool.QueryRow(ctx, `
		SELECT report_id, session_id, gate, verdict, content, evidence_package_ref, archived_at
		FROM cert_reports WHERE session_id = $1 ORDER BY archived_at DESC LIMIT 1
	`, sessionID).Scan(&rpt.ReportID, &rpt.SessionID, &gateStr, &verdictStr, &content, &rpt.EvidencePackageRef, &rpt.ArchivedAt)
	if err != nil {
		return common.CertReport{}, fmt.Errorf("%w: no report for session %s", common.ErrSessionNotFound, sessionID)
	}
	rpt.Gate = common.CertGate(gateStr)
	rpt.Verdict = common.Verdict3(verdictStr)
	rpt.Content = content
	return rpt, nil
}
