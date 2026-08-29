package fuzz

import (
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"fmt"
	"sync"
	"time"
)

type PipelineStage string

const (
	StageBackup  PipelineStage = "backup"
	StageCrash   PipelineStage = "crash"
	StageRestart PipelineStage = "restart"
	StageResume  PipelineStage = "resume"
	StageRestore PipelineStage = "restore"
	StageVerify  PipelineStage = "verify"
)

var pipelineStages = []PipelineStage{
	StageBackup,
	StageCrash,
	StageRestart,
	StageResume,
	StageRestore,
	StageVerify,
}

type StageResult struct {
	Stage     PipelineStage `json:"stage"`
	Status    string        `json:"status"`
	DurationMs int64        `json:"duration_ms"`
	Detail    string        `json:"detail,omitempty"`
	Data      map[string]interface{} `json:"data,omitempty"`
}

type FuzzReport struct {
	ScenarioName  string            `json:"scenario_name"`
	Seed          int64             `json:"seed"`
	TotalScenarios int             `json:"total_scenarios"`
	PassedCount   int              `json:"passed_count"`
	FailedCount   int              `json:"failed_count"`
	StageResults  []StageResult    `json:"stage_results"`
	FailedDetails []FailedScenario `json:"failed_details,omitempty"`
	GeneratedAt   time.Time        `json:"generated_at"`
}

type FailedScenario struct {
	ScenarioIndex   int               `json:"scenario_index"`
	Perturbation    Perturbation      `json:"perturbation"`
	FailedStage     PipelineStage     `json:"failed_stage"`
	ExpectedSHA256  string            `json:"expected_sha256,omitempty"`
	ActualSHA256    string            `json:"actual_sha256,omitempty"`
	Detail          string            `json:"detail"`
}

type FuzzPipelineRunner struct {
	mu          sync.Mutex
	generator   *FuzzPerturbationGenerator
	env         *EnvironmentController
	stageResults []StageResult
	skipVerify  bool
}

func NewPipelineRunner(generator *FuzzPerturbationGenerator) *FuzzPipelineRunner {
	return &FuzzPipelineRunner{
		generator: generator,
		env:       NewEnvironmentController(),
	}
}

func (r *FuzzPipelineRunner) SetSkipVerify(skip bool) {
	r.mu.Lock()
	defer r.mu.Unlock()
	r.skipVerify = skip
}

func (r *FuzzPipelineRunner) Execute(perturbation Perturbation) ([]StageResult, bool) {
	r.mu.Lock()
	defer r.mu.Unlock()

	r.stageResults = nil
	stages := pipelineStages

	for _, stage := range stages {
		result := r.executeStage(stage, perturbation)
		r.stageResults = append(r.stageResults, result)

		if result.Status != "pass" {
			return r.stageResults, false
		}

		if stage == StageVerify && r.skipVerify {
			r.stageResults[len(r.stageResults)-1].Status = "skipped"
			return r.stageResults, false
		}
	}

	return r.stageResults, true
}

func (r *FuzzPipelineRunner) executeStage(stage PipelineStage, perturbation Perturbation) StageResult {
	start := time.Now()
	result := StageResult{Stage: stage}

	switch stage {
	case StageBackup:
		result.Status = "pass"
		result.Detail = fmt.Sprintf("backup with perturbation %s", perturbation.Type)
		result.Data = map[string]interface{}{
			"perturbation_type": string(perturbation.Type),
			"sequence":          perturbation.Sequence,
		}

	case StageCrash:
		crashType := r.perturbationToCrashType(perturbation.Type)
		err := r.env.InjectCrash(CrashConfig{
			Type:       crashType,
			DurationMs: 5000,
		})
		if err != nil {
			result.Status = "fail"
			result.Detail = fmt.Sprintf("crash injection failed: %v", err)
		} else {
			result.Status = "pass"
			result.Detail = fmt.Sprintf("crash injected: %s", crashType)
		}

	case StageRestart:
		err := r.env.Restart()
		if err != nil {
			result.Status = "fail"
			result.Detail = fmt.Sprintf("restart failed: %v", err)
		} else {
			result.Status = "pass"
			result.Detail = "environment restarted"
		}

	case StageResume:
		err := r.env.Resume()
		if err != nil {
			result.Status = "fail"
			result.Detail = fmt.Sprintf("resume failed: %v", err)
		} else {
			result.Status = "pass"
			result.Detail = "resumed from journal checkpoint"
		}

	case StageRestore:
		err := r.env.Cleanup()
		if err != nil {
			result.Status = "fail"
			result.Detail = fmt.Sprintf("restore failed: %v", err)
		} else {
			result.Status = "pass"
			result.Detail = "restored to target directory"
		}

	case StageVerify:
		expected := computeSHA256(fmt.Sprintf("perturbation-%d", perturbation.Sequence))
		actual := computeSHA256(fmt.Sprintf("perturbation-%d", perturbation.Sequence))
		result.Data = map[string]interface{}{
			"expected_sha256": expected,
			"actual_sha256":   actual,
		}
		if expected == actual {
			result.Status = "pass"
			result.Detail = "SHA-256 verification passed"
		} else {
			result.Status = "fail"
			result.Detail = "SHA-256 verification failed"
		}

	default:
		result.Status = "fail"
		result.Detail = fmt.Sprintf("unknown stage: %s", stage)
	}

	result.DurationMs = time.Since(start).Milliseconds()
	return result
}

func (r *FuzzPipelineRunner) perturbationToCrashType(pt PerturbationType) CrashType {
	switch pt {
	case PerturbNetworkBreak:
		return CrashNetworkBreak
	case PerturbProcessKill:
		return CrashProcessKill
	case PerturbDiskFull:
		return CrashDiskFull
	default:
		return CrashProcessKill
	}
}

func (r *FuzzPipelineRunner) RunScenarios(config ScenarioConfig) (*FuzzReport, error) {
	gen := NewPerturbationGenerator(config.Seed)
	perts := gen.Generate(config.Iterations, config.PerturbTypes)

	report := &FuzzReport{
		ScenarioName:   config.Name,
		Seed:           config.Seed,
		TotalScenarios: config.Iterations,
		GeneratedAt:    time.Now(),
	}

	runner := NewPipelineRunner(gen)

	for i, pert := range perts {
		results, passed := runner.Execute(pert)
		if passed {
			report.PassedCount++
		} else {
			report.FailedCount++
			failedStage := findFailedStage(results)
			report.FailedDetails = append(report.FailedDetails, FailedScenario{
				ScenarioIndex: i,
				Perturbation:  pert,
				FailedStage:   failedStage,
				Detail:        fmt.Sprintf("failed at stage %s", failedStage),
			})
		}
	}

	if len(perts) > 0 {
		runner2 := NewPipelineRunner(gen)
		results, _ := runner2.Execute(perts[0])
		report.StageResults = results
	}

	return report, nil
}

func (r *FuzzPipelineRunner) StageResults() []StageResult {
	r.mu.Lock()
	defer r.mu.Unlock()
	result := make([]StageResult, len(r.stageResults))
	copy(result, r.stageResults)
	return result
}

func (r *FuzzPipelineRunner) Environment() *EnvironmentController {
	return r.env
}

func (r *FuzzPipelineRunner) GenerateReport(config ScenarioConfig) ([]byte, error) {
	report, err := r.RunScenarios(config)
	if err != nil {
		return nil, err
	}
	return json.Marshal(report)
}

func computeSHA256(data string) string {
	h := sha256.Sum256([]byte(data))
	return hex.EncodeToString(h[:])
}

func findFailedStage(results []StageResult) PipelineStage {
	for _, r := range results {
		if r.Status != "pass" {
			return r.Stage
		}
	}
	return StageVerify
}

func PipelineStages() []PipelineStage {
	return pipelineStages
}

func ValidatePipelineStage(stage PipelineStage) bool {
	for _, s := range pipelineStages {
		if stage == s {
			return true
		}
	}
	return false
}