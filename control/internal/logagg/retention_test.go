package logagg

import (
	"context"
	"fmt"
	"testing"
	"time"
)

func TestDefaultRetentionPolicy(t *testing.T) {
	p := DefaultRetentionPolicy()
	if p.DailyRetention != 7 {
		t.Fatalf("Expected 7 daily, got %d", p.DailyRetention)
	}
	if p.WeeklyRetention != 4 {
		t.Fatalf("Expected 4 weekly, got %d", p.WeeklyRetention)
	}
	if p.MonthlyRetention != 12 {
		t.Fatalf("Expected 12 monthly, got %d", p.MonthlyRetention)
	}
}

func TestGetExpiredPartitions(t *testing.T) {
	m := NewRetentionManager(RetentionPolicy{DailyRetention: 7, WeeklyRetention: 4, MonthlyRetention: 12})
	now := time.Now().UTC()

	m.AddPartition(Partition{Name: "p_recent", StartDate: now.AddDate(0, 0, -1), EndDate: now})
	m.AddPartition(Partition{Name: "p_old", StartDate: now.AddDate(0, 0, -30), EndDate: now.AddDate(0, 0, -29)})
	m.AddPartition(Partition{Name: "p_archived", StartDate: now.AddDate(0, 0, -30), EndDate: now.AddDate(0, 0, -29), IsArchived: true})

	expired := m.GetExpiredPartitions(now)
	if len(expired) != 1 {
		t.Fatalf("Expected 1 expired, got %d", len(expired))
	}
	if expired[0].Name != "p_old" {
		t.Fatalf("Expected p_old, got %s", expired[0].Name)
	}
}

func TestArchivePartition(t *testing.T) {
	m := NewRetentionManager(DefaultRetentionPolicy())
	m.AddPartition(Partition{Name: "p1", StartDate: time.Now(), EndDate: time.Now()})

	if !m.ArchivePartition("p1") {
		t.Fatal("ArchivePartition failed")
	}
	partitions := m.ListPartitions()
	if !partitions[0].IsArchived {
		t.Fatal("Partition should be archived")
	}
}

func TestCleanupExpired(t *testing.T) {
	m := NewRetentionManager(RetentionPolicy{DailyRetention: 1, WeeklyRetention: 0, MonthlyRetention: 0})
	now := time.Now().UTC()

	m.AddPartition(Partition{Name: "p1", StartDate: now.AddDate(0, 0, -10), EndDate: now.AddDate(0, 0, -9)})
	m.AddPartition(Partition{Name: "p2", StartDate: now.AddDate(0, 0, -10), EndDate: now.AddDate(0, 0, -9)})

	dropped := []string{}
	dropFn := func(name string) error {
		dropped = append(dropped, name)
		return nil
	}

	cleaned, err := m.CleanupExpired(context.Background(), dropFn)
	if err != nil {
		t.Fatalf("CleanupExpired failed: %v", err)
	}
	if cleaned != 2 {
		t.Fatalf("Expected 2 cleaned, got %d", cleaned)
	}
	if len(dropped) != 2 {
		t.Fatalf("Expected 2 dropped, got %d", len(dropped))
	}
}

func TestCleanupExpiredWithError(t *testing.T) {
	m := NewRetentionManager(RetentionPolicy{DailyRetention: 1, WeeklyRetention: 0, MonthlyRetention: 0})
	now := time.Now().UTC()
	m.AddPartition(Partition{Name: "p1", StartDate: now.AddDate(0, 0, -10), EndDate: now.AddDate(0, 0, -9)})

	dropFn := func(name string) error {
		return fmt.Errorf("drop failed")
	}

	_, err := m.CleanupExpired(context.Background(), dropFn)
	if err == nil {
		t.Fatal("Expected error")
	}
}

func TestGeneratePartitionName(t *testing.T) {
	name := GeneratePartitionName("agent_logs", time.Date(2024, 3, 15, 0, 0, 0, 0, time.UTC))
	if name != "agent_logs_20240315" {
		t.Fatalf("Expected agent_logs_20240315, got %s", name)
	}
}

func TestEnsurePartition(t *testing.T) {
	m := NewRetentionManager(DefaultRetentionPolicy())
	date := time.Date(2024, 3, 15, 12, 0, 0, 0, time.UTC)

	p1 := m.EnsurePartition("agent_logs", date)
	p2 := m.EnsurePartition("agent_logs", date)

	if p1.Name != p2.Name {
		t.Fatal("Should return same partition for same date")
	}
	if len(m.ListPartitions()) != 1 {
		t.Fatal("Should have 1 partition")
	}
}

func TestSetPolicy(t *testing.T) {
	m := NewRetentionManager(DefaultRetentionPolicy())
	m.SetPolicy(RetentionPolicy{DailyRetention: 30, WeeklyRetention: 12, MonthlyRetention: 24})
	p := m.GetPolicy()
	if p.DailyRetention != 30 {
		t.Fatalf("Expected 30, got %d", p.DailyRetention)
	}
}