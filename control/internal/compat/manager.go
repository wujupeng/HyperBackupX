package compat

import (
	"context"
	"sync"
	"time"

	"github.com/google/uuid"
)

type RepoStatus string

const (
	RepoStatusActive   RepoStatus = "active"
	RepoStatusDisabled RepoStatus = "disabled"
	RepoStatusError    RepoStatus = "error"
)

type JobStatus string

const (
	JobStatusActive   JobStatus = "active"
	JobStatusPaused   JobStatus = "paused"
	JobStatusDisabled JobStatus = "disabled"
)

type BackupType string

const (
	BackupTypeFull        BackupType = "full"
	BackupTypeIncremental BackupType = "incremental"
)

type DualRepoMode string

const (
	DualRepoNativeOnly          DualRepoMode = "native_only"
	DualRepoCompatibleOnly      DualRepoMode = "compatible_only"
	DualRepoDualWithConsistency DualRepoMode = "dual_with_consistency"
)

type ConsistencyMode string

const (
	ConsistencySHA256    ConsistencyMode = "sha256"
	ConsistencySizeOnly  ConsistencyMode = "size_only"
	ConsistencyMetadata  ConsistencyMode = "metadata"
)

type ExecutionState string

const (
	ExecPending        ExecutionState = "pending"
	ExecAligning       ExecutionState = "aligning"
	ExecScanning       ExecutionState = "scanning"
	ExecChunking       ExecutionState = "chunking"
	ExecEncrypting     ExecutionState = "encrypting"
	ExecUploading      ExecutionState = "uploading"
	ExecCommitting     ExecutionState = "comp_committing"
	ExecVerifying      ExecutionState = "verifying"
	ExecSuccess        ExecutionState = "success"
	ExecFailed         ExecutionState = "failed"
	ExecPaused         ExecutionState = "paused"
)

type ImportStatus string

const (
	ImportSuccess ImportStatus = "success"
	ImportPartial ImportStatus = "partial"
	ImportFailed  ImportStatus = "failed"
)

type SourceFormat string

const (
	SourceFormatJSON   SourceFormat = "json"
	SourceFormatSQLite SourceFormat = "sqlite"
	SourceFormatXML    SourceFormat = "xml"
)

type CompatRepository struct {
	ID              uuid.UUID
	Name            string
	RootPath        string
	StorageBackend  string
	BackendConfig   map[string]interface{}
	FormatVersion   int
	DuplicatiSemver string
	Status          RepoStatus
	CreatedAt       time.Time
	UpdatedAt       time.Time
}

type CompatJob struct {
	ID                 uuid.UUID
	Name               string
	RepoID             uuid.UUID
	SourceConfig       map[string]interface{}
	BackupType         BackupType
	ScheduleConfig     map[string]interface{}
	RetentionConfig    map[string]interface{}
	EncryptionConfig   map[string]interface{}
	CompressionConfig  map[string]interface{}
	DualRepoMode       DualRepoMode
	DualRepoConfigID   *uuid.UUID
	Status             JobStatus
	CreatedAt          time.Time
	UpdatedAt          time.Time
}

type DualRepoConfig struct {
	ID                  uuid.UUID
	Name                string
	NativeRepoID        uuid.UUID
	CompatRepoID        uuid.UUID
	ConsistencyMode     ConsistencyMode
	AutoRepair          bool
	AlertOnInconsistency bool
	CreatedAt           time.Time
	UpdatedAt           time.Time
}

type CompatExecution struct {
	ID             uuid.UUID
	JobID          uuid.UUID
	VersionID      *uuid.UUID
	State          ExecutionState
	Progress       float64
	FilesProcessed int64
	BytesProcessed int64
	DurationMs     *int64
	ErrorMessage   *string
	CheckpointData map[string]interface{}
	StartedAt      time.Time
	CompletedAt    *time.Time
}

type ConfigImport struct {
	ID              uuid.UUID
	SourceConfigHash string
	SourceFormat    SourceFormat
	SourceConfig    map[string]interface{}
	ResultingJobID  *uuid.UUID
	FieldMappings   map[string]interface{}
	UnsupportedItems []interface{}
	ImportStatus    ImportStatus
	ImportedAt      time.Time
}

type CompatMetric struct {
	ID      uuid.UUID
	Name    string
	Value   float64
	Labels  map[string]interface{}
	RecordedAt time.Time
}

type Manager struct {
	mu         sync.RWMutex
	repos      map[uuid.UUID]*CompatRepository
	jobs       map[uuid.UUID]*CompatJob
	dualConfigs map[uuid.UUID]*DualRepoConfig
	executions  map[uuid.UUID]*CompatExecution
	imports     map[uuid.UUID]*ConfigImport
	metrics     map[uuid.UUID]*CompatMetric
}

func NewManager() *Manager {
	return &Manager{
		repos:       make(map[uuid.UUID]*CompatRepository),
		jobs:        make(map[uuid.UUID]*CompatJob),
		dualConfigs: make(map[uuid.UUID]*DualRepoConfig),
		executions:  make(map[uuid.UUID]*CompatExecution),
		imports:     make(map[uuid.UUID]*ConfigImport),
		metrics:     make(map[uuid.UUID]*CompatMetric),
	}
}

func (m *Manager) CreateRepo(name, rootPath, storageBackend string, backendConfig map[string]interface{}) *CompatRepository {
	m.mu.Lock()
	defer m.mu.Unlock()

	now := time.Now().UTC()
	repo := &CompatRepository{
		ID:              uuid.New(),
		Name:            name,
		RootPath:        rootPath,
		StorageBackend:  storageBackend,
		BackendConfig:   backendConfig,
		FormatVersion:   1,
		DuplicatiSemver: "2.0-compatible",
		Status:          RepoStatusActive,
		CreatedAt:       now,
		UpdatedAt:       now,
	}
	m.repos[repo.ID] = repo
	return repo
}

func (m *Manager) GetRepo(id uuid.UUID) (*CompatRepository, bool) {
	m.mu.RLock()
	defer m.mu.RUnlock()
	r, ok := m.repos[id]
	return r, ok
}

func (m *Manager) ListRepos() []*CompatRepository {
	m.mu.RLock()
	defer m.mu.RUnlock()
	result := make([]*CompatRepository, 0, len(m.repos))
	for _, r := range m.repos {
		result = append(result, r)
	}
	return result
}

func (m *Manager) UpdateRepo(id uuid.UUID, name, rootPath, storageBackend string, backendConfig map[string]interface{}) (*CompatRepository, bool) {
	m.mu.Lock()
	defer m.mu.Unlock()
	r, ok := m.repos[id]
	if !ok {
		return nil, false
	}
	r.Name = name
	r.RootPath = rootPath
	r.StorageBackend = storageBackend
	r.BackendConfig = backendConfig
	r.UpdatedAt = time.Now().UTC()
	return r, true
}

func (m *Manager) DeleteRepo(id uuid.UUID) bool {
	m.mu.Lock()
	defer m.mu.Unlock()
	if _, ok := m.repos[id]; !ok {
		return false
	}
	delete(m.repos, id)
	return true
}

func (m *Manager) SetRepoStatus(id uuid.UUID, status RepoStatus) bool {
	m.mu.Lock()
	defer m.mu.Unlock()
	r, ok := m.repos[id]
	if !ok {
		return false
	}
	r.Status = status
	r.UpdatedAt = time.Now().UTC()
	return true
}

func (m *Manager) CreateJob(name string, repoID uuid.UUID, backupType BackupType, dualRepoMode DualRepoMode, sourceConfig map[string]interface{}) (*CompatJob, error) {
	m.mu.Lock()
	defer m.mu.Unlock()

	if _, ok := m.repos[repoID]; !ok {
		return nil, ErrRepoNotFound
	}

	now := time.Now().UTC()
	job := &CompatJob{
		ID:            uuid.New(),
		Name:          name,
		RepoID:        repoID,
		SourceConfig:  sourceConfig,
		BackupType:    backupType,
		DualRepoMode:  dualRepoMode,
		Status:        JobStatusActive,
		CreatedAt:     now,
		UpdatedAt:     now,
	}
	m.jobs[job.ID] = job
	return job, nil
}

func (m *Manager) GetJob(id uuid.UUID) (*CompatJob, bool) {
	m.mu.RLock()
	defer m.mu.RUnlock()
	j, ok := m.jobs[id]
	return j, ok
}

func (m *Manager) ListJobs() []*CompatJob {
	m.mu.RLock()
	defer m.mu.RUnlock()
	result := make([]*CompatJob, 0, len(m.jobs))
	for _, j := range m.jobs {
		result = append(result, j)
	}
	return result
}

func (m *Manager) ListJobsByRepo(repoID uuid.UUID) []*CompatJob {
	m.mu.RLock()
	defer m.mu.RUnlock()
	var result []*CompatJob
	for _, j := range m.jobs {
		if j.RepoID == repoID {
			result = append(result, j)
		}
	}
	return result
}

func (m *Manager) UpdateJob(id uuid.UUID, name string, backupType BackupType, dualRepoMode DualRepoMode, sourceConfig map[string]interface{}) (*CompatJob, bool) {
	m.mu.Lock()
	defer m.mu.Unlock()
	j, ok := m.jobs[id]
	if !ok {
		return nil, false
	}
	j.Name = name
	j.BackupType = backupType
	j.DualRepoMode = dualRepoMode
	j.SourceConfig = sourceConfig
	j.UpdatedAt = time.Now().UTC()
	return j, true
}

func (m *Manager) DeleteJob(id uuid.UUID) bool {
	m.mu.Lock()
	defer m.mu.Unlock()
	if _, ok := m.jobs[id]; !ok {
		return false
	}
	delete(m.jobs, id)
	return true
}

func (m *Manager) SetJobStatus(id uuid.UUID, status JobStatus) bool {
	m.mu.Lock()
	defer m.mu.Unlock()
	j, ok := m.jobs[id]
	if !ok {
		return false
	}
	j.Status = status
	j.UpdatedAt = time.Now().UTC()
	return true
}

func (m *Manager) SetJobDualRepoConfig(id uuid.UUID, configID *uuid.UUID) bool {
	m.mu.Lock()
	defer m.mu.Unlock()
	j, ok := m.jobs[id]
	if !ok {
		return false
	}
	j.DualRepoConfigID = configID
	j.UpdatedAt = time.Now().UTC()
	return true
}

func (m *Manager) CreateDualRepoConfig(name string, nativeRepoID, compatRepoID uuid.UUID, mode ConsistencyMode, autoRepair, alertOnInconsistency bool) (*DualRepoConfig, error) {
	m.mu.Lock()
	defer m.mu.Unlock()

	if _, ok := m.repos[compatRepoID]; !ok {
		return nil, ErrRepoNotFound
	}

	now := time.Now().UTC()
	cfg := &DualRepoConfig{
		ID:                  uuid.New(),
		Name:                name,
		NativeRepoID:        nativeRepoID,
		CompatRepoID:        compatRepoID,
		ConsistencyMode:     mode,
		AutoRepair:          autoRepair,
		AlertOnInconsistency: alertOnInconsistency,
		CreatedAt:           now,
		UpdatedAt:           now,
	}
	m.dualConfigs[cfg.ID] = cfg
	return cfg, nil
}

func (m *Manager) GetDualRepoConfig(id uuid.UUID) (*DualRepoConfig, bool) {
	m.mu.RLock()
	defer m.mu.RUnlock()
	c, ok := m.dualConfigs[id]
	return c, ok
}

func (m *Manager) ListDualRepoConfigs() []*DualRepoConfig {
	m.mu.RLock()
	defer m.mu.RUnlock()
	result := make([]*DualRepoConfig, 0, len(m.dualConfigs))
	for _, c := range m.dualConfigs {
		result = append(result, c)
	}
	return result
}

func (m *Manager) DeleteDualRepoConfig(id uuid.UUID) bool {
	m.mu.Lock()
	defer m.mu.Unlock()
	if _, ok := m.dualConfigs[id]; !ok {
		return false
	}
	delete(m.dualConfigs, id)
	return true
}

func (m *Manager) CreateExecution(jobID uuid.UUID) (*CompatExecution, error) {
	m.mu.Lock()
	defer m.mu.Unlock()

	if _, ok := m.jobs[jobID]; !ok {
		return nil, ErrJobNotFound
	}

	exec := &CompatExecution{
		ID:        uuid.New(),
		JobID:     jobID,
		State:     ExecPending,
		Progress:  0.0,
		StartedAt: time.Now().UTC(),
	}
	m.executions[exec.ID] = exec
	return exec, nil
}

func (m *Manager) GetExecution(id uuid.UUID) (*CompatExecution, bool) {
	m.mu.RLock()
	defer m.mu.RUnlock()
	e, ok := m.executions[id]
	return e, ok
}

func (m *Manager) ListExecutionsByJob(jobID uuid.UUID) []*CompatExecution {
	m.mu.RLock()
	defer m.mu.RUnlock()
	var result []*CompatExecution
	for _, e := range m.executions {
		if e.JobID == jobID {
			result = append(result, e)
		}
	}
	return result
}

func (m *Manager) UpdateExecutionState(id uuid.UUID, state ExecutionState, progress float64, filesProcessed, bytesProcessed int64) bool {
	m.mu.Lock()
	defer m.mu.Unlock()
	e, ok := m.executions[id]
	if !ok {
		return false
	}
	e.State = state
	e.Progress = progress
	e.FilesProcessed = filesProcessed
	e.BytesProcessed = bytesProcessed
	if state == ExecSuccess || state == ExecFailed {
		now := time.Now().UTC()
		e.CompletedAt = &now
	}
	return true
}

func (m *Manager) SetExecutionError(id uuid.UUID, errMsg string) bool {
	m.mu.Lock()
	defer m.mu.Unlock()
	e, ok := m.executions[id]
	if !ok {
		return false
	}
	e.State = ExecFailed
	e.ErrorMessage = &errMsg
	now := time.Now().UTC()
	e.CompletedAt = &now
	return true
}

func (m *Manager) SetExecutionCheckpoint(id uuid.UUID, checkpoint map[string]interface{}) bool {
	m.mu.Lock()
	defer m.mu.Unlock()
	e, ok := m.executions[id]
	if !ok {
		return false
	}
	e.CheckpointData = checkpoint
	return true
}

func (m *Manager) RecordImport(sourceConfigHash string, sourceFormat SourceFormat, sourceConfig map[string]interface{}, resultingJobID *uuid.UUID, fieldMappings map[string]interface{}, unsupportedItems []interface{}, status ImportStatus) *ConfigImport {
	m.mu.Lock()
	defer m.mu.Unlock()

	for _, imp := range m.imports {
		if imp.SourceConfigHash == sourceConfigHash {
			return imp
		}
	}

	imp := &ConfigImport{
		ID:               uuid.New(),
		SourceConfigHash: sourceConfigHash,
		SourceFormat:     sourceFormat,
		SourceConfig:     sourceConfig,
		ResultingJobID:   resultingJobID,
		FieldMappings:    fieldMappings,
		UnsupportedItems: unsupportedItems,
		ImportStatus:     status,
		ImportedAt:       time.Now().UTC(),
	}
	m.imports[imp.ID] = imp
	return imp
}

func (m *Manager) GetImport(id uuid.UUID) (*ConfigImport, bool) {
	m.mu.RLock()
	defer m.mu.RUnlock()
	imp, ok := m.imports[id]
	return imp, ok
}

func (m *Manager) GetImportByHash(hash string) (*ConfigImport, bool) {
	m.mu.RLock()
	defer m.mu.RUnlock()
	for _, imp := range m.imports {
		if imp.SourceConfigHash == hash {
			return imp, true
		}
	}
	return nil, false
}

func (m *Manager) ListImports() []*ConfigImport {
	m.mu.RLock()
	defer m.mu.RUnlock()
	result := make([]*ConfigImport, 0, len(m.imports))
	for _, imp := range m.imports {
		result = append(result, imp)
	}
	return result
}

func (m *Manager) RecordMetric(name string, value float64, labels map[string]interface{}) *CompatMetric {
	m.mu.Lock()
	defer m.mu.Unlock()
	metric := &CompatMetric{
		ID:        uuid.New(),
		Name:      name,
		Value:     value,
		Labels:    labels,
		RecordedAt: time.Now().UTC(),
	}
	m.metrics[metric.ID] = metric
	return metric
}

func (m *Manager) ListMetrics(name string) []*CompatMetric {
	m.mu.RLock()
	defer m.mu.RUnlock()
	var result []*CompatMetric
	for _, metric := range m.metrics {
		if name == "" || metric.Name == name {
			result = append(result, metric)
		}
	}
	return result
}

func (m *Manager) GetSuccessRate() float64 {
	m.mu.RLock()
	defer m.mu.RUnlock()
	total := len(m.executions)
	if total == 0 {
		return 1.0
	}
	success := 0
	for _, e := range m.executions {
		if e.State == ExecSuccess {
			success++
		}
	}
	return float64(success) / float64(total)
}

func (m *Manager) StartMetricCollector(ctx context.Context, interval time.Duration) {
	go func() {
		ticker := time.NewTicker(interval)
		defer ticker.Stop()
		for {
			select {
			case <-ctx.Done():
				return
			case <-ticker.C:
				m.RecordMetric("compat_success_rate", m.GetSuccessRate(), nil)
			}
		}
	}()
}