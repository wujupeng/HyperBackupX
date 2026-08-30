package testorch

import (
	"testing"
)

func TestRunGoldenDualComparison(t *testing.T) {
	builder := NewGoldenDatasetBuilder("/tmp/golden")
	dataset := builder.Build()
	comparator := NewDualRunComparator()

	report := comparator.RunGoldenDualComparison(dataset)

	if report.DatasetName != dataset.Name {
		t.Errorf("expected dataset name %s, got %s", dataset.Name, report.DatasetName)
	}

	if report.TotalFiles != dataset.Count() {
		t.Errorf("expected total files %d, got %d", dataset.Count(), report.TotalFiles)
	}

	if len(report.FileResults) != dataset.Count() {
		t.Errorf("expected %d file results, got %d", dataset.Count(), len(report.FileResults))
	}
}

func TestGoldenDualComparisonConsistency(t *testing.T) {
	builder := NewGoldenDatasetBuilder("/tmp/golden")
	dataset := builder.Build()
	comparator := NewDualRunComparator()

	report := comparator.RunGoldenDualComparison(dataset)

	if report.PassedFiles+report.FailedFiles != report.TotalFiles {
		t.Errorf("passed+failed != total: %d+%d != %d", report.PassedFiles, report.FailedFiles, report.TotalFiles)
	}

	expectedRate := float64(report.PassedFiles) / float64(report.TotalFiles)
	if report.ConsistencyRate != expectedRate {
		t.Errorf("expected consistency rate %f, got %f", expectedRate, report.ConsistencyRate)
	}
}

func TestGoldenDualComparisonAllPass(t *testing.T) {
	builder := NewGoldenDatasetBuilder("/tmp/golden")
	dataset := builder.Build()
	comparator := NewDualRunComparator()

	report := comparator.RunGoldenDualComparison(dataset)

	if report.FailedFiles != 0 {
		t.Errorf("expected 0 failed files for valid golden dataset, got %d", report.FailedFiles)
		for _, r := range report.FileResults {
			if !r.Pass {
				t.Logf("  FAIL: %s - %s", r.RelativePath, r.FailReason)
			}
		}
	}

	if report.ConsistencyRate != 1.0 {
		t.Errorf("expected 1.0 consistency rate, got %f", report.ConsistencyRate)
	}
}

func TestGoldenDualComparisonSummary(t *testing.T) {
	builder := NewGoldenDatasetBuilder("/tmp/golden")
	dataset := builder.Build()
	comparator := NewDualRunComparator()

	report := comparator.RunGoldenDualComparison(dataset)

	if report.Summary == "" {
		t.Error("expected non-empty summary")
	}
}

func TestGoldenDualComparisonManaged(t *testing.T) {
	builder := NewGoldenDatasetBuilder("/tmp/golden")
	dataset := builder.Build()
	comparator := NewDualRunComparator()
	manager := NewManager()

	report, err := comparator.RunGoldenDualComparisonManaged(manager, dataset)
	if err != nil {
		t.Fatalf("RunGoldenDualComparisonManaged failed: %v", err)
	}

	if report.TotalFiles != dataset.Count() {
		t.Errorf("expected total files %d, got %d", dataset.Count(), report.TotalFiles)
	}

	runs := manager.ListDualRuns()
	if len(runs) != 1 {
		t.Errorf("expected 1 dual run, got %d", len(runs))
	}
}

func TestFullChainDualComparison_AllPass(t *testing.T) {
	comparator := NewDualRunComparator()
	stages := []ChainStage{StageBackup, StageRestore, StageVersion, StageDelete, StageVerify, StageRecovery}
	dupResults := map[ChainStage]bool{
		StageBackup: true, StageRestore: true, StageVersion: true,
		StageDelete: true, StageVerify: true, StageRecovery: true,
	}
	hbxResults := map[ChainStage]bool{
		StageBackup: true, StageRestore: true, StageVersion: true,
		StageDelete: true, StageVerify: true, StageRecovery: true,
	}

	result := comparator.RunFullChainDualComparison(stages, dupResults, hbxResults, nil, nil)

	if !result.AllPassed {
		t.Error("expected all stages to pass")
	}
	if len(result.Stages) != 6 {
		t.Errorf("expected 6 stages, got %d", len(result.Stages))
	}
	for _, s := range result.Stages {
		if s.Verdict != SVPass {
			t.Errorf("expected pass for stage %s, got %s", s.Stage, s.Verdict)
		}
	}
}

func TestFullChainDualComparison_FailAtRestore(t *testing.T) {
	comparator := NewDualRunComparator()
	stages := []ChainStage{StageBackup, StageRestore}
	dupResults := map[ChainStage]bool{StageBackup: true, StageRestore: true}
	hbxResults := map[ChainStage]bool{StageBackup: true, StageRestore: false}

	result := comparator.RunFullChainDualComparison(stages, dupResults, hbxResults, nil, nil)

	if result.AllPassed {
		t.Error("expected not all passed")
	}
	if result.Stages[1].Verdict != SVFail {
		t.Errorf("expected fail at restore, got %s", result.Stages[1].Verdict)
	}
	if result.Stages[1].RootCause == "" {
		t.Error("expected non-empty root cause for fail")
	}
}

func TestFullChainDualComparison_DifferentByDesign(t *testing.T) {
	comparator := NewDualRunComparator()
	stages := []ChainStage{StageBackup}
	dupResults := map[ChainStage]bool{StageBackup: true}
	hbxResults := map[ChainStage]bool{StageBackup: true}
	diffByDesign := map[ChainStage]string{
		StageBackup: "HBX Adaptive chunking vs Duplicati fixed chunking",
	}

	result := comparator.RunFullChainDualComparison(stages, dupResults, hbxResults, diffByDesign, nil)

	if result.Stages[0].Verdict != SVDifferentByDesign {
		t.Errorf("expected different_by_design, got %s", result.Stages[0].Verdict)
	}
	if result.Stages[0].DesignRationale == "" {
		t.Error("expected non-empty design rationale")
	}
}

func TestFullChainDualComparison_NotSupported(t *testing.T) {
	comparator := NewDualRunComparator()
	stages := []ChainStage{StageRecovery}
	dupResults := map[ChainStage]bool{StageRecovery: true}
	hbxResults := map[ChainStage]bool{StageRecovery: false}
	notSupported := map[ChainStage]string{
		StageRecovery: "HBX does not support bare-metal recovery",
	}

	result := comparator.RunFullChainDualComparison(stages, dupResults, hbxResults, nil, notSupported)

	if result.Stages[0].Verdict != SVNotSupported {
		t.Errorf("expected not_supported, got %s", result.Stages[0].Verdict)
	}
	if result.Stages[0].NotSupportedReason == "" {
		t.Error("expected non-empty not supported reason")
	}
}

func TestFullChainDualComparison_Summary(t *testing.T) {
	comparator := NewDualRunComparator()
	stages := []ChainStage{StageBackup, StageRestore, StageVerify}
	dupResults := map[ChainStage]bool{StageBackup: true, StageRestore: true, StageVerify: true}
	hbxResults := map[ChainStage]bool{StageBackup: true, StageRestore: true, StageVerify: true}

	result := comparator.RunFullChainDualComparison(stages, dupResults, hbxResults, nil, nil)

	if result.Summary == "" {
		t.Error("expected non-empty summary")
	}
}
