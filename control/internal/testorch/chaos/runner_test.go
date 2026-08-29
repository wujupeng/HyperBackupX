package chaos

import (
	"testing"
)

func TestExecuteScenarioAllFaultTypes(t *testing.T) {
	injector := NewFaultInjector(42)
	runner := NewPipelineRunner(injector)

	for _, ft := range AllFaultTypes() {
		result, err := runner.ExecuteScenario(ft, "test-target")
		if err != nil {
			t.Errorf("scenario %s failed: %v", ft, err)
		}
		if !result.Passed {
			t.Errorf("scenario %s should pass: detection=%v rejected=%v markedFailed=%v",
				ft, result.DamageReport.Detected, result.RecoveryResult.Rejected, result.RecoveryResult.MarkedFailed)
		}
	}
}

func TestDamageDetection(t *testing.T) {
	injector := NewFaultInjector(42)
	runner := NewPipelineRunner(injector)

	for _, ft := range AllFaultTypes() {
		result, err := runner.ExecuteScenario(ft, "test-target")
		if err != nil {
			t.Fatalf("scenario %s failed: %v", ft, err)
		}
		if !result.DamageReport.Detected {
			t.Errorf("fault %s: damage should be detected", ft)
		}
		if result.DamageReport.Type == "" {
			t.Errorf("fault %s: damage type should not be empty", ft)
		}
		if result.DamageReport.Description == "" {
			t.Errorf("fault %s: damage description should not be empty", ft)
		}
	}
}

func TestRecoveryRejected(t *testing.T) {
	injector := NewFaultInjector(42)
	runner := NewPipelineRunner(injector)

	for _, ft := range AllFaultTypes() {
		result, err := runner.ExecuteScenario(ft, "test-target")
		if err != nil {
			t.Fatalf("scenario %s failed: %v", ft, err)
		}
		if !result.RecoveryResult.Rejected {
			t.Errorf("fault %s: recovery should be rejected", ft)
		}
		if !result.RecoveryResult.MarkedFailed {
			t.Errorf("fault %s: recovery should be marked failed", ft)
		}
	}
}

func TestScenarioJudgment(t *testing.T) {
	injector := NewFaultInjector(42)
	runner := NewPipelineRunner(injector)

	result, err := runner.ExecuteScenario(FaultModifyChunk, "test-target")
	if err != nil {
		t.Fatalf("scenario failed: %v", err)
	}

	if !result.Passed {
		t.Error("scenario with detected damage and rejected recovery should pass")
	}
}

func TestRunAllScenarios(t *testing.T) {
	injector := NewFaultInjector(42)
	runner := NewPipelineRunner(injector)

	report, err := runner.RunAllScenarios("test-target")
	if err != nil {
		t.Fatalf("RunAllScenarios failed: %v", err)
	}

	if report.TotalScenarios != 5 {
		t.Errorf("expected 5 scenarios, got %d", report.TotalScenarios)
	}
	if report.FailedCount != 0 {
		t.Errorf("expected 0 failures, got %d", report.FailedCount)
	}
	if report.PassedCount != 5 {
		t.Errorf("expected 5 passes, got %d", report.PassedCount)
	}
	if report.LeakDetected {
		t.Error("no leak should be detected when all scenarios pass")
	}
}

func TestGenerateReport(t *testing.T) {
	injector := NewFaultInjector(42)
	runner := NewPipelineRunner(injector)

	data, err := runner.GenerateReport("test-target")
	if err != nil {
		t.Fatalf("GenerateReport failed: %v", err)
	}
	if len(data) == 0 {
		t.Error("report data should not be empty")
	}
}

func TestResults(t *testing.T) {
	injector := NewFaultInjector(42)
	runner := NewPipelineRunner(injector)

	runner.ExecuteScenario(FaultKillAgent, "target1")
	runner.ExecuteScenario(FaultModifyChunk, "target2")

	results := runner.Results()
	if len(results) != 2 {
		t.Errorf("expected 2 results, got %d", len(results))
	}
}

func TestCheckLeak(t *testing.T) {
	injector := NewFaultInjector(42)
	runner := NewPipelineRunner(injector)

	goodResult := &ChaosScenarioResult{
		DamageReport:   DamageReport{Detected: true},
		RecoveryResult: RecoveryResult{Rejected: true},
	}
	if runner.CheckLeak(goodResult) {
		t.Error("should not detect leak when damage detected and recovery rejected")
	}

	leakResult1 := &ChaosScenarioResult{
		DamageReport:   DamageReport{Detected: false},
		RecoveryResult: RecoveryResult{Rejected: true},
	}
	if !runner.CheckLeak(leakResult1) {
		t.Error("should detect leak when damage not detected")
	}

	leakResult2 := &ChaosScenarioResult{
		DamageReport:   DamageReport{Detected: true},
		RecoveryResult: RecoveryResult{Rejected: false},
	}
	if !runner.CheckLeak(leakResult2) {
		t.Error("should detect leak when recovery not rejected")
	}
}

func TestDamageReportLocation(t *testing.T) {
	injector := NewFaultInjector(42)
	runner := NewPipelineRunner(injector)

	result, err := runner.ExecuteScenario(FaultModifyChunk, "/data/repo")
	if err != nil {
		t.Fatalf("scenario failed: %v", err)
	}

	if result.DamageReport.Location.Path != "/data/repo" {
		t.Errorf("expected location /data/repo, got %s", result.DamageReport.Location.Path)
	}
	if result.DamageReport.Location.ChunkID == "" {
		t.Error("chunk modification should have chunk_id in location")
	}
}

func TestBaselineCreated(t *testing.T) {
	injector := NewFaultInjector(42)
	runner := NewPipelineRunner(injector)

	result, err := runner.ExecuteScenario(FaultKillAgent, "test-target")
	if err != nil {
		t.Fatalf("scenario failed: %v", err)
	}
	if !result.BaselineCreated {
		t.Error("baseline should be created")
	}
}

func TestFaultInjected(t *testing.T) {
	injector := NewFaultInjector(42)
	runner := NewPipelineRunner(injector)

	result, err := runner.ExecuteScenario(FaultDeleteVolume, "test-target")
	if err != nil {
		t.Fatalf("scenario failed: %v", err)
	}
	if !result.FaultInjected {
		t.Error("fault should be injected")
	}
}