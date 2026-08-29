package service

import (
	"context"
	"time"

	"github.com/google/uuid"

	"hbx-control/internal/badou/model"
	"hbx-control/internal/badou/repository"
)

type BadouClient interface {
	ListVersions(ctx context.Context, repoID uuid.UUID) ([]model.Version, error)
	DeleteVersion(ctx context.Context, repoID uuid.UUID, versionID string) error
	VerifyRepository(ctx context.Context, repoID uuid.UUID, level string) (*model.VerifyResult, error)
	TriggerGC(ctx context.Context, repoID uuid.UUID) (int64, int64, int64, int64, error)
	GetClusterHealth(ctx context.Context, nodeAddr string, nodePort int) (*model.ClusterHealth, error)
	ScrapeMetrics(ctx context.Context, nodeAddr string, nodePort int) (string, error)
}

type Service struct {
	repo   *repository.BadouRepo
	client BadouClient
}

func NewService(repo *repository.BadouRepo, client BadouClient) *Service {
	return &Service{repo: repo, client: client}
}

func (s *Service) ListRepositories(ctx context.Context) ([]model.Repository, error) {
	return s.repo.ListRepositories(ctx)
}

func (s *Service) GetRepository(ctx context.Context, id uuid.UUID) (*model.Repository, error) {
	return s.repo.GetRepository(ctx, id)
}

func (s *Service) CreateRepository(ctx context.Context, req model.CreateRepositoryRequest) (*model.Repository, error) {
	return s.repo.CreateRepository(ctx, req)
}

func (s *Service) UpdateRepository(ctx context.Context, id uuid.UUID, req model.UpdateRepositoryRequest) error {
	return s.repo.UpdateRepository(ctx, id, req)
}

func (s *Service) DeleteRepository(ctx context.Context, id uuid.UUID) error {
	return s.repo.DeleteRepository(ctx, id)
}

func (s *Service) SetImmutableRetention(ctx context.Context, id uuid.UUID, days int) error {
	return s.repo.SetImmutableRetention(ctx, id, days)
}

func (s *Service) ListVersions(ctx context.Context, repoID uuid.UUID) ([]model.Version, error) {
	return s.client.ListVersions(ctx, repoID)
}

func (s *Service) DeleteVersion(ctx context.Context, repoID uuid.UUID, versionID string) error {
	return s.client.DeleteVersion(ctx, repoID, versionID)
}

func (s *Service) VerifyRepository(ctx context.Context, repoID uuid.UUID, level string) (*model.VerifyResult, error) {
	if level == "" {
		level = "full"
	}
	return s.client.VerifyRepository(ctx, repoID, level)
}

func (s *Service) TriggerGC(ctx context.Context, repoID uuid.UUID, triggeredBy string) (*model.GCReport, error) {
	reportID, err := s.repo.CreateGCReport(ctx, repoID, triggeredBy)
	if err != nil {
		return nil, err
	}
	scanned, deleted, freed, durationMs, err := s.client.TriggerGC(ctx, repoID)
	status := "completed"
	if err != nil {
		status = "failed"
	}
	_ = s.repo.UpdateGCReport(ctx, reportID, scanned, deleted, freed, durationMs, status)
	report, _ := s.repo.GetGCReport(ctx, repoID)
	if report == nil {
		report = &model.GCReport{
			ReportID:      reportID,
			RepoID:        repoID,
			TriggeredBy:   triggeredBy,
			ChunksScanned: scanned,
			ChunksDeleted: deleted,
			BytesFreed:    freed,
			DurationMs:    durationMs,
			Status:        status,
			StartedAt:     time.Now().UTC(),
		}
	}
	return report, nil
}

func (s *Service) GetGCReport(ctx context.Context, repoID uuid.UUID) (*model.GCReport, error) {
	return s.repo.GetGCReport(ctx, repoID)
}

func (s *Service) ListNodes(ctx context.Context) ([]model.Node, error) {
	return s.repo.ListNodes(ctx)
}

func (s *Service) AddNode(ctx context.Context, req model.AddNodeRequest) (*model.Node, error) {
	return s.repo.AddNode(ctx, req)
}

func (s *Service) RemoveNode(ctx context.Context, id uuid.UUID) error {
	return s.repo.RemoveNode(ctx, id)
}

func (s *Service) GetClusterHealth(ctx context.Context, nodeAddr string, nodePort int) (*model.ClusterHealth, error) {
	return s.client.GetClusterHealth(ctx, nodeAddr, nodePort)
}

func (s *Service) ScrapeMetrics(ctx context.Context, nodeAddr string, nodePort int) (string, error) {
	return s.client.ScrapeMetrics(ctx, nodeAddr, nodePort)
}