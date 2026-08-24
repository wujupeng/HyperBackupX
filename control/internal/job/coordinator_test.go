package job

import (
	"testing"

	"github.com/google/uuid"
)

func TestCreateJob(t *testing.T) {
	c := NewCoordinator()
	deviceID := uuid.New()
	job, err := c.CreateJob(deviceID, "backup-daily", map[string]interface{}{"path": "/data"}, nil)
	if err != nil {
		t.Fatalf("CreateJob failed: %v", err)
	}
	if job.Status != StatusPending {
		t.Fatalf("Expected pending, got %s", job.Status)
	}
}

func TestCreateJobEmptyName(t *testing.T) {
	c := NewCoordinator()
	_, err := c.CreateJob(uuid.New(), "", nil, nil)
	if err == nil {
		t.Fatal("Should fail with empty name")
	}
}

func TestTriggerJob(t *testing.T) {
	c := NewCoordinator()
	deviceID := uuid.New()
	job, _ := c.CreateJob(deviceID, "backup", nil, nil)

	cmd, err := c.TriggerJob(job.ID)
	if err != nil {
		t.Fatalf("TriggerJob failed: %v", err)
	}
	if cmd.Type != CmdTriggerBackup {
		t.Fatalf("Expected trigger_backup, got %s", cmd.Type)
	}

	pending := c.GetPendingCommands(deviceID)
	if len(pending) != 1 {
		t.Fatalf("Expected 1 pending command, got %d", len(pending))
	}

	updated, _ := c.GetJob(job.ID)
	if updated.Status != StatusRunning {
		t.Fatalf("Expected running, got %s", updated.Status)
	}
}

func TestTriggerJobNotPending(t *testing.T) {
	c := NewCoordinator()
	deviceID := uuid.New()
	job, _ := c.CreateJob(deviceID, "backup", nil, nil)
	c.TriggerJob(job.ID)

	_, err := c.TriggerJob(job.ID)
	if err == nil {
		t.Fatal("Should fail to trigger non-pending job")
	}
}

func TestRecordResult(t *testing.T) {
	c := NewCoordinator()
	deviceID := uuid.New()
	job, _ := c.CreateJob(deviceID, "backup", nil, nil)
	c.TriggerJob(job.ID)

	result := TaskResult{
		JobID:          job.ID,
		DeviceID:       deviceID,
		Status:         StatusCompleted,
		BytesProcessed: 1024,
		BytesStored:    512,
		FileCount:      10,
		ChunkCount:     5,
		DedupRatio:     0.5,
	}
	if err := c.RecordResult(result); err != nil {
		t.Fatalf("RecordResult failed: %v", err)
	}

	completed, _ := c.GetJob(job.ID)
	if completed.Status != StatusCompleted {
		t.Fatalf("Expected completed, got %s", completed.Status)
	}
}

func TestDistributePolicy(t *testing.T) {
	c := NewCoordinator()
	deviceID := uuid.New()
	policyID := uuid.New()

	cmd := c.DistributePolicy(deviceID, policyID, map[string]interface{}{"mode": "daily"})
	if cmd.Type != CmdPolicyUpdate {
		t.Fatalf("Expected policy_update, got %s", cmd.Type)
	}

	pending := c.GetPendingCommands(deviceID)
	if len(pending) != 1 {
		t.Fatalf("Expected 1 pending, got %d", len(pending))
	}
}

func TestDistributePolicyToGroup(t *testing.T) {
	c := NewCoordinator()
	policyID := uuid.New()
	devices := []uuid.UUID{uuid.New(), uuid.New(), uuid.New()}

	cmds := c.DistributePolicyToGroup(devices, policyID, nil)
	if len(cmds) != 3 {
		t.Fatalf("Expected 3 commands, got %d", len(cmds))
	}

	for _, d := range devices {
		pending := c.GetPendingCommands(d)
		if len(pending) != 1 {
			t.Fatalf("Expected 1 pending per device, got %d", len(pending))
		}
	}
}

func TestAggregateStatus(t *testing.T) {
	c := NewCoordinator()
	deviceID := uuid.New()

	j1, _ := c.CreateJob(deviceID, "j1", nil, nil)
	j2, _ := c.CreateJob(deviceID, "j2", nil, nil)
	j3, _ := c.CreateJob(deviceID, "j3", nil, nil)
	_ = j3

	c.TriggerJob(j1.ID)
	c.RecordResult(TaskResult{JobID: j1.ID, Status: StatusCompleted})
	c.TriggerJob(j2.ID)
	c.RecordResult(TaskResult{JobID: j2.ID, Status: StatusFailed})

	summary := c.AggregateStatus()
	if summary.Total != 3 {
		t.Fatalf("Expected total 3, got %d", summary.Total)
	}
	if summary.Pending != 1 {
		t.Fatalf("Expected pending 1, got %d", summary.Pending)
	}
	if summary.Completed != 1 {
		t.Fatalf("Expected completed 1, got %d", summary.Completed)
	}
	if summary.Failed != 1 {
		t.Fatalf("Expected failed 1, got %d", summary.Failed)
	}
}

func TestCancelJob(t *testing.T) {
	c := NewCoordinator()
	deviceID := uuid.New()
	job, _ := c.CreateJob(deviceID, "backup", nil, nil)

	if err := c.CancelJob(job.ID); err != nil {
		t.Fatalf("CancelJob failed: %v", err)
	}
	cancelled, _ := c.GetJob(job.ID)
	if cancelled.Status != StatusCancelled {
		t.Fatalf("Expected cancelled, got %s", cancelled.Status)
	}
}

func TestCancelFinishedJob(t *testing.T) {
	c := NewCoordinator()
	deviceID := uuid.New()
	job, _ := c.CreateJob(deviceID, "backup", nil, nil)
	c.TriggerJob(job.ID)
	c.RecordResult(TaskResult{JobID: job.ID, Status: StatusCompleted})

	if err := c.CancelJob(job.ID); err == nil {
		t.Fatal("Should fail to cancel completed job")
	}
}

func TestListJobsByDevice(t *testing.T) {
	c := NewCoordinator()
	d1 := uuid.New()
	d2 := uuid.New()
	c.CreateJob(d1, "j1", nil, nil)
	c.CreateJob(d1, "j2", nil, nil)
	c.CreateJob(d2, "j3", nil, nil)

	jobsD1 := c.ListJobsByDevice(d1)
	if len(jobsD1) != 2 {
		t.Fatalf("Expected 2 jobs for d1, got %d", len(jobsD1))
	}
	jobsD2 := c.ListJobsByDevice(d2)
	if len(jobsD2) != 1 {
		t.Fatalf("Expected 1 job for d2, got %d", len(jobsD2))
	}
}

func TestGetPendingCommandsEmpty(t *testing.T) {
	c := NewCoordinator()
	deviceID := uuid.New()
	pending := c.GetPendingCommands(deviceID)
	if len(pending) != 0 {
		t.Fatalf("Expected 0 pending, got %d", len(pending))
	}
}