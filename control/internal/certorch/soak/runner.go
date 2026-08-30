package soak


import (
	"context"
	"encoding/json"
	"fmt"
	"time"

	"hbx-control/internal/certorch/common"
)

type SoakDuration string

const (
	Duration24h SoakDuration = "24h"
	Duration72h SoakDuration = "72h"
	Duration7d  SoakDuration = "7d"
)

func (d SoakDuration) Duration() time.Duration {
	switch d {
	case Duration24h:
		return 24 * time.Hour
	case Duration72h:
		return 72 * time.Hour
	case Duration7d:
		return 7 * 24 * time.Hour
	default:
		return 0
	}
}

func (d SoakDuration) Valid() bool {
	return d == Duration24h || d == Duration72h || d == Duration7d
}

type SoakStartRequest struct {
	Duration SoakDuration `json:"duration"`
	Operator string       `json:"operator"`
}

type AnomalyEvent struct {
	Timestamp time.Time `json:"timestamp"`
	Component string    `json:"component"`
	Type      string    `json:"type"`
	Detail    string    `json:"detail"`
}

type SoakTestRunner struct {
	loadGen       *LoadGenerator
	stability     *StabilityAnalyzer
	regression    *common.RegressionRunner
	reportGen     *SoakReportGenerator
	freezer       *common.FrozenTargetFreezer
	nottestedReg *common.NotTestedReasonRegistry
}

func NewSoakTestRunner(
	loadGen *LoadGenerator,
	stability *StabilityAnalyzer,
	regression *common.RegressionRunner,
	reportGen *SoakReportGenerator,
	freezer *common.FrozenTargetFreezer,
	nottestedReg *common.NotTestedReasonRegistry,
) *SoakTestRunner {
	return &SoakTestRunner{
		loadGen:       loadGen,
		stability:     stability,
		regression:    regression,
		reportGen:     reportGen,
		freezer:       freezer,
		nottestedReg: nottestedReg,
	}
}

func (r *SoakTestRunner) Run(ctx context.Context, sessionID string, req json.RawMessage) error {
	var startReq SoakStartRequest
	if err := json.Unmarshal(req, &startReq); err != nil {
		return fmt.Errorf("unmarshal soak request: %w", err)
	}

	if !startReq.Duration.Valid() {
		return fmt.Errorf("invalid duration: %s", startReq.Duration)
	}

	if startReq.Duration == Duration7d {
		err := r.nottestedReg.Register(ctx, sessionID, "soak_7d", "7d exclusive environment not available", "7d dedicated hardware")
		if err != nil {
			return err
		}
		return fmt.Errorf("7d soak test requires dedicated environment, marked NOT_TESTED")
	}

	startTime := time.Now()

	loadCtx, loadCancel := context.WithCancel(ctx)
	defer loadCancel()

	if err := r.loadGen.Start(loadCtx, LoadPattern{
		BackupInterval:   30 * time.Minute,
		IncrementalInterval: 15 * time.Minute,
		RestoreInterval:  2 * time.Hour,
	}); err != nil {
		return fmt.Errorf("start load generator: %w", err)
	}

	samples := r.loadGen.CollectSamples(ctx, startReq.Duration.Duration())

	actualDuration := time.Since(startTime)
	if actualDuration < startReq.Duration.Duration() {
		return fmt.Errorf("soak test ran for %v, less than required %v, marked FAIL", actualDuration, startReq.Duration.Duration())
	}

	verdict := r.stability.Analyze(samples)

	regResult, err := r.regression.RunRegression(ctx)
	if err != nil {
		return fmt.Errorf("regression test failed: %w", err)
	}

	anomalies := r.loadGen.GetAnomalies()

	_, err = r.reportGen.Generate(sessionID, verdict, anomalies, regResult)
	if err != nil {
		return fmt.Errorf("generate report: %w", err)
	}

	if verdict.AllPassed() && regResult.Failed == 0 {
		thresholds := r.stability.ExtractThresholds(verdict)
		if err := r.freezer.FreezeStabilityThresholds(ctx, sessionID, thresholds, startReq.Operator); err != nil {
			return fmt.Errorf("freeze thresholds: %w", err)
		}
	}

	return nil
}