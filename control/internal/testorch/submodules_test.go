package testorch

import (
	"testing"
)

func TestDuplicatiReferenceManager(t *testing.T) {
	mgr := NewDuplicatiReferenceManager()
	inst, err := mgr.StartInstance("run-001")
	if err != nil {
		t.Fatalf("start failed: %v", err)
	}
	if inst.Status != "running" {
		t.Errorf("expected running, got %s", inst.Status)
	}
	if !mgr.HealthCheck(inst.ID) {
		t.Error("health check should pass for running instance")
	}
	mgr.StopInstance(inst.ID)
	if mgr.HealthCheck(inst.ID) {
		t.Error("health check should fail for stopped instance")
	}
}

func TestAllocateNamespace(t *testing.T) {
	mgr := NewDuplicatiReferenceManager()
	dupNS := mgr.AllocateNamespace("run-001", true)
	hbxNS := mgr.AllocateNamespace("run-001", false)
	if dupNS == hbxNS {
		t.Error("namespaces should be different")
	}
	if dupNS != "duplicati-ref/run-001/" {
		t.Errorf("unexpected dup namespace: %s", dupNS)
	}
	if hbxNS != "hbx-compat/run-001/" {
		t.Errorf("unexpected hbx namespace: %s", hbxNS)
	}
}

func TestSampleBehavior(t *testing.T) {
	mgr := NewDuplicatiReferenceManager()
	inst, _ := mgr.StartInstance("run-001")
	sample, err := mgr.SampleBehavior(inst.ID, "backup")
	if err != nil {
		t.Fatalf("sample failed: %v", err)
	}
	if sample.VersionStructure["operation"] != "backup" {
		t.Error("behavior sample incorrect")
	}
}

func TestMatrixExecutorLoadDefinition(t *testing.T) {
	exec := NewMatrixExecutor()
	def := exec.LoadDefinition()
	if len(def.Entries) == 0 {
		t.Error("expected non-zero entries")
	}
	l1Count := 0
	for _, e := range def.Entries {
		if e.Layer == "L1" {
			l1Count++
		}
	}
	if l1Count != 14 {
		t.Errorf("expected 14 L1 entries, got %d", l1Count)
	}
}

func TestMatrixExecutorExecute(t *testing.T) {
	manager := NewManager()
	matrix := manager.CreateMatrix("test", 0)
	exec := NewMatrixExecutor()
	results, err := exec.ExecuteMatrix(manager, matrix.ID, "")
	if err != nil {
		t.Fatalf("execute failed: %v", err)
	}
	if len(results) == 0 {
		t.Error("expected non-zero results")
	}
	passCount := 0
	for _, r := range results {
		if r.Status == EntryPass {
			passCount++
		}
	}
	if passCount == 0 {
		t.Error("expected at least some passes")
	}
}

func TestMatrixExecutorExecuteWithFilter(t *testing.T) {
	manager := NewManager()
	matrix := manager.CreateMatrix("test", 0)
	exec := NewMatrixExecutor()
	results, _ := exec.ExecuteMatrix(manager, matrix.ID, "L1")
	for _, r := range results {
		if r.Layer != "L1" {
			t.Errorf("expected only L1 entries, got %s", r.Layer)
		}
	}
}

func TestGoldenExecutorLoadScenarios(t *testing.T) {
	exec := NewGoldenExecutor(4)
	count := exec.LoadScenarios()
	if count != 1000 {
		t.Errorf("expected 1000 scenarios, got %d", count)
	}
}

func TestGoldenExecutorExecuteAll(t *testing.T) {
	exec := NewGoldenExecutor(4)
	exec.LoadScenarios()
	manager := NewManager()
	passed, failed, skipped := exec.ExecuteAll(manager)
	if passed+failed+skipped != 1000 {
		t.Errorf("expected 1000 total, got %d", passed+failed+skipped)
	}
	if failed != 0 {
		t.Errorf("expected 0 failures, got %d", failed)
	}
}

func TestDualRunComparatorGenerateInput(t *testing.T) {
	cmp := NewDualRunComparator()
	input := cmp.GenerateInput(10000, 100)
	if input.FileCount != 10000 {
		t.Errorf("expected 10000 files, got %d", input.FileCount)
	}
	if len(input.FileTypes) != 6 {
		t.Errorf("expected 6 file types, got %d", len(input.FileTypes))
	}
}

func TestDualRunComparatorCompare(t *testing.T) {
	cmp := NewDualRunComparator()
	comparison := cmp.Compare(
		map[string]interface{}{"file_count": 100},
		map[string]interface{}{"file_count": 100},
	)
	if !comparison.SHA256Match {
		t.Error("expected SHA256 match")
	}
	if len(comparison.Deviations) != 0 {
		t.Error("expected no deviations")
	}
}

func TestDualRunComparatorRunDual(t *testing.T) {
	manager := NewManager()
	dupMgr := NewDuplicatiReferenceManager()
	cmp := NewDualRunComparator()
	input := cmp.GenerateInput(100, 1)
	result, err := cmp.RunDualComparison(manager, input, dupMgr)
	if err != nil {
		t.Fatalf("dual run failed: %v", err)
	}
	if result.Status != DualRunCompleted {
		t.Errorf("expected completed, got %s", result.Status)
	}
	if result.ConsistencyRate != 1.0 {
		t.Errorf("expected 1.0 consistency rate, got %f", result.ConsistencyRate)
	}
}