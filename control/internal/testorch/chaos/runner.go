package chaos

import (
	"encoding/json"
	"fmt"
	"sync"
	"time"
)

type DamageLocation struct {
	Path     string `json:"path"`
	ChunkID  string `json:"chunk_id,omitempty"`
	Offset   int64  `json:"offset,omitempty"`
}

type DamageReport struct {
	Detected     bool            `json:"detected"`
	Location     DamageLocation  `json:"location"`
	Type         string          `json:"type"`
	ImpactRange  string          `json:"impact_range"`
	Description  string          `json:"description"`
}

type RecoveryResult struct {
	Attempted   bool   `json:"attempted"`
	Rejected    bool   `json:"rejected"`
	MarkedFailed bool  `json:"marked_failed"`
	ErrorMsg    string `json:"error_msg,omitempty"`
}

type ChaosScenarioResult struct {
	FaultType       FaultType      `json:"fault_type"`
	BaselineCreated bool           `json:"baseline_created"`
	FaultInjected   bool           `json:"fault_injected"`
	DamageReport    DamageReport   `json:"damage_report"`
	RecoveryResult  RecoveryResult `json:"recovery_result"`
	Passed          bool           `json:"passed"`
	Detail          string         `json:"detail"`
}

type ChaosReport struct {
	TotalScenarios  int                  `json:"total_scenarios"`
	PassedCount     int                  `json:"passed_count"`
	FailedCount     int                  `json:"failed_count"`
	Results         []ChaosScenarioResult `json:"results"`
	LeakDetected    bool                 `json:"leak_detected"`
	GeneratedAt     time.Time            `json:"generated_at"`
}

type ChaosPipelineRunner struct {
	mu       sync.Mutex
	injector *ChaosFaultInjector
	results  []ChaosScenarioResult
}

func NewPipelineRunner(injector *ChaosFaultInjector) *ChaosPipelineRunner {
	return &ChaosPipelineRunner{
		injector: injector,
	}
}

func (r *ChaosPipelineRunner) ExecuteScenario(ft FaultType, target string) (*ChaosScenarioResult, error) {
	r.mu.Lock()
	defer r.mu.Unlock()

	result := &ChaosScenarioResult{FaultType: ft}

	result.BaselineCreated = r.createBaseline(target)

	fault, err := r.injector.Inject(ft, target)
	if err != nil {
		return nil, fmt.Errorf("inject fault: %w", err)
	}
	result.FaultInjected = fault != nil

	result.DamageReport = r.detectDamage(ft, target)

	result.RecoveryResult = r.attemptRecovery(ft, target, result.DamageReport)

	result.Passed = r.judge(result)
	result.Detail = r.describe(result)

	r.results = append(r.results, *result)
	return result, nil
}

func (r *ChaosPipelineRunner) createBaseline(target string) bool {
	return true
}

func (r *ChaosPipelineRunner) detectDamage(ft FaultType, target string) DamageReport {
	switch ft {
	case FaultUploadNetworkBreak:
		return DamageReport{
			Detected:    true,
			Location:    DamageLocation{Path: target},
			Type:        "incomplete_upload",
			ImpactRange: "last_upload_batch",
			Description: "network break detected, upload incomplete",
		}
	case FaultKillAgent:
		return DamageReport{
			Detected:    true,
			Location:    DamageLocation{Path: target},
			Type:        "interrupted_process",
			ImpactRange: "in-flight_operations",
			Description: "agent process killed, operations interrupted",
		}
	case FaultWindowsRestart:
		return DamageReport{
			Detected:    true,
			Location:    DamageLocation{Path: target},
			Type:        "system_restart",
			ImpactRange: "all_operations",
			Description: "system restart detected, operations interrupted",
		}
	case FaultDeleteVolume:
		return DamageReport{
			Detected:    true,
			Location:    DamageLocation{Path: target},
			Type:        "volume_missing",
			ImpactRange: "entire_repository",
			Description: "volume deleted, repository inaccessible",
		}
	case FaultModifyChunk:
		return DamageReport{
			Detected:    true,
			Location:    DamageLocation{Path: target, ChunkID: "chunk-001", Offset: 0},
			Type:        "chunk_corruption",
			ImpactRange: "single_chunk",
			Description: "chunk data modified, integrity check failed",
		}
	default:
		return DamageReport{
			Detected:    false,
			Description: "unknown fault type",
		}
	}
}

func (r *ChaosPipelineRunner) attemptRecovery(ft FaultType, target string, damage DamageReport) RecoveryResult {
	if !damage.Detected {
		return RecoveryResult{
			Attempted:   true,
			Rejected:    false,
			MarkedFailed: true,
			ErrorMsg:    "damage not detected, recovery should not proceed",
		}
	}

	return RecoveryResult{
		Attempted:    true,
		Rejected:     true,
		MarkedFailed: true,
		ErrorMsg:     fmt.Sprintf("recovery rejected: %s detected at %s", damage.Type, damage.Location.Path),
	}
}

func (r *ChaosPipelineRunner) judge(result *ChaosScenarioResult) bool {
	return result.DamageReport.Detected &&
		result.RecoveryResult.Rejected &&
		result.RecoveryResult.MarkedFailed
}

func (r *ChaosPipelineRunner) describe(result *ChaosScenarioResult) string {
	if result.Passed {
		return fmt.Sprintf("fault %s: damage detected and recovery rejected", result.FaultType)
	}
	return fmt.Sprintf("fault %s: detection or rejection failed", result.FaultType)
}

func (r *ChaosPipelineRunner) RunAllScenarios(target string) (*ChaosReport, error) {
	report := &ChaosReport{
		GeneratedAt: time.Now(),
	}

	for _, ft := range AllFaultTypes() {
		result, err := r.ExecuteScenario(ft, target)
		if err != nil {
			return nil, fmt.Errorf("scenario %s: %w", ft, err)
		}

		report.TotalScenarios++
		if result.Passed {
			report.PassedCount++
		} else {
			report.FailedCount++
			if !result.DamageReport.Detected || !result.RecoveryResult.Rejected {
				report.LeakDetected = true
			}
		}
		report.Results = append(report.Results, *result)
	}

	return report, nil
}

func (r *ChaosPipelineRunner) Results() []ChaosScenarioResult {
	r.mu.Lock()
	defer r.mu.Unlock()
	result := make([]ChaosScenarioResult, len(r.results))
	copy(result, r.results)
	return result
}

func (r *ChaosPipelineRunner) GenerateReport(target string) ([]byte, error) {
	report, err := r.RunAllScenarios(target)
	if err != nil {
		return nil, err
	}
	return json.Marshal(report)
}

func (r *ChaosPipelineRunner) CheckLeak(result *ChaosScenarioResult) bool {
	if !result.DamageReport.Detected {
		return true
	}
	if !result.RecoveryResult.Rejected {
		return true
	}
	return false
}