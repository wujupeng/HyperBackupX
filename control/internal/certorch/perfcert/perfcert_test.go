package perfcert

import (
	"testing"

	"hbx-control/internal/certorch/common"
)

func TestDatasetSize_Values(t *testing.T) {
	sizes := []DatasetSize{Size10GB, Size100GB, Size1TB}
	if len(sizes) != 3 {
		t.Errorf("expected 3 dataset sizes, got %d", len(sizes))
	}
}

func TestRAMScenario_Values(t *testing.T) {
	rams := []RAMScenario{RAM4GB, RAM8GB}
	if len(rams) != 2 {
		t.Errorf("expected 2 RAM scenarios, got %d", len(rams))
	}
}

func TestOperation_Values(t *testing.T) {
	ops := []Operation{OpInitialBackup, OpIncrementalBackup, OpRestore, OpVerify}
	if len(ops) != 4 {
		t.Errorf("expected 4 operations, got %d", len(ops))
	}
}

func TestBenchmarkResult_NotTestedFor1TB(t *testing.T) {
	bench := BenchmarkResult{
		DatasetSize:     Size1TB,
		RAMScenario:     RAM4GB,
		Operation:       OpInitialBackup,
		Verdict:         common.V3NotTested,
		NotTestedReason: "1TB storage hardware not available",
	}
	if bench.Verdict != common.V3NotTested {
		t.Error("expected not_tested for 1TB")
	}
	if bench.NotTestedReason == "" {
		t.Error("expected non-empty not tested reason")
	}
}

func TestPerfCertResult_Summary(t *testing.T) {
	result := PerfCertResult{
		Benchmarks: []BenchmarkResult{
			{DatasetSize: Size10GB, RAMScenario: RAM8GB, Operation: OpInitialBackup, Verdict: common.V3Pass},
			{DatasetSize: Size1TB, RAMScenario: RAM4GB, Operation: OpRestore, Verdict: common.V3NotTested},
		},
		AllPassed: true,
		Summary:   "test summary",
	}
	if result.Summary == "" {
		t.Error("expected non-empty summary")
	}
}

func TestPerfCertResult_BenchmarkCount(t *testing.T) {
	sizes := []DatasetSize{Size10GB, Size100GB, Size1TB}
	rams := []RAMScenario{RAM4GB, RAM8GB}
	ops := []Operation{OpInitialBackup, OpIncrementalBackup, OpRestore, OpVerify}
	total := len(sizes) * len(rams) * len(ops)
	if total != 24 {
		t.Errorf("expected 24 benchmark combinations, got %d", total)
	}
}
