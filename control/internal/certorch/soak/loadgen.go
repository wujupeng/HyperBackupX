package soak

import (
	"context"
	"sync"
	"time"
)

type LoadPattern struct {
	BackupInterval       time.Duration `json:"backup_interval"`
	IncrementalInterval  time.Duration `json:"incremental_interval"`
	RestoreInterval      time.Duration `json:"restore_interval"`
}

type ResourceSample struct {
	Timestamp       time.Time `json:"timestamp"`
	RSSBytes        uint64    `json:"rss_bytes"`
	OpenHandles     uint64    `json:"open_handles"`
	CPUUsagePercent float64   `json:"cpu_usage_percent"`
	DBConnections   uint64    `json:"db_connections"`
	HTTPConnections uint64    `json:"http_connections"`
	DataDirBytes    uint64    `json:"data_dir_bytes"`
	LogDirBytes     uint64    `json:"log_dir_bytes"`
	TmpDirBytes     uint64    `json:"tmp_dir_bytes"`
	HeartbeatOK     bool      `json:"heartbeat_ok"`
}

type LoadGenerator struct {
	mu        sync.Mutex
	samples   []ResourceSample
	anomalies []AnomalyEvent
	running   bool
}

func NewLoadGenerator() *LoadGenerator {
	return &LoadGenerator{}
}

func (g *LoadGenerator) Start(ctx context.Context, pattern LoadPattern) error {
	g.mu.Lock()
	g.running = true
	g.samples = nil
	g.anomalies = nil
	g.mu.Unlock()

	go g.runLoadLoop(ctx, pattern)
	return nil
}

func (g *LoadGenerator) runLoadLoop(ctx context.Context, pattern LoadPattern) {
	ticker := time.NewTicker(5 * time.Second)
	defer ticker.Stop()

	backupTicker := time.NewTicker(pattern.BackupInterval)
	defer backupTicker.Stop()

	for {
		select {
		case <-ctx.Done():
			g.mu.Lock()
			g.running = false
			g.mu.Unlock()
			return
		case <-ticker.C:
			g.collectSample()
		case <-backupTicker.C:
			// trigger backup job via Control Plane API
		}
	}
}

func (g *LoadGenerator) collectSample() {
	now := time.Now()
	sample := ResourceSample{
		Timestamp:   now,
		HeartbeatOK: true,
	}
	g.mu.Lock()
	g.samples = append(g.samples, sample)
	g.mu.Unlock()
}

func (g *LoadGenerator) CollectSamples(ctx context.Context, duration time.Duration) []ResourceSample {
	deadline := time.Now().Add(duration)
	ticker := time.NewTicker(5 * time.Second)
	defer ticker.Stop()

	for {
		select {
		case <-ctx.Done():
			break
		case <-ticker.C:
			if time.Now().After(deadline) {
				goto done
			}
			g.collectSample()
		}
	}
done:
	g.mu.Lock()
	defer g.mu.Unlock()
	return g.samples
}

func (g *LoadGenerator) GetAnomalies() []AnomalyEvent {
	g.mu.Lock()
	defer g.mu.Unlock()
	result := make([]AnomalyEvent, len(g.anomalies))
	copy(result, g.anomalies)
	return result
}

func (g *LoadGenerator) IsRunning() bool {
	g.mu.Lock()
	defer g.mu.Unlock()
	return g.running
}