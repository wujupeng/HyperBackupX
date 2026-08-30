package soak


import (
	"testing"
	"time"

	"hbx-control/internal/certorch/common"
)

func TestSoakDuration_Valid(t *testing.T) {
	if !Duration24h.Valid() {
		t.Error("24h should be valid")
	}
	if !Duration72h.Valid() {
		t.Error("72h should be valid")
	}
	if !Duration7d.Valid() {
		t.Error("7d should be valid")
	}
	if SoakDuration("1h").Valid() {
		t.Error("1h should be invalid")
	}
}

func TestSoakDuration_Duration(t *testing.T) {
	if Duration24h.Duration() != 24*time.Hour {
		t.Error("24h duration mismatch")
	}
	if Duration7d.Duration() != 7*24*time.Hour {
		t.Error("7d duration mismatch")
	}
}

func TestStabilityAnalyzer_AllPass(t *testing.T) {
	analyzer := NewStabilityAnalyzer()
	samples := make([]ResourceSample, 10)
	for i := range samples {
		samples[i] = ResourceSample{
			Timestamp:   time.Now().Add(time.Duration(i) * time.Second),
			RSSBytes:    100 * 1024 * 1024,
			OpenHandles: 100,
			HeartbeatOK: true,
		}
	}

	verdict := analyzer.Analyze(samples)
	if !verdict.AllPassed() {
		for _, m := range verdict.Metrics {
			if m.Verdict != common.V3Pass {
				t.Logf("  %s: %s %s", m.Metric, m.Verdict, m.RootCause)
			}
		}
		t.Error("expected all metrics to pass")
	}
}

func TestStabilityAnalyzer_MemoryLeak(t *testing.T) {
	analyzer := NewStabilityAnalyzer()
	samples := make([]ResourceSample, 20)
	for i := range samples {
		samples[i] = ResourceSample{
			Timestamp:   time.Now().Add(time.Duration(i) * time.Second),
			RSSBytes:    uint64(100*1024*1024 + i*10*1024*1024),
			HeartbeatOK: true,
		}
	}

	verdict := analyzer.Analyze(samples)
	found := false
	for _, m := range verdict.Metrics {
		if m.Metric == MetricMemoryLeak {
			found = true
			if m.Verdict != common.V3Fail {
				t.Errorf("expected memory leak to fail, got %s", m.Verdict)
			}
			if m.RootCause == "" {
				t.Error("expected non-empty root cause")
			}
		}
	}
	if !found {
		t.Error("memory leak metric not found")
	}
}

func TestStabilityAnalyzer_InsufficientSamples(t *testing.T) {
	analyzer := NewStabilityAnalyzer()
	samples := []ResourceSample{}

	verdict := analyzer.Analyze(samples)
	for _, m := range verdict.Metrics {
		if m.Verdict != common.V3NotTested && m.Metric != MetricZombieProcess {
			t.Errorf("expected not_tested for %s, got %s", m.Metric, m.Verdict)
		}
	}
}

func TestStabilityAnalyzer_HeartbeatFailure(t *testing.T) {
	analyzer := NewStabilityAnalyzer()
	samples := make([]ResourceSample, 10)
	for i := range samples {
		samples[i] = ResourceSample{
			Timestamp:   time.Now().Add(time.Duration(i) * time.Second),
			HeartbeatOK: i%5 != 0,
		}
	}

	verdict := analyzer.Analyze(samples)
	for _, m := range verdict.Metrics {
		if m.Metric == MetricHeartbeatStability {
			if m.Verdict != common.V3Fail {
				t.Errorf("expected heartbeat to fail, got %s", m.Verdict)
			}
		}
	}
}

func TestStabilityVerdict_AllPassed(t *testing.T) {
	sv := StabilityVerdict{
		Metrics: []MetricVerdict{
			{Metric: MetricMemoryLeak, Verdict: common.V3Pass},
			{Metric: MetricHandleLeak, Verdict: common.V3Pass},
		},
	}
	if !sv.AllPassed() {
		t.Error("expected all passed")
	}

	sv.Metrics[1].Verdict = common.V3Fail
	if sv.AllPassed() {
		t.Error("expected not all passed after fail")
	}
}

func TestStabilityAnalyzer_ExtractThresholds(t *testing.T) {
	analyzer := NewStabilityAnalyzer()
	verdict := StabilityVerdict{}
	thresholds := analyzer.ExtractThresholds(verdict)
	if thresholds.MemoryLeakRateUpper <= 0 {
		t.Error("expected positive memory leak threshold")
	}
	if thresholds.HeartbeatLossRateUpper <= 0 {
		t.Error("expected positive heartbeat loss threshold")
	}
}

func TestLoadGenerator_Start(t *testing.T) {
	gen := NewLoadGenerator()
	err := gen.Start(t.Context(), LoadPattern{
		BackupInterval:      1 * time.Second,
		IncrementalInterval: 1 * time.Second,
		RestoreInterval:     2 * time.Second,
	})
	if err != nil {
		t.Fatalf("Start failed: %v", err)
	}
	if !gen.IsRunning() {
		t.Error("expected running after start")
	}
}

func TestLoadGenerator_GetAnomalies(t *testing.T) {
	gen := NewLoadGenerator()
	anomalies := gen.GetAnomalies()
	if anomalies == nil {
		t.Error("expected non-nil anomalies slice")
	}
}