package fuzz

import (
	"testing"
)

func TestPipelineSixStages(t *testing.T) {
	gen := NewPerturbationGenerator(42)
	runner := NewPipelineRunner(gen)

	pert := Perturbation{Type: PerturbFileContent, Sequence: 0}
	results, passed := runner.Execute(pert)

	if len(results) != 6 {
		t.Fatalf("expected 6 stages, got %d", len(results))
	}

	expectedStages := []PipelineStage{StageBackup, StageCrash, StageRestart, StageResume, StageRestore, StageVerify}
	for i, expected := range expectedStages {
		if results[i].Stage != expected {
			t.Errorf("stage %d: expected %s, got %s", i, expected, results[i].Stage)
		}
	}

	if !passed {
		t.Error("pipeline should pass for valid perturbation")
	}
}

func TestPipelineStageOrder(t *testing.T) {
	stages := PipelineStages()
	if len(stages) != 6 {
		t.Fatalf("expected 6 stages, got %d", len(stages))
	}

	expected := []PipelineStage{StageBackup, StageCrash, StageRestart, StageResume, StageRestore, StageVerify}
	for i, s := range expected {
		if stages[i] != s {
			t.Errorf("stage %d: expected %s, got %s", i, s, stages[i])
		}
	}
}

func TestPipelineVerifyNotSkippable(t *testing.T) {
	gen := NewPerturbationGenerator(42)
	runner := NewPipelineRunner(gen)
	runner.SetSkipVerify(true)

	pert := Perturbation{Type: PerturbFileContent, Sequence: 0}
	_, passed := runner.Execute(pert)

	if passed {
		t.Error("pipeline should fail when verify is skipped")
	}

	results := runner.StageResults()
	if results[5].Status != "skipped" {
		t.Errorf("verify stage should be skipped, got %s", results[5].Status)
	}
}

func TestPipelineVerifySHA256(t *testing.T) {
	gen := NewPerturbationGenerator(42)
	runner := NewPipelineRunner(gen)

	pert := Perturbation{Type: PerturbFileContent, Sequence: 1}
	results, _ := runner.Execute(pert)

	verifyResult := results[5]
	if verifyResult.Stage != StageVerify {
		t.Fatalf("expected verify stage, got %s", verifyResult.Stage)
	}

	expected, ok := verifyResult.Data["expected_sha256"].(string)
	if !ok {
		t.Fatal("expected_sha256 not found in verify result")
	}
	actual, ok := verifyResult.Data["actual_sha256"].(string)
	if !ok {
		t.Fatal("actual_sha256 not found in verify result")
	}

	if expected == "" || actual == "" {
		t.Error("SHA-256 values should not be empty")
	}

	if expected != actual {
		t.Error("SHA-256 values should match for same input")
	}
}

func TestPipelineAllPerturbationTypes(t *testing.T) {
	gen := NewPerturbationGenerator(42)
	runner := NewPipelineRunner(gen)

	for _, pt := range AllPerturbationTypes() {
		runner.Environment().Reset()
		pert := Perturbation{Type: pt, Sequence: 0}
		results, passed := runner.Execute(pert)

		if len(results) != 6 {
			t.Errorf("perturbation %s: expected 6 stages, got %d", pt, len(results))
		}
		if !passed {
			t.Errorf("perturbation %s: pipeline should pass", pt)
		}
	}
}

func TestPipelineStageResults(t *testing.T) {
	gen := NewPerturbationGenerator(42)
	runner := NewPipelineRunner(gen)

	pert := Perturbation{Type: PerturbFileContent, Sequence: 0}
	runner.Execute(pert)

	results := runner.StageResults()
	if len(results) != 6 {
		t.Fatalf("expected 6 stage results, got %d", len(results))
	}

	for _, r := range results {
		if r.DurationMs < 0 {
			t.Errorf("stage %s has negative duration: %d", r.Stage, r.DurationMs)
		}
	}
}

func TestRunScenarios(t *testing.T) {
	gen := NewPerturbationGenerator(42)
	runner := NewPipelineRunner(gen)

	config := ScenarioConfig{
		Name:       "test-fuzz",
		Seed:       42,
		Iterations: 10,
	}

	report, err := runner.RunScenarios(config)
	if err != nil {
		t.Fatalf("RunScenarios failed: %v", err)
	}

	if report.TotalScenarios != 10 {
		t.Errorf("expected 10 total scenarios, got %d", report.TotalScenarios)
	}

	if report.PassedCount+report.FailedCount != 10 {
		t.Errorf("passed + failed should equal total: %d + %d != %d", report.PassedCount, report.FailedCount, report.TotalScenarios)
	}
}

func TestRunScenariosAllPass(t *testing.T) {
	gen := NewPerturbationGenerator(42)
	runner := NewPipelineRunner(gen)

	config := ScenarioConfig{
		Name:       "test-fuzz-all-pass",
		Seed:       42,
		Iterations: 20,
	}

	report, err := runner.RunScenarios(config)
	if err != nil {
		t.Fatalf("RunScenarios failed: %v", err)
	}

	if report.FailedCount != 0 {
		t.Errorf("expected 0 failures, got %d", report.FailedCount)
	}
	if report.PassedCount != 20 {
		t.Errorf("expected 20 passes, got %d", report.PassedCount)
	}
}

func TestGenerateReport(t *testing.T) {
	gen := NewPerturbationGenerator(42)
	runner := NewPipelineRunner(gen)

	config := ScenarioConfig{
		Name:       "test-report",
		Seed:       42,
		Iterations: 5,
	}

	data, err := runner.GenerateReport(config)
	if err != nil {
		t.Fatalf("GenerateReport failed: %v", err)
	}
	if len(data) == 0 {
		t.Error("report data should not be empty")
	}
}

func TestValidatePipelineStage(t *testing.T) {
	if !ValidatePipelineStage(StageBackup) {
		t.Error("StageBackup should be valid")
	}
	if !ValidatePipelineStage(StageVerify) {
		t.Error("StageVerify should be valid")
	}
	if ValidatePipelineStage(PipelineStage("unknown")) {
		t.Error("unknown stage should be invalid")
	}
}

func TestPipelineEnvironmentController(t *testing.T) {
	gen := NewPerturbationGenerator(42)
	runner := NewPipelineRunner(gen)

	env := runner.Environment()
	if env == nil {
		t.Fatal("environment controller should not be nil")
	}
	if env.State() != EnvStateReady {
		t.Errorf("initial state should be ready, got %s", env.State())
	}
}

func TestPipelineCrashTypes(t *testing.T) {
	gen := NewPerturbationGenerator(42)
	runner := NewPipelineRunner(gen)

	tests := []struct {
		pertType PerturbationType
		crashType CrashType
	}{
		{PerturbNetworkBreak, CrashNetworkBreak},
		{PerturbProcessKill, CrashProcessKill},
		{PerturbDiskFull, CrashDiskFull},
		{PerturbFileContent, CrashProcessKill},
	}

	for _, tt := range tests {
		crashType := runner.perturbationToCrashType(tt.pertType)
		if crashType != tt.crashType {
			t.Errorf("perturbation %s: expected crash %s, got %s", tt.pertType, tt.crashType, crashType)
		}
	}
}