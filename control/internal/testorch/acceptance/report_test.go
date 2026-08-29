package acceptance

import (
	"testing"
)

func TestGenerateConclusionAllPass(t *testing.T) {
	gen := NewReportGenerator()
	conclusion := gen.GenerateConclusion(100, 100, 50, 50, 1000, 1000, 5, 5, 12, 12, true)

	if !conclusion.AllPassed {
		t.Error("all lines pass should set AllPassed to true")
	}

	for _, line := range conclusion.Lines {
		if line.Status != LineStatusPass {
			t.Errorf("line %s should be pass, got %s", line.Name, line.Status)
		}
	}
}

func TestGenerateConclusionSixLines(t *testing.T) {
	gen := NewReportGenerator()
	conclusion := gen.GenerateConclusion(100, 100, 50, 50, 1000, 1000, 5, 5, 12, 12, true)

	if len(conclusion.Lines) != 6 {
		t.Errorf("expected 6 lines, got %d", len(conclusion.Lines))
	}

	expectedNames := []LineName{
		LineFeatureCompat, LineRestoreCompat, LineDataIntegrity,
		LineCrashRecovery, LineWinCompat, LineRAMStability,
	}
	for i, expected := range expectedNames {
		if conclusion.Lines[i].Name != expected {
			t.Errorf("line %d: expected %s, got %s", i, expected, conclusion.Lines[i].Name)
		}
	}
}

func TestGenerateConclusionPartialFail(t *testing.T) {
	gen := NewReportGenerator()
	conclusion := gen.GenerateConclusion(90, 100, 50, 50, 1000, 1000, 5, 5, 12, 12, true)

	if conclusion.AllPassed {
		t.Error("should not be AllPassed when feature compat fails")
	}
	if conclusion.Lines[0].Status != LineStatusFail {
		t.Error("feature compat should be fail")
	}
}

func TestGenerateConclusionRAMFail(t *testing.T) {
	gen := NewReportGenerator()
	conclusion := gen.GenerateConclusion(100, 100, 50, 50, 1000, 1000, 5, 5, 12, 12, false)

	if conclusion.AllPassed {
		t.Error("should not be AllPassed when RAM stability fails")
	}
	if conclusion.Lines[5].Status != LineStatusFail {
		t.Error("RAM stability should be fail")
	}
}

func TestGenerateConclusionZeroTotal(t *testing.T) {
	gen := NewReportGenerator()
	conclusion := gen.GenerateConclusion(0, 0, 0, 0, 0, 0, 0, 0, 0, 0, false)

	for _, line := range conclusion.Lines {
		if line.Status != LineStatusPending && line.Status != LineStatusFail {
			t.Errorf("line %s should be pending or fail, got %s", line.Name, line.Status)
		}
	}
}

func TestGenerateReport(t *testing.T) {
	gen := NewReportGenerator()
	report := gen.GenerateReport(100, 100, 50, 50, 1000, 1000, 5, 5, 12, 12, true)

	if report.ID == "" {
		t.Error("report ID should not be empty")
	}
	if report.SignRecord != nil {
		t.Error("new report should not have sign record")
	}
}

func TestMarshalUnmarshalReport(t *testing.T) {
	gen := NewReportGenerator()
	report := gen.GenerateReport(100, 100, 50, 50, 1000, 1000, 5, 5, 12, 12, true)

	data, err := gen.MarshalReport(report)
	if err != nil {
		t.Fatalf("marshal failed: %v", err)
	}

	restored, err := UnmarshalReport(data)
	if err != nil {
		t.Fatalf("unmarshal failed: %v", err)
	}

	if restored.ID != report.ID {
		t.Error("ID should match after marshal/unmarshal")
	}
	if len(restored.SixLineConclusion.Lines) != 6 {
		t.Error("should have 6 lines after marshal/unmarshal")
	}
}

func TestSignGateSuccess(t *testing.T) {
	gen := NewReportGenerator()
	report := gen.GenerateReport(100, 100, 50, 50, 1000, 1000, 5, 5, 12, 12, true)

	gate := NewSignGate()
	signed, err := gate.Sign(report, "admin", "all lines pass")
	if err != nil {
		t.Fatalf("sign should succeed: %v", err)
	}
	if signed.SignRecord == nil {
		t.Error("sign record should be set")
	}
	if signed.SignRecord.SignedBy != "admin" {
		t.Errorf("expected signed_by=admin, got %s", signed.SignRecord.SignedBy)
	}
}

func TestSignGateFailure(t *testing.T) {
	gen := NewReportGenerator()
	report := gen.GenerateReport(90, 100, 50, 50, 1000, 1000, 5, 5, 12, 12, true)

	gate := NewSignGate()
	_, err := gate.Sign(report, "admin", "should fail")
	if err == nil {
		t.Fatal("sign should fail when lines not all passed")
	}

	signErr, ok := err.(*SignError)
	if !ok {
		t.Fatal("error should be SignError")
	}
	if len(signErr.FailedLines) == 0 {
		t.Error("should have failed lines")
	}
}

func TestCanSign(t *testing.T) {
	gen := NewReportGenerator()
	gate := NewSignGate()

	allPass := gen.GenerateConclusion(100, 100, 50, 50, 1000, 1000, 5, 5, 12, 12, true)
	canSign, failed := gate.CanSign(allPass)
	if !canSign {
		t.Errorf("should be able to sign all-pass conclusion, failed: %v", failed)
	}

	partialFail := gen.GenerateConclusion(90, 100, 50, 50, 1000, 1000, 5, 5, 12, 12, true)
	canSign, failed = gate.CanSign(partialFail)
	if canSign {
		t.Error("should not be able to sign partial-fail conclusion")
	}
	if len(failed) == 0 {
		t.Error("should report failed lines")
	}
}

func TestCheckAllLines(t *testing.T) {
	gen := NewReportGenerator()
	gate := NewSignGate()

	conclusion := gen.GenerateConclusion(100, 100, 50, 50, 1000, 1000, 5, 5, 12, 12, true)
	result := gate.CheckAllLines(conclusion)

	if len(result) != 6 {
		t.Errorf("expected 6 results, got %d", len(result))
	}
	for _, passed := range result {
		if !passed {
			t.Error("all lines should be passed")
		}
	}
}

func TestAllLines(t *testing.T) {
	lines := AllLines()
	if len(lines) != 6 {
		t.Errorf("expected 6 lines, got %d", len(lines))
	}
}

func TestLineEvidence(t *testing.T) {
	gen := NewReportGenerator()
	conclusion := gen.GenerateConclusion(100, 100, 50, 50, 1000, 1000, 5, 5, 12, 12, true)

	for _, line := range conclusion.Lines {
		if len(line.Evidence) == 0 {
			t.Errorf("line %s should have evidence", line.Name)
		}
		for _, ev := range line.Evidence {
			if ev.ReportType == "" {
				t.Errorf("line %s evidence should have report_type", line.Name)
			}
			if ev.Summary == "" {
				t.Errorf("line %s evidence should have summary", line.Name)
			}
		}
	}
}

func TestSignGateNilReport(t *testing.T) {
	gate := NewSignGate()
	_, err := gate.Sign(nil, "admin", "")
	if err == nil {
		t.Error("should error on nil report")
	}
}