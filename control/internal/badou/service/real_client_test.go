package service

import (
	"context"
	"testing"
	"time"

	"github.com/google/uuid"
)

func TestRealBadouClientConstruction(t *testing.T) {
	client := NewRealBadouClient("http://localhost:9092", "test-jwt", 30*time.Second)
	if client == nil {
		t.Fatal("NewRealBadouClient returned nil")
	}
	if client.endpoint != "http://localhost:9092" {
		t.Errorf("endpoint = %s, want http://localhost:9092", client.endpoint)
	}
	if client.jwtToken != "test-jwt" {
		t.Errorf("jwtToken = %s, want test-jwt", client.jwtToken)
	}
}

func TestRealBadouClientClose(t *testing.T) {
	client := NewRealBadouClient("http://localhost:9092", "test-jwt", 30*time.Second)
	if err := client.Close(); err != nil {
		t.Errorf("Close() error = %v", err)
	}
}

func TestRealBadouClientListVersionsConnectionFailure(t *testing.T) {
	client := NewRealBadouClient("http://127.0.0.1:1", "test-jwt", 1*time.Second)
	ctx, cancel := context.WithTimeout(context.Background(), 2*time.Second)
	defer cancel()

	repoID := uuid.New()
	_, err := client.ListVersions(ctx, repoID)
	if err == nil {
		t.Error("ListVersions with unreachable endpoint should return error, not nil")
	}
}

func TestRealBadouClientVerifyRepositoryConnectionFailure(t *testing.T) {
	client := NewRealBadouClient("http://127.0.0.1:1", "test-jwt", 1*time.Second)
	ctx, cancel := context.WithTimeout(context.Background(), 2*time.Second)
	defer cancel()

	repoID := uuid.New()
	_, err := client.VerifyRepository(ctx, repoID, "full")

	if err == nil {
		t.Error("VerifyRepository with unreachable endpoint should return error, not nil")
	}
}

func TestRealBadouClientTriggerGCConnectionFailure(t *testing.T) {
	client := NewRealBadouClient("http://127.0.0.1:1", "test-jwt", 1*time.Second)
	ctx, cancel := context.WithTimeout(context.Background(), 2*time.Second)
	defer cancel()

	repoID := uuid.New()
	_, _, _, _, err := client.TriggerGC(ctx, repoID)
	if err == nil {
		t.Error("TriggerGC with unreachable endpoint should return error, not nil")
	}
}

func TestRealBadouClientImplementsBadouClient(t *testing.T) {
	var _ BadouClient = (*RealBadouClient)(nil)
}