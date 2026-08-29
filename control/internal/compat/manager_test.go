package compat

import (
	"testing"

	"github.com/google/uuid"
)

func TestCreateRepo(t *testing.T) {
	m := NewManager()
	repo := m.CreateRepo("test-repo", "/data/repo", "local", nil)
	if repo.Name != "test-repo" {
		t.Errorf("expected name 'test-repo', got %s", repo.Name)
	}
	if repo.Status != RepoStatusActive {
		t.Errorf("expected status active, got %s", repo.Status)
	}
	if repo.FormatVersion != 1 {
		t.Errorf("expected format version 1, got %d", repo.FormatVersion)
	}
}

func TestGetRepo(t *testing.T) {
	m := NewManager()
	repo := m.CreateRepo("test-repo", "/data/repo", "local", nil)
	got, ok := m.GetRepo(repo.ID)
	if !ok {
		t.Fatal("repo not found")
	}
	if got.Name != repo.Name {
		t.Errorf("name mismatch")
	}
	_, ok = m.GetRepo(uuid.New())
	if ok {
		t.Error("expected not found for random UUID")
	}
}

func TestListRepos(t *testing.T) {
	m := NewManager()
	m.CreateRepo("repo1", "/data/1", "local", nil)
	m.CreateRepo("repo2", "/data/2", "s3", nil)
	repos := m.ListRepos()
	if len(repos) != 2 {
		t.Errorf("expected 2 repos, got %d", len(repos))
	}
}

func TestUpdateRepo(t *testing.T) {
	m := NewManager()
	repo := m.CreateRepo("test-repo", "/data/repo", "local", nil)
	updated, ok := m.UpdateRepo(repo.ID, "updated", "/new/path", "s3", nil)
	if !ok {
		t.Fatal("update failed")
	}
	if updated.Name != "updated" || updated.RootPath != "/new/path" || updated.StorageBackend != "s3" {
		t.Errorf("update did not apply correctly")
	}
}

func TestDeleteRepo(t *testing.T) {
	m := NewManager()
	repo := m.CreateRepo("test-repo", "/data/repo", "local", nil)
	if !m.DeleteRepo(repo.ID) {
		t.Error("delete failed")
	}
	if _, ok := m.GetRepo(repo.ID); ok {
		t.Error("repo still exists after delete")
	}
	if m.DeleteRepo(uuid.New()) {
		t.Error("delete should fail for non-existent repo")
	}
}

func TestSetRepoStatus(t *testing.T) {
	m := NewManager()
	repo := m.CreateRepo("test-repo", "/data/repo", "local", nil)
	if !m.SetRepoStatus(repo.ID, RepoStatusDisabled) {
		t.Fatal("set status failed")
	}
	got, _ := m.GetRepo(repo.ID)
	if got.Status != RepoStatusDisabled {
		t.Errorf("expected disabled, got %s", got.Status)
	}
}

func TestCreateJob(t *testing.T) {
	m := NewManager()
	repo := m.CreateRepo("test-repo", "/data/repo", "local", nil)
	job, err := m.CreateJob("test-job", repo.ID, BackupTypeFull, DualRepoCompatibleOnly, nil)
	if err != nil {
		t.Fatalf("create job failed: %v", err)
	}
	if job.Status != JobStatusActive {
		t.Errorf("expected active, got %s", job.Status)
	}
}

func TestCreateJobRepoNotFound(t *testing.T) {
	m := NewManager()
	_, err := m.CreateJob("test-job", uuid.New(), BackupTypeFull, DualRepoCompatibleOnly, nil)
	if err != ErrRepoNotFound {
		t.Errorf("expected ErrRepoNotFound, got %v", err)
	}
}

func TestListJobsByRepo(t *testing.T) {
	m := NewManager()
	repo := m.CreateRepo("test-repo", "/data/repo", "local", nil)
	m.CreateJob("job1", repo.ID, BackupTypeFull, DualRepoCompatibleOnly, nil)
	m.CreateJob("job2", repo.ID, BackupTypeIncremental, DualRepoCompatibleOnly, nil)
	jobs := m.ListJobsByRepo(repo.ID)
	if len(jobs) != 2 {
		t.Errorf("expected 2 jobs, got %d", len(jobs))
	}
}

func TestSetJobStatus(t *testing.T) {
	m := NewManager()
	repo := m.CreateRepo("test-repo", "/data/repo", "local", nil)
	job, _ := m.CreateJob("test-job", repo.ID, BackupTypeFull, DualRepoCompatibleOnly, nil)
	if !m.SetJobStatus(job.ID, JobStatusPaused) {
		t.Fatal("set status failed")
	}
	got, _ := m.GetJob(job.ID)
	if got.Status != JobStatusPaused {
		t.Errorf("expected paused, got %s", got.Status)
	}
}

func TestSetJobDualRepoConfig(t *testing.T) {
	m := NewManager()
	repo := m.CreateRepo("test-repo", "/data/repo", "local", nil)
	job, _ := m.CreateJob("test-job", repo.ID, BackupTypeFull, DualRepoDualWithConsistency, nil)
	cfg, _ := m.CreateDualRepoConfig("dual-cfg", uuid.New(), repo.ID, ConsistencySHA256, false, true)
	if !m.SetJobDualRepoConfig(job.ID, &cfg.ID) {
		t.Fatal("set dual repo config failed")
	}
	got, _ := m.GetJob(job.ID)
	if got.DualRepoConfigID == nil || *got.DualRepoConfigID != cfg.ID {
		t.Error("dual repo config not set correctly")
	}
}

func TestCreateDualRepoConfig(t *testing.T) {
	m := NewManager()
	repo := m.CreateRepo("test-repo", "/data/repo", "local", nil)
	cfg, err := m.CreateDualRepoConfig("dual-cfg", uuid.New(), repo.ID, ConsistencySHA256, true, true)
	if err != nil {
		t.Fatalf("create dual config failed: %v", err)
	}
	if !cfg.AutoRepair || !cfg.AlertOnInconsistency {
		t.Error("flags not set correctly")
	}
}

func TestCreateDualRepoConfigRepoNotFound(t *testing.T) {
	m := NewManager()
	_, err := m.CreateDualRepoConfig("dual-cfg", uuid.New(), uuid.New(), ConsistencySHA256, false, true)
	if err != ErrRepoNotFound {
		t.Errorf("expected ErrRepoNotFound, got %v", err)
	}
}

func TestCreateExecution(t *testing.T) {
	m := NewManager()
	repo := m.CreateRepo("test-repo", "/data/repo", "local", nil)
	job, _ := m.CreateJob("test-job", repo.ID, BackupTypeFull, DualRepoCompatibleOnly, nil)
	exec, err := m.CreateExecution(job.ID)
	if err != nil {
		t.Fatalf("create execution failed: %v", err)
	}
	if exec.State != ExecPending {
		t.Errorf("expected pending, got %s", exec.State)
	}
}

func TestCreateExecutionJobNotFound(t *testing.T) {
	m := NewManager()
	_, err := m.CreateExecution(uuid.New())
	if err != ErrJobNotFound {
		t.Errorf("expected ErrJobNotFound, got %v", err)
	}
}

func TestUpdateExecutionState(t *testing.T) {
	m := NewManager()
	repo := m.CreateRepo("test-repo", "/data/repo", "local", nil)
	job, _ := m.CreateJob("test-job", repo.ID, BackupTypeFull, DualRepoCompatibleOnly, nil)
	exec, _ := m.CreateExecution(job.ID)
	m.UpdateExecutionState(exec.ID, ExecScanning, 0.1, 5, 1024)
	got, _ := m.GetExecution(exec.ID)
	if got.State != ExecScanning || got.Progress != 0.1 {
		t.Errorf("state update failed")
	}
}

func TestUpdateExecutionStateCompletion(t *testing.T) {
	m := NewManager()
	repo := m.CreateRepo("test-repo", "/data/repo", "local", nil)
	job, _ := m.CreateJob("test-job", repo.ID, BackupTypeFull, DualRepoCompatibleOnly, nil)
	exec, _ := m.CreateExecution(job.ID)
	m.UpdateExecutionState(exec.ID, ExecSuccess, 1.0, 100, 1048576)
	got, _ := m.GetExecution(exec.ID)
	if got.CompletedAt == nil {
		t.Error("completed_at should be set for success state")
	}
}

func TestSetExecutionError(t *testing.T) {
	m := NewManager()
	repo := m.CreateRepo("test-repo", "/data/repo", "local", nil)
	job, _ := m.CreateJob("test-job", repo.ID, BackupTypeFull, DualRepoCompatibleOnly, nil)
	exec, _ := m.CreateExecution(job.ID)
	m.SetExecutionError(exec.ID, "disk full")
	got, _ := m.GetExecution(exec.ID)
	if got.State != ExecFailed || got.ErrorMessage == nil || *got.ErrorMessage != "disk full" {
		t.Errorf("error not set correctly")
	}
}

func TestSetExecutionCheckpoint(t *testing.T) {
	m := NewManager()
	repo := m.CreateRepo("test-repo", "/data/repo", "local", nil)
	job, _ := m.CreateJob("test-job", repo.ID, BackupTypeFull, DualRepoCompatibleOnly, nil)
	exec, _ := m.CreateExecution(job.ID)
	checkpoint := map[string]interface{}{"phase": "uploading", "offset": 4096}
	m.SetExecutionCheckpoint(exec.ID, checkpoint)
	got, _ := m.GetExecution(exec.ID)
	if got.CheckpointData["phase"] != "uploading" {
		t.Error("checkpoint not set correctly")
	}
}

func TestRecordImportIdempotent(t *testing.T) {
	m := NewManager()
	hash := "abc123"
	imp1 := m.RecordImport(hash, SourceFormatJSON, map[string]interface{}{"key": "val"}, nil, nil, nil, ImportSuccess)
	imp2 := m.RecordImport(hash, SourceFormatJSON, map[string]interface{}{"key": "val"}, nil, nil, nil, ImportSuccess)
	if imp1.ID != imp2.ID {
		t.Error("import should be idempotent for same hash")
	}
}

func TestGetImportByHash(t *testing.T) {
	m := NewManager()
	hash := "abc123"
	m.RecordImport(hash, SourceFormatJSON, nil, nil, nil, nil, ImportSuccess)
	imp, ok := m.GetImportByHash(hash)
	if !ok {
		t.Fatal("import not found by hash")
	}
	if imp.SourceConfigHash != hash {
		t.Error("hash mismatch")
	}
}

func TestListImports(t *testing.T) {
	m := NewManager()
	m.RecordImport("hash1", SourceFormatJSON, nil, nil, nil, nil, ImportSuccess)
	m.RecordImport("hash2", SourceFormatSQLite, nil, nil, nil, nil, ImportPartial)
	imports := m.ListImports()
	if len(imports) != 2 {
		t.Errorf("expected 2 imports, got %d", len(imports))
	}
}

func TestRecordMetric(t *testing.T) {
	m := NewManager()
	m.RecordMetric("success_rate", 0.95, map[string]interface{}{"repo": "test"})
	metrics := m.ListMetrics("success_rate")
	if len(metrics) != 1 {
		t.Errorf("expected 1 metric, got %d", len(metrics))
	}
}

func TestGetSuccessRate(t *testing.T) {
	m := NewManager()
	if rate := m.GetSuccessRate(); rate != 1.0 {
		t.Errorf("expected 1.0 for no executions, got %f", rate)
	}
	repo := m.CreateRepo("test-repo", "/data/repo", "local", nil)
	job, _ := m.CreateJob("test-job", repo.ID, BackupTypeFull, DualRepoCompatibleOnly, nil)
	exec1, _ := m.CreateExecution(job.ID)
	exec2, _ := m.CreateExecution(job.ID)
	m.UpdateExecutionState(exec1.ID, ExecSuccess, 1.0, 0, 0)
	m.UpdateExecutionState(exec2.ID, ExecFailed, 0.5, 0, 0)
	if rate := m.GetSuccessRate(); rate != 0.5 {
		t.Errorf("expected 0.5, got %f", rate)
	}
}

func TestDeleteJob(t *testing.T) {
	m := NewManager()
	repo := m.CreateRepo("test-repo", "/data/repo", "local", nil)
	job, _ := m.CreateJob("test-job", repo.ID, BackupTypeFull, DualRepoCompatibleOnly, nil)
	if !m.DeleteJob(job.ID) {
		t.Error("delete failed")
	}
	if _, ok := m.GetJob(job.ID); ok {
		t.Error("job still exists")
	}
}

func TestDeleteDualRepoConfig(t *testing.T) {
	m := NewManager()
	repo := m.CreateRepo("test-repo", "/data/repo", "local", nil)
	cfg, _ := m.CreateDualRepoConfig("dual-cfg", uuid.New(), repo.ID, ConsistencySHA256, false, true)
	if !m.DeleteDualRepoConfig(cfg.ID) {
		t.Error("delete failed")
	}
	if _, ok := m.GetDualRepoConfig(cfg.ID); ok {
		t.Error("config still exists")
	}
}

func TestListExecutionsByJob(t *testing.T) {
	m := NewManager()
	repo := m.CreateRepo("test-repo", "/data/repo", "local", nil)
	job, _ := m.CreateJob("test-job", repo.ID, BackupTypeFull, DualRepoCompatibleOnly, nil)
	m.CreateExecution(job.ID)
	m.CreateExecution(job.ID)
	executions := m.ListExecutionsByJob(job.ID)
	if len(executions) != 2 {
		t.Errorf("expected 2 executions, got %d", len(executions))
	}
}