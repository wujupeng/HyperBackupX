package common


import (
	"testing"
)

func TestCertVerdict_IsPass(t *testing.T) {
	v := CertVerdict{Status: V3Pass}
	if !v.IsPass() {
		t.Error("expected IsPass=true")
	}
	if v.IsFail() {
		t.Error("expected IsFail=false")
	}
	if v.IsNotTested() {
		t.Error("expected IsNotTested=false")
	}
}

func TestCertVerdict_Validate_FailRequiresRootCause(t *testing.T) {
	v := CertVerdict{Status: V3Fail}
	if err := v.Validate(); err == nil {
		t.Error("expected error for fail without root_cause")
	}
}

func TestCertVerdict_Validate_FailWithRootCause(t *testing.T) {
	v := CertVerdict{Status: V3Fail, RootCause: "memory leak in agent"}
	if err := v.Validate(); err != nil {
		t.Errorf("unexpected error: %v", err)
	}
}

func TestCertVerdict_Validate_NotTestedRequiresReason(t *testing.T) {
	v := CertVerdict{Status: V3NotTested}
	if err := v.Validate(); err == nil {
		t.Error("expected error for not_tested without reason")
	}
}

func TestCertVerdict_Validate_NotTestedWithReason(t *testing.T) {
	v := CertVerdict{Status: V3NotTested, NotTestedReason: "missing 1TB storage"}
	if err := v.Validate(); err != nil {
		t.Errorf("unexpected error: %v", err)
	}
}

func TestCertVerdict_Validate_PassNoExtras(t *testing.T) {
	v := CertVerdict{Status: V3Pass}
	if err := v.Validate(); err != nil {
		t.Errorf("unexpected error: %v", err)
	}
}

func TestCertVerdict4_Validate_FailRequiresRootCause(t *testing.T) {
	v := CertVerdict4{Status: V4Fail}
	if err := v.Validate(); err == nil {
		t.Error("expected error for fail without root_cause")
	}
}

func TestCertVerdict4_Validate_DifferentByDesignRequiresRationale(t *testing.T) {
	v := CertVerdict4{Status: V4DifferentByDesign}
	if err := v.Validate(); err == nil {
		t.Error("expected error for different_by_design without design_rationale")
	}
}

func TestCertVerdict4_Validate_DifferentByDesignWithRationale(t *testing.T) {
	v := CertVerdict4{Status: V4DifferentByDesign, DesignRationale: "HBX Adaptive chunking vs Duplicati fixed"}
	if err := v.Validate(); err != nil {
		t.Errorf("unexpected error: %v", err)
	}
}

func TestCertVerdict4_Validate_NotSupportedRequiresReason(t *testing.T) {
	v := CertVerdict4{Status: V4NotSupported}
	if err := v.Validate(); err == nil {
		t.Error("expected error for not_supported without reason")
	}
}

func TestCertVerdict4_Validate_NotSupportedWithReason(t *testing.T) {
	v := CertVerdict4{Status: V4NotSupported, NotSupportedReason: "HBX does not support FTP backend"}
	if err := v.Validate(); err != nil {
		t.Errorf("unexpected error: %v", err)
	}
}

func TestRedactContent(t *testing.T) {
	input := []byte(`{"password":"secret123","dsn":"postgres://user:pass@localhost/db","token":"Bearer abc123"}`)
	redacted := redactJSONKeys(input)
	s := string(redacted)
	if contains(s, "secret123") {
		t.Error("password value not redacted")
	}
	if contains(s, "pass@localhost") {
		t.Error("DSN password not redacted")
	}
}

func TestDetectLeak(t *testing.T) {
	clean := []byte(`{"status":"pass","metric":"throughput"}`)
	if detectLeak(clean) {
		t.Error("false positive leak detection")
	}
	leaky := []byte(`postgres://user:password@localhost/db`)
	if !detectLeak(leaky) {
		t.Error("leak not detected in DSN")
	}
	leaky2 := []byte(`Bearer eyJhbGciOiJIUzI1NiJ9.payload`)
	if !detectLeak(leaky2) {
		t.Error("leak not detected in Bearer token")
	}
}

func TestParseGoTestOutput(t *testing.T) {
	output := `ok  hbx-control/internal/compat  0.123s
ok  hbx-control/internal/testorch  0.456s
FAIL hbx-control/internal/api  0.789s
--- FAIL: TestBadEndpoint (0.001s)
`
	result := parseGoTestOutput(output)
	if result.Passed != 2 {
		t.Errorf("expected 2 passed, got %d", result.Passed)
	}
	if result.Failed != 1 {
		t.Errorf("expected 1 failed, got %d", result.Failed)
	}
	if result.Total != 3 {
		t.Errorf("expected 3 total, got %d", result.Total)
	}
}

func TestParseCargoTestOutput(t *testing.T) {
	output := `running 10 tests
test test_a ... ok
test test_b ... ok
test result: ok. 10 passed; 0 failed
running 5 tests
test test_c ... FAILED
test result: FAILED. 4 passed; 1 failed
`
	result := parseCargoTestOutput(output)
	if result.Passed != 1 {
		t.Errorf("expected 1 passed suite, got %d", result.Passed)
	}
	if result.Failed != 1 {
		t.Errorf("expected 1 failed suite, got %d", result.Failed)
	}
}

func TestCPMetrics_Collect(t *testing.T) {
	c := NewCPMetricsCollector()
	m := c.Collect(t.Context())
	if m.NumGoroutine <= 0 {
		t.Error("expected non-zero goroutine count")
	}
	if m.HeapAllocBytes == 0 {
		t.Error("expected non-zero heap alloc")
	}
}

func contains(s, substr string) bool {
	return len(s) >= len(substr) && (indexOf(s, substr) >= 0)
}

func indexOf(s, substr string) int {
	for i := 0; i <= len(s)-len(substr); i++ {
		if s[i:i+len(substr)] == substr {
			return i
		}
	}
	return -1
}