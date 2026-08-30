package common

import (
	"context"
	"encoding/json"
	"fmt"
	"regexp"
	"strings"
	"time"

	"github.com/google/uuid"
	"github.com/jackc/pgx/v5/pgxpool"
)

type CertGate string

const (
	GateG17Soak   CertGate = "G17_SOAK"
	GateG18Compat CertGate = "G18_COMPAT"
	GateG19Perf   CertGate = "G19_PERF"
	GateG20DR     CertGate = "G20_DR"
)

type EvidenceRef struct {
	Type string `json:"type"`
	Path string `json:"path"`
	Hash string `json:"hash"`
}

type CertReport struct {
	ReportID           string    `json:"report_id"`
	SessionID          string    `json:"session_id"`
	Gate               CertGate  `json:"gate"`
	Verdict            Verdict3  `json:"verdict"`
	Content            []byte    `json:"content"`
	EvidencePackageRef string    `json:"evidence_package_ref"`
	ArchivedAt         time.Time `json:"archived_at"`
}

var (
	archiverDSNPattern      = regexp.MustCompile(`postgres://[^@]+@`)
	archiverBearerPattern   = regexp.MustCompile(`Bearer\s+\S+`)
	archiverKeyPattern      = regexp.MustCompile(`-----BEGIN [A-Z ]*PRIVATE KEY-----[\s\S]*?-----END [A-Z ]*PRIVATE KEY-----`)
	archiverSensitiveFields = map[string]bool{
		"password": true, "passwd": true, "secret": true, "token": true,
		"authorization": true, "dsn": true, "private_key": true,
		"api_key": true, "jwt_secret": true, "db_password": true,
	}
	archiverLeakPatterns = []*regexp.Regexp{
		regexp.MustCompile(`postgres://[^@\[]+@`),
		regexp.MustCompile(`Bearer\s+[A-Za-z0-9\-_\.]{8,}`),
		regexp.MustCompile(`-----BEGIN [A-Z ]*PRIVATE KEY-----`),
	}
)

func redactContent(content []byte) []byte {
	s := string(content)
	s = archiverDSNPattern.ReplaceAllString(s, "postgres://[REDACTED]@")
	s = archiverBearerPattern.ReplaceAllString(s, "Bearer [REDACTED]")
	s = archiverKeyPattern.ReplaceAllString(s, "[REDACTED-KEY]")
	return []byte(s)
}

func redactJSONKeys(content []byte) []byte {
	var raw map[string]interface{}
	if err := json.Unmarshal(content, &raw); err != nil {
		return redactContent(content)
	}
	redactMapInPlace(raw)
	out, err := json.Marshal(raw)
	if err != nil {
		return redactContent(content)
	}
	return out
}

func redactMapInPlace(m map[string]interface{}) {
	for k, v := range m {
		if archiverSensitiveFields[strings.ToLower(k)] {
			m[k] = "[REDACTED]"
			continue
		}
		switch vv := v.(type) {
		case map[string]interface{}:
			redactMapInPlace(vv)
		case string:
			m[k] = string(redactContent([]byte(vv)))
		}
	}
}

func detectLeak(content []byte) bool {
	for _, p := range archiverLeakPatterns {
		if p.Match(content) {
			return true
		}
	}
	return false
}

type CertReportArchiver struct {
	pool *pgxpool.Pool
}

func NewCertReportArchiver(pool *pgxpool.Pool) *CertReportArchiver {
	return &CertReportArchiver{pool: pool}
}

func (a *CertReportArchiver) Archive(ctx context.Context, sessionID string, gate CertGate, verdict Verdict3, content []byte, evidence []EvidenceRef) (CertReport, error) {
	redacted := redactJSONKeys(content)
	redacted = redactContent(redacted)

	if detectLeak(redacted) {
		return CertReport{}, fmt.Errorf("%w: leak detected after redaction", ErrLeakDetected)
	}

	reportID := "rpt-" + uuid.NewString()
	now := time.Now().UTC()

	evidenceJSON, _ := json.Marshal(evidence)
	evidenceRef := string(evidenceJSON)

	_, err := a.pool.Exec(ctx, `
		INSERT INTO cert_reports (report_id, session_id, gate, verdict, content, evidence_package_ref, archived_at)
		VALUES ($1, $2, $3, $4, $5, $6, $7)
	`, reportID, sessionID, string(gate), string(verdict), redacted, evidenceRef, now)
	if err != nil {
		return CertReport{}, err
	}

	return CertReport{
		ReportID:           reportID,
		SessionID:          sessionID,
		Gate:               gate,
		Verdict:            verdict,
		Content:            redacted,
		EvidencePackageRef: evidenceRef,
		ArchivedAt:         now,
	}, nil
}
