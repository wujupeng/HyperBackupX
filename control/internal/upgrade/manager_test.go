package upgrade

import (
	"os"
	"path/filepath"
	"testing"

	"github.com/google/uuid"
)

func TestCreateUpgradeTask(t *testing.T) {
	m := NewManager("")
	deviceID := uuid.New()
	task := m.CreateUpgradeTask(deviceID, "0.1.0", "0.2.0", "http://example.com/agent-0.2.0.exe", "abc123")

	if task.Status != StatusDownloading {
		t.Fatalf("Expected downloading, got %s", task.Status)
	}
	if task.FromVersion != "0.1.0" {
		t.Fatalf("Expected from 0.1.0, got %s", task.FromVersion)
	}
}

func TestBackupAndRollback(t *testing.T) {
	dir := t.TempDir()
	m := NewManager(dir)

	currentBinary := filepath.Join(dir, "current-agent.exe")
	os.WriteFile(currentBinary, []byte("old binary content"), 0755)

	deviceID := uuid.New()
	task := m.CreateUpgradeTask(deviceID, "0.1.0", "0.2.0", "", "")

	if err := m.BackupCurrentBinary(task.ID, currentBinary); err != nil {
		t.Fatalf("BackupCurrentBinary failed: %v", err)
	}

	got, _ := m.GetTask(task.ID)
	if got.BackupPath == "" {
		t.Fatal("BackupPath should be set")
	}

	backupContent, err := os.ReadFile(got.BackupPath)
	if err != nil {
		t.Fatalf("Read backup failed: %v", err)
	}
	if string(backupContent) != "old binary content" {
		t.Fatal("Backup content mismatch")
	}

	os.WriteFile(currentBinary, []byte("new binary content"), 0755)

	if err := m.Rollback(task.ID, currentBinary); err != nil {
		t.Fatalf("Rollback failed: %v", err)
	}

	rolled, _ := m.GetTask(task.ID)
	if rolled.Status != StatusRolledBack {
		t.Fatalf("Expected rolled_back, got %s", rolled.Status)
	}

	restoredContent, _ := os.ReadFile(currentBinary)
	if string(restoredContent) != "old binary content" {
		t.Fatal("Rollback should restore old content")
	}
}

func TestRollbackNoBackup(t *testing.T) {
	m := NewManager("")
	deviceID := uuid.New()
	task := m.CreateUpgradeTask(deviceID, "0.1.0", "0.2.0", "", "")

	err := m.Rollback(task.ID, "/tmp/nonexistent")
	if err == nil {
		t.Fatal("Should fail without backup")
	}
}

func TestReplaceBinary(t *testing.T) {
	dir := t.TempDir()
	m := NewManager(dir)

	currentBinary := filepath.Join(dir, "agent.exe")
	os.WriteFile(currentBinary, []byte("old"), 0755)

	newBinary := filepath.Join(dir, "new-agent.exe")
	os.WriteFile(newBinary, []byte("new"), 0755)

	deviceID := uuid.New()
	task := m.CreateUpgradeTask(deviceID, "0.1.0", "0.2.0", "", "")
	task.NewBinaryPath = newBinary

	if err := m.ReplaceBinary(task.ID, currentBinary); err != nil {
		t.Fatalf("ReplaceBinary failed: %v", err)
	}

	content, _ := os.ReadFile(currentBinary)
	if string(content) != "new" {
		t.Fatal("Binary should be replaced")
	}

	got, _ := m.GetTask(task.ID)
	if got.Status != StatusRestarting {
		t.Fatalf("Expected restarting, got %s", got.Status)
	}
}

func TestCompleteUpgrade(t *testing.T) {
	m := NewManager("")
	deviceID := uuid.New()
	task := m.CreateUpgradeTask(deviceID, "0.1.0", "0.2.0", "", "")

	m.CompleteUpgrade(task.ID)

	got, _ := m.GetTask(task.ID)
	if got.Status != StatusCompleted {
		t.Fatalf("Expected completed, got %s", got.Status)
	}
	if got.CompletedAt == nil {
		t.Fatal("CompletedAt should be set")
	}
}

func TestListTasksByDevice(t *testing.T) {
	m := NewManager("")
	d1 := uuid.New()
	d2 := uuid.New()

	m.CreateUpgradeTask(d1, "0.1.0", "0.2.0", "", "")
	m.CreateUpgradeTask(d1, "0.1.0", "0.3.0", "", "")
	m.CreateUpgradeTask(d2, "0.1.0", "0.2.0", "", "")

	tasksD1 := m.ListTasksByDevice(d1)
	if len(tasksD1) != 2 {
		t.Fatalf("Expected 2 tasks for d1, got %d", len(tasksD1))
	}
}

func TestMarkFailed(t *testing.T) {
	m := NewManager("")
	deviceID := uuid.New()
	task := m.CreateUpgradeTask(deviceID, "0.1.0", "0.2.0", "", "")

	m.markFailed(task.ID, "test error")

	got, _ := m.GetTask(task.ID)
	if got.Status != StatusFailed {
		t.Fatalf("Expected failed, got %s", got.Status)
	}
	if got.ErrorMsg != "test error" {
		t.Fatalf("Expected 'test error', got '%s'", got.ErrorMsg)
	}
}

func TestListTasks(t *testing.T) {
	m := NewManager("")
	m.CreateUpgradeTask(uuid.New(), "0.1.0", "0.2.0", "", "")
	m.CreateUpgradeTask(uuid.New(), "0.1.0", "0.3.0", "", "")

	tasks := m.ListTasks()
	if len(tasks) != 2 {
		t.Fatalf("Expected 2 tasks, got %d", len(tasks))
	}
}