package service

import (
	"context"
	"testing"

	"github.com/google/uuid"
)

func TestStubClientListVersions(t *testing.T) {
	client := NewStubClient()
	versions, err := client.ListVersions(context.Background(), uuid.New())
	if err != nil {
		t.Errorf("unexpected error: %v", err)
	}
	if len(versions) != 0 {
		t.Errorf("expected empty versions, got %d", len(versions))
	}
}

func TestStubClientDeleteVersion(t *testing.T) {
	client := NewStubClient()
	err := client.DeleteVersion(context.Background(), uuid.New(), "v1")
	if err != nil {
		t.Errorf("unexpected error: %v", err)
	}
}

func TestStubClientVerifyRepository(t *testing.T) {
	client := NewStubClient()
	repoID := uuid.New()
	result, err := client.VerifyRepository(context.Background(), repoID, "full")
	if err != nil {
		t.Errorf("unexpected error: %v", err)
	}
	if result == nil {
		t.Fatal("expected non-nil result")
	}
	if !result.Passed {
		t.Error("expected verify to pass")
	}
	if result.RepoID != repoID.String() {
		t.Errorf("expected repo ID %s, got %s", repoID.String(), result.RepoID)
	}
}

func TestStubClientTriggerGC(t *testing.T) {
	client := NewStubClient()
	scanned, deleted, freed, duration, err := client.TriggerGC(context.Background(), uuid.New())
	if err != nil {
		t.Errorf("unexpected error: %v", err)
	}
	if scanned != 0 || deleted != 0 || freed != 0 || duration != 0 {
		t.Error("expected zero values from stub GC")
	}
}

func TestStubClientGetClusterHealth(t *testing.T) {
	client := NewStubClient()
	health, err := client.GetClusterHealth(context.Background(), "localhost", 50051)
	if err != nil {
		t.Errorf("unexpected error: %v", err)
	}
	if health == nil {
		t.Fatal("expected non-nil health")
	}
	if health.Status != "healthy" {
		t.Errorf("expected healthy, got %s", health.Status)
	}
	if health.TotalNodes != 1 || health.OnlineNodes != 1 {
		t.Error("expected 1 total and 1 online node")
	}
}