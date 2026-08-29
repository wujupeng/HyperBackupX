package service

import (
	"bytes"
	"context"
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"time"

	"github.com/google/uuid"

	"hbx-control/internal/badou/model"
)

type RealBadouClient struct {
	endpoint string
	client   *http.Client
	jwtToken string
}

func NewRealBadouClient(endpoint string, jwtToken string, timeout time.Duration) *RealBadouClient {
	return &RealBadouClient{
		endpoint: endpoint,
		client:   &http.Client{Timeout: timeout},
		jwtToken: jwtToken,
	}
}

func (c *RealBadouClient) Close() error {
	c.client.CloseIdleConnections()
	return nil
}

func (c *RealBadouClient) doRequest(ctx context.Context, method, path string, body interface{}) (*http.Response, error) {
	url := fmt.Sprintf("%s%s", c.endpoint, path)

	var bodyReader io.Reader
	if body != nil {
		data, err := json.Marshal(body)
		if err != nil {
			return nil, fmt.Errorf("marshal request: %w", err)
		}
		bodyReader = bytes.NewReader(data)
	}

	req, err := http.NewRequestWithContext(ctx, method, url, bodyReader)
	if err != nil {
		return nil, fmt.Errorf("create request: %w", err)
	}

	if body != nil {
		req.Header.Set("Content-Type", "application/json")
	}
	if c.jwtToken != "" {
		req.Header.Set("Authorization", "Bearer "+c.jwtToken)
	}

	resp, err := c.client.Do(req)
	if err != nil {
		return nil, fmt.Errorf("http request: %w", err)
	}

	if resp.StatusCode >= 400 {
		respBody, _ := io.ReadAll(resp.Body)
		resp.Body.Close()
		return nil, fmt.Errorf("badou server error %d: %s", resp.StatusCode, string(respBody))
	}

	return resp, nil
}

func (c *RealBadouClient) ListVersions(ctx context.Context, repoID uuid.UUID) ([]model.Version, error) {
	resp, err := c.doRequest(ctx, "GET", fmt.Sprintf("/api/v1/repos/%s/versions", repoID.String()), nil)
	if err != nil {
		return nil, err
	}
	defer resp.Body.Close()

	var result struct {
		Versions []model.Version `json:"versions"`
	}
	if err := json.NewDecoder(resp.Body).Decode(&result); err != nil {
		return nil, fmt.Errorf("decode response: %w", err)
	}
	return result.Versions, nil
}

func (c *RealBadouClient) DeleteVersion(ctx context.Context, repoID uuid.UUID, versionID string) error {
	resp, err := c.doRequest(ctx, "DELETE", fmt.Sprintf("/api/v1/repos/%s/versions/%s", repoID.String(), versionID), nil)
	if err != nil {
		return err
	}
	defer resp.Body.Close()

	var result struct {
		Deleted bool `json:"deleted"`
	}
	if err := json.NewDecoder(resp.Body).Decode(&result); err != nil {
		return fmt.Errorf("decode response: %w", err)
	}
	if !result.Deleted {
		return fmt.Errorf("version %s not found", versionID)
	}
	return nil
}

func (c *RealBadouClient) VerifyRepository(ctx context.Context, repoID uuid.UUID, level string) (*model.VerifyResult, error) {
	resp, err := c.doRequest(ctx, "POST", fmt.Sprintf("/api/v1/repos/%s/verify", repoID.String()), map[string]string{"level": level})
	if err != nil {
		return nil, err
	}
	defer resp.Body.Close()

	var result struct {
		RepoID      string `json:"repo_id"`
		Passed      bool   `json:"passed"`
		TotalChecked int64 `json:"total_checked"`
		TotalFailed  int64 `json:"total_failed"`
	}
	if err := json.NewDecoder(resp.Body).Decode(&result); err != nil {
		return nil, fmt.Errorf("decode response: %w", err)
	}
	return &model.VerifyResult{
		RepoID: result.RepoID,
		Level:  level,
		Passed: result.Passed,
		Errors: int(result.TotalFailed),
	}, nil
}

func (c *RealBadouClient) TriggerGC(ctx context.Context, repoID uuid.UUID) (int64, int64, int64, int64, error) {
	resp, err := c.doRequest(ctx, "POST", fmt.Sprintf("/api/v1/repos/%s/gc", repoID.String()), nil)
	if err != nil {
		return 0, 0, 0, 0, err
	}
	defer resp.Body.Close()

	var result struct {
		ChunksScanned int64 `json:"chunks_scanned"`
		ChunksDeleted int64 `json:"chunks_deleted"`
		BytesFreed    int64 `json:"bytes_freed"`
		DurationMs    int64 `json:"duration_ms"`
	}
	if err := json.NewDecoder(resp.Body).Decode(&result); err != nil {
		return 0, 0, 0, 0, fmt.Errorf("decode response: %w", err)
	}
	return result.ChunksScanned, result.ChunksDeleted, result.BytesFreed, result.DurationMs, nil
}

func (c *RealBadouClient) GetClusterHealth(ctx context.Context, nodeAddr string, nodePort int) (*model.ClusterHealth, error) {
	url := fmt.Sprintf("http://%s:%d/health", nodeAddr, nodePort)
	req, err := http.NewRequestWithContext(ctx, "GET", url, nil)
	if err != nil {
		return nil, fmt.Errorf("create request: %w", err)
	}

	resp, err := c.client.Do(req)
	if err != nil {
		return &model.ClusterHealth{
			Status:      "unhealthy",
			TotalNodes:  1,
			OnlineNodes: 0,
			Nodes:       []model.NodeHealth{},
		}, nil
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		return &model.ClusterHealth{
			Status:      "unhealthy",
			TotalNodes:  1,
			OnlineNodes: 0,
			Nodes:       []model.NodeHealth{},
		}, nil
	}

	var health struct {
		Status   string `json:"status"`
		DataRoot string `json:"data_root"`
	}
	if err := json.NewDecoder(resp.Body).Decode(&health); err != nil {
		return nil, fmt.Errorf("decode health response: %w", err)
	}

	return &model.ClusterHealth{
		Status:      "healthy",
		TotalNodes:  1,
		OnlineNodes: 1,
		Nodes: []model.NodeHealth{
			{
				NodeID:  "node-1",
				Address: nodeAddr,
				Status:  "online",
				Healthy: true,
			},
		},
	}, nil
}

func (c *RealBadouClient) ScrapeMetrics(ctx context.Context, nodeAddr string, nodePort int) (string, error) {
	url := fmt.Sprintf("http://%s:%d/metrics", nodeAddr, nodePort)
	req, err := http.NewRequestWithContext(ctx, "GET", url, nil)
	if err != nil {
		return "", fmt.Errorf("create request: %w", err)
	}

	resp, err := c.client.Do(req)
	if err != nil {
		return "", fmt.Errorf("scrape metrics: %w", err)
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		return "", fmt.Errorf("metrics endpoint returned %d", resp.StatusCode)
	}

	buf := make([]byte, 65536)
	n, _ := resp.Body.Read(buf)
	return string(buf[:n]), nil
}