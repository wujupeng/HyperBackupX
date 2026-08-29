// stub_client.go — StubClient is retained for unit tests only.
// Production code MUST use RealBadouClient (see real_client.go).
// NewService() accepts a BadouClient interface; tests inject StubClient,
// production injects RealBadouClient via NewRealBadouClient().
package service

import (
	"context"
	"fmt"
	"net/http"
	"time"

	"github.com/google/uuid"

	"hbx-control/internal/badou/model"
)

type StubClient struct{}

func NewStubClient() *StubClient {
	return &StubClient{}
}

func (s *StubClient) ListVersions(_ context.Context, _ uuid.UUID) ([]model.Version, error) {
	return []model.Version{}, nil
}

func (s *StubClient) DeleteVersion(_ context.Context, _ uuid.UUID, _ string) error {
	return nil
}

func (s *StubClient) VerifyRepository(_ context.Context, repoID uuid.UUID, level string) (*model.VerifyResult, error) {
	return &model.VerifyResult{
		RepoID: repoID.String(),
		Level:  level,
		Passed: true,
	}, nil
}

func (s *StubClient) TriggerGC(_ context.Context, _ uuid.UUID) (int64, int64, int64, int64, error) {
	return 0, 0, 0, 0, nil
}

func (s *StubClient) GetClusterHealth(_ context.Context, _ string, _ int) (*model.ClusterHealth, error) {
	return &model.ClusterHealth{
		Status:      "healthy",
		TotalNodes:  1,
		OnlineNodes: 1,
		Nodes:       []model.NodeHealth{},
	}, nil
}

func (s *StubClient) ScrapeMetrics(_ context.Context, nodeAddr string, nodePort int) (string, error) {
	url := fmt.Sprintf("http://%s:%d/metrics", nodeAddr, nodePort)
	client := &http.Client{Timeout: 5 * time.Second}
	resp, err := client.Get(url)
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