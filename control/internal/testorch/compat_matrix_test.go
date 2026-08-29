package testorch

import (
	"testing"

	"hbx-control/internal/compat"
)

func TestExecuteCompatMatrix(t *testing.T) {
	catalog := compat.NewDuplicatiFeatureCatalog()
	registry := NewHbxFeatureRegistry()
	executor := NewMatrixExecutor()

	report := executor.ExecuteCompatMatrix(catalog, registry)

	if report.TotalFeatures != catalog.Count() {
		t.Errorf("expected total features %d, got %d", catalog.Count(), report.TotalFeatures)
	}

	if report.TotalFeatures == 0 {
		t.Fatal("expected non-zero total features")
	}

	if report.ImplementedCount == 0 {
		t.Error("expected at least some implemented features")
	}

	expectedCoverage := float64(report.ImplementedCount) / float64(report.TotalFeatures)
	if report.CoverageRate != expectedCoverage {
		t.Errorf("expected coverage %f, got %f", expectedCoverage, report.CoverageRate)
	}

	if report.CoverageRate < 0.0 || report.CoverageRate > 1.0 {
		t.Errorf("coverage rate %f out of range [0,1]", report.CoverageRate)
	}
}

func TestCompatMatrixEntryCount(t *testing.T) {
	catalog := compat.NewDuplicatiFeatureCatalog()
	registry := NewHbxFeatureRegistry()
	executor := NewMatrixExecutor()

	report := executor.ExecuteCompatMatrix(catalog, registry)

	if len(report.Entries) != report.TotalFeatures {
		t.Errorf("expected %d entries, got %d", report.TotalFeatures, len(report.Entries))
	}
}

func TestCompatMatrixStatusConsistency(t *testing.T) {
	catalog := compat.NewDuplicatiFeatureCatalog()
	registry := NewHbxFeatureRegistry()
	executor := NewMatrixExecutor()

	report := executor.ExecuteCompatMatrix(catalog, registry)

	implCount := 0
	partialCount := 0
	notImplCount := 0

	for _, entry := range report.Entries {
		switch entry.HbxStatus {
		case HbxImplemented:
			if entry.TestResult != EntryPass {
				t.Errorf("implemented feature %s should have PASS result, got %s", entry.FeatureName, entry.TestResult)
			}
			implCount++
		case HbxPartial:
			if entry.TestResult != EntryFail {
				t.Errorf("partial feature %s should have FAIL result, got %s", entry.FeatureName, entry.TestResult)
			}
			partialCount++
		case HbxNotImplemented:
			if entry.TestResult != EntryFail {
				t.Errorf("not implemented feature %s should have FAIL result, got %s", entry.FeatureName, entry.TestResult)
			}
			notImplCount++
		}
	}

	if implCount != report.ImplementedCount {
		t.Errorf("implemented count mismatch: entries=%d, report=%d", implCount, report.ImplementedCount)
	}
	if partialCount != report.PartialCount {
		t.Errorf("partial count mismatch: entries=%d, report=%d", partialCount, report.PartialCount)
	}
	if notImplCount != report.NotImplementedCount {
		t.Errorf("not implemented count mismatch: entries=%d, report=%d", notImplCount, report.NotImplementedCount)
	}
}

func TestFeatureRegistryGet(t *testing.T) {
	registry := NewHbxFeatureRegistry()

	if registry.Get("full_backup") != HbxImplemented {
		t.Error("expected full_backup to be implemented")
	}

	if registry.Get("gzip_compression") != HbxNotImplemented {
		t.Error("expected gzip_compression to be not implemented")
	}

	if registry.Get("regex_filter") != HbxPartial {
		t.Error("expected regex_filter to be partial")
	}

	if registry.Get("nonexistent_feature") != HbxNotImplemented {
		t.Error("expected nonexistent feature to default to not implemented")
	}
}

func TestFeatureRegistrySet(t *testing.T) {
	registry := NewHbxFeatureRegistry()

	registry.Set("new_feature", HbxImplemented)
	if registry.Get("new_feature") != HbxImplemented {
		t.Error("expected newly set feature to be implemented")
	}

	registry.Set("full_backup", HbxNotImplemented)
	if registry.Get("full_backup") != HbxNotImplemented {
		t.Error("expected updated feature to be not implemented")
	}
}

func TestCompatMatrixManaged(t *testing.T) {
	catalog := compat.NewDuplicatiFeatureCatalog()
	registry := NewHbxFeatureRegistry()
	executor := NewMatrixExecutor()
	manager := NewManager()

	matrix := manager.CreateMatrix("compat-test", catalog.Count())

	report, err := executor.ExecuteCompatMatrixManaged(manager, matrix.ID, catalog, registry)
	if err != nil {
		t.Fatalf("ExecuteCompatMatrixManaged failed: %v", err)
	}

	if report.TotalFeatures != catalog.Count() {
		t.Errorf("expected total features %d, got %d", catalog.Count(), report.TotalFeatures)
	}

	entries := manager.ListEntries(matrix.ID)
	if len(entries) != catalog.Count() {
		t.Errorf("expected %d managed entries, got %d", catalog.Count(), len(entries))
	}
}