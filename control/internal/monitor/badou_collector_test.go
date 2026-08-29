package monitor

import (
	"context"
	"testing"
)

func TestBadouCollectorAddRemoveTarget(t *testing.T) {
	c := NewBadouCollector()

	target := BadouMetricsTarget{
		NodeID:  "node-1",
		Address: "192.168.1.60",
		Port:    9090,
	}
	c.AddTarget(target)

	c.mu.RLock()
	count := len(c.targets)
	c.mu.RUnlock()
	if count != 1 {
		t.Errorf("expected 1 target, got %d", count)
	}

	c.AddTarget(target)
	c.mu.RLock()
	count = len(c.targets)
	c.mu.RUnlock()
	if count != 1 {
		t.Errorf("expected 1 target after duplicate add, got %d", count)
	}

	c.RemoveTarget("node-1")
	c.mu.RLock()
	count = len(c.targets)
	c.mu.RUnlock()
	if count != 0 {
		t.Errorf("expected 0 targets after remove, got %d", count)
	}
}

func TestBadouCollectorGetMetricsEmpty(t *testing.T) {
	c := NewBadouCollector()
	if m := c.GetMetrics("nonexistent"); m != "" {
		t.Errorf("expected empty string for nonexistent node, got %s", m)
	}
}

func TestBadouCollectorGetAllMetricsEmpty(t *testing.T) {
	c := NewBadouCollector()
	m := c.GetAllMetrics()
	if len(m) != 0 {
		t.Errorf("expected empty map, got %d entries", len(m))
	}
}

func TestBadouCollectorAggregateMetricsEmpty(t *testing.T) {
	c := NewBadouCollector()
	if m := c.AggregateMetrics(); m != "" {
		t.Errorf("expected empty string, got %s", m)
	}
}

func TestBadouCollectorScrapeNoTargets(t *testing.T) {
	c := NewBadouCollector()
	err := c.Scrape(context.Background())
	if err != nil {
		t.Errorf("unexpected error: %v", err)
	}
}