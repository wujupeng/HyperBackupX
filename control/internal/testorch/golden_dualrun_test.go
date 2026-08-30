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