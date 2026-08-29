package monitor

import (
	"context"
	"fmt"
	"net/http"
	"strings"
	"sync"
	"time"
)

type BadouCollector struct {
	mu         sync.RWMutex
	targets    []BadouMetricsTarget
	lastScrape map[string]string
	client     *http.Client
}

type BadouMetricsTarget struct {
	NodeID   string
	Address  string
	Port     int
}

func NewBadouCollector() *BadouCollector {
	return &BadouCollector{
		lastScrape: make(map[string]string),
		client:     &http.Client{Timeout: 5 * time.Second},
	}
}

func (c *BadouCollector) AddTarget(target BadouMetricsTarget) {
	c.mu.Lock()
	defer c.mu.Unlock()
	for _, t := range c.targets {
		if t.NodeID == target.NodeID {
			return
		}
	}
	c.targets = append(c.targets, target)
}

func (c *BadouCollector) RemoveTarget(nodeID string) {
	c.mu.Lock()
	defer c.mu.Unlock()
	for i, t := range c.targets {
		if t.NodeID == nodeID {
			c.targets = append(c.targets[:i], c.targets[i+1:]...)
			delete(c.lastScrape, nodeID)
			return
		}
	}
}

func (c *BadouCollector) Scrape(ctx context.Context) error {
	c.mu.RLock()
	targets := make([]BadouMetricsTarget, len(c.targets))
	copy(targets, c.targets)
	c.mu.RUnlock()

	for _, t := range targets {
		url := fmt.Sprintf("http://%s:%d/metrics", t.Address, t.Port)
		req, err := http.NewRequestWithContext(ctx, "GET", url, nil)
		if err != nil {
			continue
		}
		resp, err := c.client.Do(req)
		if err != nil {
			continue
		}
		buf := make([]byte, 65536)
		n, _ := resp.Body.Read(buf)
		resp.Body.Close()

		c.mu.Lock()
		c.lastScrape[t.NodeID] = string(buf[:n])
		c.mu.Unlock()
	}
	return nil
}

func (c *BadouCollector) GetMetrics(nodeID string) string {
	c.mu.RLock()
	defer c.mu.RUnlock()
	return c.lastScrape[nodeID]
}

func (c *BadouCollector) GetAllMetrics() map[string]string {
	c.mu.RLock()
	defer c.mu.RUnlock()
	result := make(map[string]string, len(c.lastScrape))
	for k, v := range c.lastScrape {
		result[k] = v
	}
	return result
}

func (c *BadouCollector) AggregateMetrics() string {
	c.mu.RLock()
	defer c.mu.RUnlock()

	var sb strings.Builder
	for nodeID, metrics := range c.lastScrape {
		sb.WriteString(fmt.Sprintf("# Badou node %s\n", nodeID))
		sb.WriteString(metrics)
		sb.WriteString("\n")
	}
	return sb.String()
}