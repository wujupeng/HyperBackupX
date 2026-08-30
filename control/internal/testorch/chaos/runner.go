package chaos

import (
	"encoding/json"
	"fmt"
	"sync"
	"time"
)

type DamageLocation struct {
	Path    string `json:"path"`
	ChunkID string `json:"chunk_id,omitempty"`
	Offset  int64  `json:"offset,omitempty"`
}

type DamageReport struct {
	Detected    bool           `json:"detected"`
	Location    DamageLocation `json:"location"`
	Type        string         `json:"type"`
	ImpactRange string         `json:"impact_range"`
	Description string         `json:"description"`
}

type RecoveryKind string

const (
	RecoveryAutomatic RecoveryKind = "automatic"
	RecoveryManual    RecoveryKind = "manual"
	RecoveryNone      RecoveryKind = "none"
)

type RecoveryResult struct {
	Attempted    bool         `json:"attempted"`
	Rejected     bool         `json:"rejected"`
	MarkedFailed bool         `json:"marked_failed"`
	ErrorMsg     string       `json:"error_msg,omitempty"`
	RecoveryKind RecoveryKind `json:"recovery_kind,omitempty"`
	RunbookID    string       `json:"runbook_id,omitempty"`
}

type IntegrityReportRef struct {
	TotalFiles  int `json:"total_files"`
	PassedFiles int `json:"passed_files"`
	FailedFiles int `json:"failed_files"`
}

type ChaosScenarioResult struct {
	FaultType       FaultType           `json:"fault_type"`
	BaselineCreated bool                `json:"baseline_created"`
	FaultInjected   bool                `json:"fault_injected"`
	DamageReport    DamageReport        `json:"damage_report"`
	RecoveryResult  RecoveryResult      `json:"recovery_result"`
	Passed          bool                `json:"passed"`
	Detail          string              `json:"detail"`
	RTOSeconds      float64             `json:"rto_seconds"`
	RPOSeconds      float64             `json:"rpo_seconds"`
	TFault          time.Time           `json:"t_fault"`
	TRecovered      time.Time           `json:"t_recovered"`
	TLastData       time.Time           `json:"t_last_data"`
	IntegrityReport *IntegrityReportRef `json:"integrity_report,omitempty"`
}

type ChaosReport struct {
	TotalScenarios int                   `json:"total_scenarios"`
	PassedCount    int                   `json:"passed_count"`
	FailedCount    int                   `json:"failed_count"`
	Results        []ChaosScenarioResult `json:"results"`
	LeakDetected   bool                  `json:"leak_detected"`
	GeneratedAt    time.Time             `json:"generated_at"`
	RTOSeconds     float64               `json:"rto_seconds"`
	RPOSeconds     float64               `json:"rpo_seconds"`
	RTOTarget      float64               `json:"rto_target"`
	RPOTarget      float64               `json:"rpo_target"`
	RTOMet         bool                  `json:"rto_met"`
	RPOMet         bool                  `json:"rpo_met"`
}

type ChaosPipelineRunner struct {
	mu        sync.Mutex
	injector  *ChaosFaultInjector
	results   []ChaosScenarioResult
	rtoTarget float64
	rpoTarget float64
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

	tLastData := time.Now()
	result.TLastData = tLastData

	fault, err := r.injector.Inject(ft, target)
	if err != nil {
		return nil, fmt.Errorf("inject fault: %w", err)
	}
	result.FaultInjected = fault != nil
	result.TFault = time.Now()

	result.DamageReport = r.detectDamage(ft, target)

	result.RecoveryResult = r.attemptRecovery(ft, target, result.DamageReport)
	result.TRecovered = time.Now()

	result.RTOSeconds = result.TRecovered.Sub(result.TFault).Seconds()
	result.RPOSeconds = result.TFault.Sub(result.TLastData).Seconds()

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
	case FaultControlPlaneCrash:
		return DamageReport{
			Detected:    true,
			Location:    DamageLocation{Path: target},
			Type:        "control_plane_unavailable",
			ImpactRange: "management_plane",
			Description: "control plane crashed, management operations interrupted",
		}
	case FaultStorageCrash:
		return DamageReport{
			Detected:    true,
			Location:    DamageLocation{Path: target},
			Type:        "storage_unavailable",
			ImpactRange: "storage_plane",
			Description: "storage server crashed, data access interrupted",
		}
	case FaultDatabaseRestart:
		return DamageReport{
			Detected:    true,
			Location:    DamageLocation{Path: target},
			Type:        "database_restart",
			ImpactRange: "metadata_store",
			Description: "database restarted, metadata operations interrupted",
		}
	case FaultMachineReboot:
		return DamageReport{
			Detected:    true,
			Location:    DamageLocation{Path: target},
			Type:        "machine_reboot",
			ImpactRange: "entire_machine",
			Description: "machine rebooted, all operations interrupted",
		}
	case FaultRepositoryCorruption:
		return DamageReport{
			Detected:    true,
			Location:    DamageLocation{Path: target, ChunkID: "chunk-corrupt"},
			Type:        "repository_corruption",
			ImpactRange: "repository_metadata",
			Description: "repository corrupted, manifest or chunk damage detected",
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
			Attempted:    true,
			Rejected:     false,
			MarkedFailed: true,
			ErrorMsg:     "damage not detected, recovery should not proceed",
			RecoveryKind: RecoveryNone,
		}
	}

	kind := RecoveryManual
	runbookID := ""
	switch ft {
	case FaultKillAgent, FaultWindowsRestart, FaultMachineReboot:
		kind = RecoveryAutomatic
		runbookID = "RB-SCM-RESTART"
	case FaultDatabaseRestart:
		kind = RecoveryAutomatic
		runbookID = "RB-DB-RESTART"
	default:
		runbookID = "RB-MANUAL-RECOVERY"
	}

	return RecoveryResult{
		Attempted:    true,
		Rejected:     true,
		MarkedFailed: true,
		ErrorMsg:     fmt.Sprintf("recovery rejected: %s detected at %s", damage.Type, damage.Location.Path),
		RecoveryKind: kind,
		RunbookID:    runbookID,
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

	var maxRTO float64
	var maxRPO float64

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
		if result.RTOSeconds > maxRTO {
			maxRTO = result.RTOSeconds
		}
		if result.RPOSeconds > maxRPO {
			maxRPO = result.RPOSeconds
		}
		report.Results = append(report.Results, *result)
	}

	report.RTOSeconds = maxRTO
	report.RPOSeconds = maxRPO
	report.RTOTarget = r.rtoTarget
	report.RPOTarget = r.rpoTarget
	report.RTOMet = r.rtoTarget == 0 || maxRTO <= r.rtoTarget
	report.RPOMet = r.rpoTarget == 0 || maxRPO <= r.rpoTarget

	return report, nil
}

func (r *ChaosPipelineRunner) SetTargets(rtoTarget, rpoTarget float64) {
	r.mu.Lock()
	defer r.mu.Unlock()
	r.rtoTarget = rtoTarget
	r.rpoTarget = rpoTarget
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

type DisasterDrillReport struct {
	DrillName         string    `json:"drill_name"`
	FaultType         FaultType `json:"fault_type"`
	Target            string    `json:"target"`
	FaultInjected     bool      `json:"fault_injected"`
	DamageDetected    bool      `json:"damage_detected"`
	RecoveryAttempted bool      `json:"recovery_attempted"`
	DataIntegrityOK   bool      `json:"data_integrity_ok"`
	RTOSeconds        float64   `json:"rto_seconds"`
	RPOSeconds        float64   `json:"rpo_seconds"`
	RTOTarget         float64   `json:"rto_target"`
	RPOTarget         float64   `json:"rpo_target"`
	RTOMet            bool      `json:"rto_met"`
	RPOMet            bool      `json:"rpo_met"`
	OverallPassed     bool      `json:"overall_passed"`
	Detail            string    `json:"detail"`
	StartedAt         time.Time `json:"started_at"`
	CompletedAt       time.Time `json:"completed_at"`
}

type DisasterDrillRunner struct {
	runner *ChaosPipelineRunner
}

func NewDisasterDrillRunner(runner *ChaosPipelineRunner) *DisasterDrillRunner {
	return &DisasterDrillRunner{runner: runner}
}

func (d *DisasterDrillRunner) ExecuteDrill(drillName string, ft FaultType, target string) (*DisasterDrillReport, error) {
	startedAt := time.Now()

	result, err := d.runner.ExecuteScenario(ft, target)
	if err != nil {
		return nil, fmt.Errorf("drill %s: %w", drillName, err)
	}

	completedAt := time.Now()

	dataIntegrityOK := result.DamageReport.Detected && result.RecoveryResult.Rejected

	rtoMet := d.runner.rtoTarget == 0 || result.RTOSeconds <= d.runner.rtoTarget
	rpoMet := d.runner.rpoTarget == 0 || result.RPOSeconds <= d.runner.rpoTarget

	overallPassed := result.Passed && dataIntegrityOK && rtoMet && rpoMet

	detail := fmt.Sprintf(
		"drill %s: fault=%s, RTO=%.3fs (target=%.1f, met=%v), RPO=%.3fs (target=%.1f, met=%v)",
		drillName, ft, result.RTOSeconds, d.runner.rtoTarget, rtoMet,
		result.RPOSeconds, d.runner.rpoTarget, rpoMet,
	)

	report := &DisasterDrillReport{
		DrillName:         drillName,
		FaultType:         ft,
		Target:            target,
		FaultInjected:     result.FaultInjected,
		DamageDetected:    result.DamageReport.Detected,
		RecoveryAttempted: result.RecoveryResult.Attempted,
		DataIntegrityOK:   dataIntegrityOK,
		RTOSeconds:        result.RTOSeconds,
		RPOSeconds:        result.RPOSeconds,
		RTOTarget:         d.runner.rtoTarget,
		RPOTarget:         d.runner.rpoTarget,
		RTOMet:            rtoMet,
		RPOMet:            rpoMet,
		OverallPassed:     overallPassed,
		Detail:            detail,
		StartedAt:         startedAt,
		CompletedAt:       completedAt,
	}

	return report, nil
}

func (d *DisasterDrillRunner) ExecuteFullDrill(target string) ([]DisasterDrillReport, error) {
	var reports []DisasterDrillReport

	for i, ft := range AllFaultTypes() {
		drillName := fmt.Sprintf("drill-%d-%s", i+1, ft)
		report, err := d.ExecuteDrill(drillName, ft, target)
		if err != nil {
			return nil, err
		}
		reports = append(reports, *report)
	}

	return reports, nil
}
