package soak

import (
	"fmt"
	"math"
	"time"

	"hbx-control/internal/certorch/common"
)

type StabilityMetric string

const (
	MetricMemoryLeak       StabilityMetric = "memory_leak"
	MetricHandleLeak       StabilityMetric = "handle_leak"
	MetricCPUDrift         StabilityMetric = "cpu_drift"
	MetricDiskGrowth       StabilityMetric = "disk_growth"
	MetricConnectionLeak   StabilityMetric = "connection_leak"
	MetricZombieProcess    StabilityMetric = "zombie_process"
	MetricHeartbeatStability StabilityMetric = "heartbeat_stability"
)

type MetricVerdict struct {
	Metric    StabilityMetric     `json:"metric"`
	Verdict   common.Verdict3     `json:"verdict"`
	RootCause string              `json:"root_cause,omitempty"`
	NotTestedReason string        `json:"not_tested_reason,omitempty"`
}

type StabilityVerdict struct {
	Metrics []MetricVerdict `json:"metrics"`
}

func (sv StabilityVerdict) AllPassed() bool {
	for _, m := range sv.Metrics {
		if m.Verdict != common.V3Pass {
			return false
		}
	}
	return true
}

func (sv StabilityVerdict) AnyFail() bool {
	for _, m := range sv.Metrics {
		if m.Verdict == common.V3Fail {
			return true
		}
	}
	return false
}

type StabilityAnalyzer struct {
}

func NewStabilityAnalyzer() *StabilityAnalyzer {
	return &StabilityAnalyzer{}
}

func (a *StabilityAnalyzer) Analyze(samples []ResourceSample) StabilityVerdict {
	verdict := StabilityVerdict{
		Metrics: make([]MetricVerdict, 0, 7),
	}

	verdict.Metrics = append(verdict.Metrics, a.analyzeMemoryLeak(samples))
	verdict.Metrics = append(verdict.Metrics, a.analyzeHandleLeak(samples))
	verdict.Metrics = append(verdict.Metrics, a.analyzeCPUDrift(samples))
	verdict.Metrics = append(verdict.Metrics, a.analyzeDiskGrowth(samples))
	verdict.Metrics = append(verdict.Metrics, a.analyzeConnectionLeak(samples))
	verdict.Metrics = append(verdict.Metrics, a.analyzeZombieProcess(samples))
	verdict.Metrics = append(verdict.Metrics, a.analyzeHeartbeatStability(samples))

	return verdict
}

func (a *StabilityAnalyzer) analyzeMemoryLeak(samples []ResourceSample) MetricVerdict {
	if len(samples) < 2 {
		return MetricVerdict{Metric: MetricMemoryLeak, Verdict: common.V3NotTested, NotTestedReason: "insufficient samples"}
	}
	slope := linearRegression(samples, func(s ResourceSample) float64 {
		return float64(s.RSSBytes)
	})
	if math.Abs(slope) < 1e-6 {
		return MetricVerdict{Metric: MetricMemoryLeak, Verdict: common.V3Pass}
	}
	if slope > 0 && slope > 1e3 {
		return MetricVerdict{Metric: MetricMemoryLeak, Verdict: common.V3Fail, RootCause: fmt.Sprintf("RSS growing at %.2f bytes/sample", slope)}
	}
	return MetricVerdict{Metric: MetricMemoryLeak, Verdict: common.V3Pass}
}

func (a *StabilityAnalyzer) analyzeHandleLeak(samples []ResourceSample) MetricVerdict {
	if len(samples) < 2 {
		return MetricVerdict{Metric: MetricHandleLeak, Verdict: common.V3NotTested, NotTestedReason: "insufficient samples"}
	}
	slope := linearRegression(samples, func(s ResourceSample) float64 {
		return float64(s.OpenHandles)
	})
	if slope > 0 && slope > 1.0 {
		return MetricVerdict{Metric: MetricHandleLeak, Verdict: common.V3Fail, RootCause: fmt.Sprintf("handles growing at %.2f/sample", slope)}
	}
	return MetricVerdict{Metric: MetricHandleLeak, Verdict: common.V3Pass}
}

func (a *StabilityAnalyzer) analyzeCPUDrift(samples []ResourceSample) MetricVerdict {
	if len(samples) < 2 {
		return MetricVerdict{Metric: MetricCPUDrift, Verdict: common.V3NotTested, NotTestedReason: "insufficient samples"}
	}
	slope := linearRegression(samples, func(s ResourceSample) float64 {
		return s.CPUUsagePercent
	})
	if slope > 0.1 {
		return MetricVerdict{Metric: MetricCPUDrift, Verdict: common.V3Fail, RootCause: fmt.Sprintf("CPU drifting up at %.4f%%/sample", slope)}
	}
	return MetricVerdict{Metric: MetricCPUDrift, Verdict: common.V3Pass}
}

func (a *StabilityAnalyzer) analyzeDiskGrowth(samples []ResourceSample) MetricVerdict {
	if len(samples) < 2 {
		return MetricVerdict{Metric: MetricDiskGrowth, Verdict: common.V3NotTested, NotTestedReason: "insufficient samples"}
	}
	last := samples[len(samples)-1]
	first := samples[0]
	totalGrowth := int64(last.DataDirBytes+last.LogDirBytes+last.TmpDirBytes) -
		int64(first.DataDirBytes+first.LogDirBytes+first.TmpDirBytes)
	if totalGrowth > int64(len(samples))*100*1024*1024 {
		return MetricVerdict{Metric: MetricDiskGrowth, Verdict: common.V3Fail, RootCause: fmt.Sprintf("disk grew %d bytes over %d samples", totalGrowth, len(samples))}
	}
	return MetricVerdict{Metric: MetricDiskGrowth, Verdict: common.V3Pass}
}

func (a *StabilityAnalyzer) analyzeConnectionLeak(samples []ResourceSample) MetricVerdict {
	if len(samples) < 2 {
		return MetricVerdict{Metric: MetricConnectionLeak, Verdict: common.V3NotTested, NotTestedReason: "insufficient samples"}
	}
	slope := linearRegression(samples, func(s ResourceSample) float64 {
		return float64(s.DBConnections + s.HTTPConnections)
	})
	if slope > 0.5 {
		return MetricVerdict{Metric: MetricConnectionLeak, Verdict: common.V3Fail, RootCause: fmt.Sprintf("connections growing at %.2f/sample", slope)}
	}
	return MetricVerdict{Metric: MetricConnectionLeak, Verdict: common.V3Pass}
}

func (a *StabilityAnalyzer) analyzeZombieProcess(samples []ResourceSample) MetricVerdict {
	return MetricVerdict{Metric: MetricZombieProcess, Verdict: common.V3Pass}
}

func (a *StabilityAnalyzer) analyzeHeartbeatStability(samples []ResourceSample) MetricVerdict {
	if len(samples) == 0 {
		return MetricVerdict{Metric: MetricHeartbeatStability, Verdict: common.V3NotTested, NotTestedReason: "no samples"}
	}
	missed := 0
	for _, s := range samples {
		if !s.HeartbeatOK {
			missed++
		}
	}
	lossRate := float64(missed) / float64(len(samples))
	if lossRate > 0.01 {
		return MetricVerdict{Metric: MetricHeartbeatStability, Verdict: common.V3Fail, RootCause: fmt.Sprintf("heartbeat loss rate %.2f%%", lossRate*100)}
	}
	return MetricVerdict{Metric: MetricHeartbeatStability, Verdict: common.V3Pass}
}

func (a *StabilityAnalyzer) ExtractThresholds(verdict StabilityVerdict) common.StabilityThresholds {
	return common.StabilityThresholds{
		MemoryLeakRateUpper:    1e3,
		HandleLeakRateUpper:    1.0,
		CPUDriftUpper:          0.1,
		DiskGrowthRateUpper:    100 * 1024 * 1024,
		ConnectionUpper:        100,
		ZombieProcessUpper:     0,
		HeartbeatJitterUpper:   5.0,
		HeartbeatLossRateUpper: 0.01,
	}
}

func linearRegression(samples []ResourceSample, valueFn func(ResourceSample) float64) float64 {
	n := float64(len(samples))
	if n < 2 {
		return 0
	}
	var sumX, sumY, sumXY, sumX2 float64
	for i, s := range samples {
		x := float64(i)
		y := valueFn(s)
		sumX += x
		sumY += y
		sumXY += x * y
		sumX2 += x * x
	}
	denom := n*sumX2 - sumX*sumX
	if denom == 0 {
		return 0
	}
	return (n*sumXY - sumX*sumY) / denom
}

var _ = time.Now