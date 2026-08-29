package testorch

import (
	"context"
	"sync"
	"time"

	"github.com/google/uuid"
)

type MatrixStatus string

const (
	MatrixIdle     MatrixStatus = "idle"
	MatrixRunning  MatrixStatus = "running"
	MatrixCompleted MatrixStatus = "completed"
	MatrixFailed   MatrixStatus = "failed"
)

type EntryStatus string

const (
	EntryPending     EntryStatus = "pending"
	EntryPass        EntryStatus = "pass"
	EntryFail        EntryStatus = "fail"
	EntryMissing     EntryStatus = "missing"
	EntryNotApplicable EntryStatus = "not_applicable"
)

type TestCaseStatus string

const (
	CasePending TestCaseStatus = "pending"
	CasePass    TestCaseStatus = "pass"
	CaseFail    TestCaseStatus = "fail"
	CaseSkipped TestCaseStatus = "skipped"
)

type JudgmentCriteria string

const (
	JudgmentSemantic        JudgmentCriteria = "semantic"
	JudgmentSHA256          JudgmentCriteria = "sha256"
	JudgmentDirectoryTree   JudgmentCriteria = "directory_tree"
	JudgmentFileSize        JudgmentCriteria = "file_size"
	JudgmentMetadata        JudgmentCriteria = "metadata"
	JudgmentExceptionDecision JudgmentCriteria = "exception_decision"
)

type DualRunStatus string

const (
	DualRunPending  DualRunStatus = "pending"
	DualRunRunning  DualRunStatus = "running"
	DualRunCompleted DualRunStatus = "completed"
	DualRunFailed   DualRunStatus = "failed"
)

type FaultType string

const (
	FaultNetworkPartition FaultType = "network_partition"
	FaultDiskFull         FaultType = "disk_full"
	FaultPermissionDenied FaultType = "permission_denied"
	FaultFileLock         FaultType = "file_lock"
	FaultSourceDeleted    FaultType = "source_deleted"
	FaultRepoUnavailable  FaultType = "repo_unavailable"
	FaultProcessKill      FaultType = "process_kill"
	FaultPowerLoss        FaultType = "power_loss"
)

type ReportType string

const (
	ReportMatrix     ReportType = "matrix"
	ReportGolden     ReportType = "golden"
	ReportDualRun    ReportType = "dual_run"
	ReportFuzz       ReportType = "fuzz"
	ReportChaos      ReportType = "chaos"
	ReportAcceptance ReportType = "acceptance"
)

type CompatibilityMatrix struct {
	ID           uuid.UUID
	Name         string
	Version      int
	TotalEntries int
	PassedCount  int
	FailedCount  int
	Status       MatrixStatus
	CreatedAt    time.Time
	UpdatedAt    time.Time
}

type MatrixEntry struct {
	ID             uuid.UUID
	MatrixID       uuid.UUID
	Layer          string
	Backend        string
	Feature        string
	Category       string
	Status         EntryStatus
	ErrorMessage   *string
	ExecutionTimeMs *int64
	Evidence       map[string]interface{}
	ExecutedAt     *time.Time
}

type CompatibilityTestCase struct {
	ID              uuid.UUID
	Name            string
	Description     string
	Layer           string
	InputConfig     map[string]interface{}
	ExpectedBehavior map[string]interface{}
	JudgmentCriteria JudgmentCriteria
	Status          TestCaseStatus
	ResultDetail    map[string]interface{}
	MatrixEntryID   *uuid.UUID
	ExecutedAt      *time.Time
}

type DualRunResult struct {
	ID             uuid.UUID
	InputSummary   map[string]interface{}
	DuplicatiResult map[string]interface{}
	HBXResult      map[string]interface{}
	Comparison     map[string]interface{}
	ConsistencyRate float64
	DeviationCount int
	Status         DualRunStatus
	StartedAt      time.Time
	CompletedAt    *time.Time
}

type FuzzScenario struct {
	ID              uuid.UUID
	Name            string
	Description     string
	InputGenerator  string
	Iterations      int
	Seed            *int64
	Status          string
	CorruptionFound bool
	ResultDetail    map[string]interface{}
	StartedAt       *time.Time
	CompletedAt     *time.Time
}

type ChaosScenario struct {
	ID           uuid.UUID
	Name         string
	Description  string
	FaultType    FaultType
	Target       string
	DurationSec  int
	Status       string
	Recovered    bool
	ResultDetail map[string]interface{}
	StartedAt    *time.Time
	CompletedAt  *time.Time
}

type CompatibilityReport struct {
	ID         uuid.UUID
	ReportType ReportType
	MatrixID   *uuid.UUID
	Summary    map[string]interface{}
	Details    map[string]interface{}
	GeneratedAt time.Time
}

type Manager struct {
	mu         sync.RWMutex
	matrices   map[uuid.UUID]*CompatibilityMatrix
	entries    map[uuid.UUID]*MatrixEntry
	testCases  map[uuid.UUID]*CompatibilityTestCase
	dualRuns   map[uuid.UUID]*DualRunResult
	fuzzScenarios map[uuid.UUID]*FuzzScenario
	chaosScenarios map[uuid.UUID]*ChaosScenario
	reports    map[uuid.UUID]*CompatibilityReport
}

func NewManager() *Manager {
	return &Manager{
		matrices:       make(map[uuid.UUID]*CompatibilityMatrix),
		entries:        make(map[uuid.UUID]*MatrixEntry),
		testCases:      make(map[uuid.UUID]*CompatibilityTestCase),
		dualRuns:       make(map[uuid.UUID]*DualRunResult),
		fuzzScenarios:  make(map[uuid.UUID]*FuzzScenario),
		chaosScenarios: make(map[uuid.UUID]*ChaosScenario),
		reports:        make(map[uuid.UUID]*CompatibilityReport),
	}
}

func (m *Manager) CreateMatrix(name string, totalEntries int) *CompatibilityMatrix {
	m.mu.Lock()
	defer m.mu.Unlock()
	now := time.Now().UTC()
	matrix := &CompatibilityMatrix{
		ID:           uuid.New(),
		Name:         name,
		Version:      1,
		TotalEntries: totalEntries,
		Status:       MatrixIdle,
		CreatedAt:    now,
		UpdatedAt:    now,
	}
	m.matrices[matrix.ID] = matrix
	return matrix
}

func (m *Manager) GetMatrix(id uuid.UUID) (*CompatibilityMatrix, bool) {
	m.mu.RLock()
	defer m.mu.RUnlock()
	matrix, ok := m.matrices[id]
	return matrix, ok
}

func (m *Manager) ListMatrices() []*CompatibilityMatrix {
	m.mu.RLock()
	defer m.mu.RUnlock()
	result := make([]*CompatibilityMatrix, 0, len(m.matrices))
	for _, matrix := range m.matrices {
		result = append(result, matrix)
	}
	return result
}

func (m *Manager) SetMatrixStatus(id uuid.UUID, status MatrixStatus) bool {
	m.mu.Lock()
	defer m.mu.Unlock()
	matrix, ok := m.matrices[id]
	if !ok {
		return false
	}
	matrix.Status = status
	matrix.UpdatedAt = time.Now().UTC()
	return true
}

func (m *Manager) AddEntry(matrixID uuid.UUID, layer, backend, feature, category string) (*MatrixEntry, error) {
	m.mu.Lock()
	defer m.mu.Unlock()
	if _, ok := m.matrices[matrixID]; !ok {
		return nil, ErrMatrixNotFound
	}
	entry := &MatrixEntry{
		ID:       uuid.New(),
		MatrixID: matrixID,
		Layer:    layer,
		Backend:  backend,
		Feature:  feature,
		Category: category,
		Status:   EntryPending,
	}
	m.entries[entry.ID] = entry
	return entry, nil
}

func (m *Manager) GetEntry(id uuid.UUID) (*MatrixEntry, bool) {
	m.mu.RLock()
	defer m.mu.RUnlock()
	entry, ok := m.entries[id]
	return entry, ok
}

func (m *Manager) ListEntries(matrixID uuid.UUID) []*MatrixEntry {
	m.mu.RLock()
	defer m.mu.RUnlock()
	var result []*MatrixEntry
	for _, e := range m.entries {
		if e.MatrixID == matrixID {
			result = append(result, e)
		}
	}
	return result
}

func (m *Manager) ListEntriesByLayer(matrixID uuid.UUID, layer string) []*MatrixEntry {
	m.mu.RLock()
	defer m.mu.RUnlock()
	var result []*MatrixEntry
	for _, e := range m.entries {
		if e.MatrixID == matrixID && e.Layer == layer {
			result = append(result, e)
		}
	}
	return result
}

func (m *Manager) UpdateEntryStatus(id uuid.UUID, status EntryStatus, execTimeMs *int64, evidence map[string]interface{}) bool {
	m.mu.Lock()
	defer m.mu.Unlock()
	entry, ok := m.entries[id]
	if !ok {
		return false
	}
	entry.Status = status
	entry.ExecutionTimeMs = execTimeMs
	entry.Evidence = evidence
	now := time.Now().UTC()
	entry.ExecutedAt = &now

	matrix, ok := m.matrices[entry.MatrixID]
	if ok {
		if status == EntryPass {
			matrix.PassedCount++
		} else if status == EntryFail {
			matrix.FailedCount++
		}
	}
	return true
}

func (m *Manager) CreateTestCase(name, layer string, judgment JudgmentCriteria) *CompatibilityTestCase {
	m.mu.Lock()
	defer m.mu.Unlock()
	tc := &CompatibilityTestCase{
		ID:               uuid.New(),
		Name:             name,
		Layer:            layer,
		JudgmentCriteria: judgment,
		Status:           CasePending,
	}
	m.testCases[tc.ID] = tc
	return tc
}

func (m *Manager) GetTestCase(id uuid.UUID) (*CompatibilityTestCase, bool) {
	m.mu.RLock()
	defer m.mu.RUnlock()
	tc, ok := m.testCases[id]
	return tc, ok
}

func (m *Manager) ListTestCases() []*CompatibilityTestCase {
	m.mu.RLock()
	defer m.mu.RUnlock()
	result := make([]*CompatibilityTestCase, 0, len(m.testCases))
	for _, tc := range m.testCases {
		result = append(result, tc)
	}
	return result
}

func (m *Manager) UpdateTestCaseResult(id uuid.UUID, status TestCaseStatus, detail map[string]interface{}) bool {
	m.mu.Lock()
	defer m.mu.Unlock()
	tc, ok := m.testCases[id]
	if !ok {
		return false
	}
	tc.Status = status
	tc.ResultDetail = detail
	now := time.Now().UTC()
	tc.ExecutedAt = &now
	return true
}

func (m *Manager) CreateDualRun(inputSummary map[string]interface{}) *DualRunResult {
	m.mu.Lock()
	defer m.mu.Unlock()
	run := &DualRunResult{
		ID:           uuid.New(),
		InputSummary: inputSummary,
		Status:       DualRunPending,
		StartedAt:    time.Now().UTC(),
	}
	m.dualRuns[run.ID] = run
	return run
}

func (m *Manager) GetDualRun(id uuid.UUID) (*DualRunResult, bool) {
	m.mu.RLock()
	defer m.mu.RUnlock()
	run, ok := m.dualRuns[id]
	return run, ok
}

func (m *Manager) ListDualRuns() []*DualRunResult {
	m.mu.RLock()
	defer m.mu.RUnlock()
	result := make([]*DualRunResult, 0, len(m.dualRuns))
	for _, run := range m.dualRuns {
		result = append(result, run)
	}
	return result
}

func (m *Manager) CompleteDualRun(id uuid.UUID, duplicatiResult, hbxResult, comparison map[string]interface{}, consistencyRate float64, deviationCount int) bool {
	m.mu.Lock()
	defer m.mu.Unlock()
	run, ok := m.dualRuns[id]
	if !ok {
		return false
	}
	run.DuplicatiResult = duplicatiResult
	run.HBXResult = hbxResult
	run.Comparison = comparison
	run.ConsistencyRate = consistencyRate
	run.DeviationCount = deviationCount
	run.Status = DualRunCompleted
	now := time.Now().UTC()
	run.CompletedAt = &now
	return true
}

func (m *Manager) CreateFuzzScenario(name, inputGenerator string, iterations int) *FuzzScenario {
	m.mu.Lock()
	defer m.mu.Unlock()
	scenario := &FuzzScenario{
		ID:             uuid.New(),
		Name:           name,
		InputGenerator: inputGenerator,
		Iterations:     iterations,
		Status:         "pending",
	}
	m.fuzzScenarios[scenario.ID] = scenario
	return scenario
}

func (m *Manager) ListFuzzScenarios() []*FuzzScenario {
	m.mu.RLock()
	defer m.mu.RUnlock()
	result := make([]*FuzzScenario, 0, len(m.fuzzScenarios))
	for _, s := range m.fuzzScenarios {
		result = append(result, s)
	}
	return result
}

func (m *Manager) CreateChaosScenario(name string, faultType FaultType, target string, durationSec int) *ChaosScenario {
	m.mu.Lock()
	defer m.mu.Unlock()
	scenario := &ChaosScenario{
		ID:          uuid.New(),
		Name:        name,
		FaultType:   faultType,
		Target:      target,
		DurationSec: durationSec,
		Status:      "pending",
	}
	m.chaosScenarios[scenario.ID] = scenario
	return scenario
}

func (m *Manager) ListChaosScenarios() []*ChaosScenario {
	m.mu.RLock()
	defer m.mu.RUnlock()
	result := make([]*ChaosScenario, 0, len(m.chaosScenarios))
	for _, s := range m.chaosScenarios {
		result = append(result, s)
	}
	return result
}

func (m *Manager) CreateReport(reportType ReportType, matrixID *uuid.UUID, summary, details map[string]interface{}) *CompatibilityReport {
	m.mu.Lock()
	defer m.mu.Unlock()
	report := &CompatibilityReport{
		ID:          uuid.New(),
		ReportType:  reportType,
		MatrixID:    matrixID,
		Summary:     summary,
		Details:     details,
		GeneratedAt: time.Now().UTC(),
	}
	m.reports[report.ID] = report
	return report
}

func (m *Manager) ListReports() []*CompatibilityReport {
	m.mu.RLock()
	defer m.mu.RUnlock()
	result := make([]*CompatibilityReport, 0, len(m.reports))
	for _, r := range m.reports {
		result = append(result, r)
	}
	return result
}

func (m *Manager) GetMatrixPassRate(id uuid.UUID) float64 {
	m.mu.RLock()
	defer m.mu.RUnlock()
	matrix, ok := m.matrices[id]
	if !ok || matrix.TotalEntries == 0 {
		return 0.0
	}
	return float64(matrix.PassedCount) / float64(matrix.TotalEntries)
}

func (m *Manager) StartOrchestrator(ctx context.Context, interval time.Duration) {
	go func() {
		ticker := time.NewTicker(interval)
		defer ticker.Stop()
		for {
			select {
			case <-ctx.Done():
				return
			case <-ticker.C:
				m.mu.RLock()
				for _, matrix := range m.matrices {
					if matrix.Status == MatrixRunning {
						_ = matrix
					}
				}
				m.mu.RUnlock()
			}
		}
	}()
}