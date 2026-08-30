package common

import (
	"context"
	"runtime"
	"time"
)

type CPMetrics struct {
	CollectedAt    time.Time `json:"collected_at"`
	NumGoroutine   int       `json:"num_goroutine"`
	HeapAllocBytes uint64    `json:"heap_alloc_bytes"`
	HeapInUseBytes uint64    `json:"heap_in_use_bytes"`
	StackInUseBytes uint64   `json:"stack_in_use_bytes"`
}

type CPMetricsCollector struct {
}

func NewCPMetricsCollector() *CPMetricsCollector {
	return &CPMetricsCollector{}
}

func (c *CPMetricsCollector) Collect(ctx context.Context) CPMetrics {
	var m runtime.MemStats
	runtime.ReadMemStats(&m)
	return CPMetrics{
		CollectedAt:     time.Now().UTC(),
		NumGoroutine:    runtime.NumGoroutine(),
		HeapAllocBytes:  m.HeapAlloc,
		HeapInUseBytes:  m.HeapInuse,
		StackInUseBytes: m.StackInuse,
	}
}