package soak

import (
	"context"
	"encoding/json"
	"fmt"
	"time"

	"hbx-control/internal/certorch/common"
)

type SoakReport struct {
	SessionID   string           `json:"session_id"`
	Duration    SoakDuration     `json:"duration"`
	Verdict     common.Verdict3  `json:"verdict"`
	Stability   StabilityVerdict `json:"stability"`
	Anomalies   []AnomalyEvent   `json:"anomalies"`
	Regression  common.RegResult `json:"regression"`
	GeneratedAt time.Time        `json:"generated_at"`
	Summary     string           `json:"summary"`
}

type SoakReportGenerator struct {
	archiver *common.CertReportArchiver
}

func NewSoakReportGenerator(archiver *common.CertReportArchiver) *SoakReportGenerator {
	return &SoakReportGenerator{archiver: archiver}
}

func (g *SoakReportGenerator) Generate(
	sessionID string,
	verdict StabilityVerdict,
	anomalies []AnomalyEvent,
	regression common.RegResult,
) (SoakReport, error) {
	overallVerdict := common.V3Pass
	if !verdict.AllPassed() {
		overallVerdict = common.V3Fail
	}
	if regression.Failed > 0 {
		overallVerdict = common.V3Fail
	}

	report := SoakReport{
		SessionID:   sessionID,
		Verdict:     overallVerdict,
		Stability:   verdict,
		Anomalies:   anomalies,
		Regression:  regression,
		GeneratedAt: time.Now().UTC(),
		Summary: fmt.Sprintf("Soak Test: %d stability metrics checked, %d anomalies, %d/%d regression tests passed",
			len(verdict.Metrics), len(anomalies), regression.Passed, regression.Total),
	}

	content, _ := json.Marshal(report)
	_, err := g.archiver.Archive(context.Background(), sessionID, common.GateG17Soak, overallVerdict, content, nil)
	if err != nil {
		return report, fmt.Errorf("archive report: %w", err)
	}

	return report, nil
}
