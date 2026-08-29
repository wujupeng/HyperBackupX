package testorch

import (
	"testing"

	"github.com/google/uuid"
)

func TestCreateMatrix(t *testing.T) {
	m := NewManager()
	matrix := m.CreateMatrix("test-matrix", 125)
	if matrix.Name != "test-matrix" {
		t.Errorf("expected name 'test-matrix', got %s", matrix.Name)
	}
	if matrix.Status != MatrixIdle {
		t.Errorf("expected idle, got %s", matrix.Status)
	}
	if matrix.TotalEntries != 125 {
		t.Errorf("expected 125 entries, got %d", matrix.TotalEntries)
	}
}

func TestSetMatrixStatus(t *testing.T) {
	m := NewManager()
	matrix := m.CreateMatrix("test", 10)
	if !m.SetMatrixStatus(matrix.ID, MatrixRunning) {
		t.Fatal("set status failed")
	}
	got, _ := m.GetMatrix(matrix.ID)
	if got.Status != MatrixRunning {
		t.Errorf("expected running, got %s", got.Status)
	}
}

func TestAddEntry(t *testing.T) {
	m := NewManager()
	matrix := m.CreateMatrix("test", 10)
	entry, err := m.AddEntry(matrix.ID, "L1", "Local", "backup", "functionality")
	if err != nil {
		t.Fatalf("add entry failed: %v", err)
	}
	if entry.Status != EntryPending {
		t.Errorf("expected pending, got %s", entry.Status)
	}
}

func TestAddEntryMatrixNotFound(t *testing.T) {
	m := NewManager()
	_, err := m.AddEntry(uuid.New(), "L1", "Local", "backup", "functionality")
	if err != ErrMatrixNotFound {
		t.Errorf("expected ErrMatrixNotFound, got %v", err)
	}
}

func TestListEntriesByLayer(t *testing.T) {
	m := NewManager()
	matrix := m.CreateMatrix("test", 10)
	m.AddEntry(matrix.ID, "L1", "Local", "backup", "functionality")
	m.AddEntry(matrix.ID, "L1", "S3", "backup", "functionality")
	m.AddEntry(matrix.ID, "L2", "Local", "connect", "backend")
	l1Entries := m.ListEntriesByLayer(matrix.ID, "L1")
	if len(l1Entries) != 2 {
		t.Errorf("expected 2 L1 entries, got %d", len(l1Entries))
	}
}

func TestUpdateEntryStatus(t *testing.T) {
	m := NewManager()
	matrix := m.CreateMatrix("test", 10)
	entry, _ := m.AddEntry(matrix.ID, "L1", "Local", "backup", "functionality")
	execTime := int64(1500)
	m.UpdateEntryStatus(entry.ID, EntryPass, &execTime, map[string]interface{}{"test": "result"})
	got, _ := m.GetEntry(entry.ID)
	if got.Status != EntryPass {
		t.Errorf("expected pass, got %s", got.Status)
	}
	if got.ExecutionTimeMs == nil || *got.ExecutionTimeMs != 1500 {
		t.Error("execution time not set")
	}
	updatedMatrix, _ := m.GetMatrix(matrix.ID)
	if updatedMatrix.PassedCount != 1 {
		t.Errorf("expected passed count 1, got %d", updatedMatrix.PassedCount)
	}
}

func TestCreateTestCase(t *testing.T) {
	m := NewManager()
	tc := m.CreateTestCase("test-case", "L1", JudgmentSHA256)
	if tc.Status != CasePending {
		t.Errorf("expected pending, got %s", tc.Status)
	}
	if tc.JudgmentCriteria != JudgmentSHA256 {
		t.Errorf("expected sha256, got %s", tc.JudgmentCriteria)
	}
}

func TestUpdateTestCaseResult(t *testing.T) {
	m := NewManager()
	tc := m.CreateTestCase("test", "L1", JudgmentSHA256)
	m.UpdateTestCaseResult(tc.ID, CasePass, map[string]interface{}{"files": 100})
	got, _ := m.GetTestCase(tc.ID)
	if got.Status != CasePass {
		t.Errorf("expected pass, got %s", got.Status)
	}
}

func TestCreateDualRun(t *testing.T) {
	m := NewManager()
	run := m.CreateDualRun(map[string]interface{}{"files": 10000})
	if run.Status != DualRunPending {
		t.Errorf("expected pending, got %s", run.Status)
	}
}

func TestCompleteDualRun(t *testing.T) {
	m := NewManager()
	run := m.CreateDualRun(nil)
	m.CompleteDualRun(run.ID,
		map[string]interface{}{"versions": 5},
		map[string]interface{}{"versions": 5},
		map[string]interface{}{"sha256_match": true},
		1.0, 0,
	)
	got, _ := m.GetDualRun(run.ID)
	if got.Status != DualRunCompleted {
		t.Errorf("expected completed, got %s", got.Status)
	}
	if got.ConsistencyRate != 1.0 {
		t.Errorf("expected 1.0, got %f", got.ConsistencyRate)
	}
}

func TestCreateFuzzScenario(t *testing.T) {
	m := NewManager()
	scenario := m.CreateFuzzScenario("fuzz-test", "random_bytes", 1000)
	if scenario.Iterations != 1000 {
		t.Errorf("expected 1000 iterations, got %d", scenario.Iterations)
	}
}

func TestCreateChaosScenario(t *testing.T) {
	m := NewManager()
	scenario := m.CreateChaosScenario("chaos-test", FaultNetworkPartition, "agent", 60)
	if scenario.FaultType != FaultNetworkPartition {
		t.Errorf("expected network_partition, got %s", scenario.FaultType)
	}
	if scenario.DurationSec != 60 {
		t.Errorf("expected 60 sec, got %d", scenario.DurationSec)
	}
}

func TestCreateReport(t *testing.T) {
	m := NewManager()
	report := m.CreateReport(ReportMatrix, nil, map[string]interface{}{"pass_rate": 0.95}, nil)
	if report.ReportType != ReportMatrix {
		t.Errorf("expected matrix, got %s", report.ReportType)
	}
}

func TestGetMatrixPassRate(t *testing.T) {
	m := NewManager()
	matrix := m.CreateMatrix("test", 10)
	entry1, _ := m.AddEntry(matrix.ID, "L1", "Local", "backup", "functionality")
	entry2, _ := m.AddEntry(matrix.ID, "L1", "S3", "backup", "functionality")
	m.UpdateEntryStatus(entry1.ID, EntryPass, nil, nil)
	m.UpdateEntryStatus(entry2.ID, EntryPass, nil, nil)
	rate := m.GetMatrixPassRate(matrix.ID)
	if rate != 0.2 {
		t.Errorf("expected 0.2, got %f", rate)
	}
}

func TestListMatrices(t *testing.T) {
	m := NewManager()
	m.CreateMatrix("m1", 10)
	m.CreateMatrix("m2", 20)
	matrices := m.ListMatrices()
	if len(matrices) != 2 {
		t.Errorf("expected 2 matrices, got %d", len(matrices))
	}
}

func TestListDualRuns(t *testing.T) {
	m := NewManager()
	m.CreateDualRun(nil)
	m.CreateDualRun(nil)
	runs := m.ListDualRuns()
	if len(runs) != 2 {
		t.Errorf("expected 2 runs, got %d", len(runs))
	}
}

func TestListReports(t *testing.T) {
	m := NewManager()
	m.CreateReport(ReportMatrix, nil, nil, nil)
	m.CreateReport(ReportGolden, nil, nil, nil)
	reports := m.ListReports()
	if len(reports) != 2 {
		t.Errorf("expected 2 reports, got %d", len(reports))
	}
}