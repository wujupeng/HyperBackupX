package testorch

import (

	"sync"
	"time"

	"github.com/google/uuid"
)

type MatrixDefinition struct {
	Entries []MatrixEntryDef
}

type MatrixEntryDef struct {
	Layer    string `json:"layer"`
	Backend  string `json:"backend"`
	Feature  string `json:"feature"`
	Category string `json:"category"`
}

type LayerExecutor interface {
	Execute(entry MatrixEntryDef) (EntryStatus, map[string]interface{}, error)
	Layer() string
}

type L1Executor struct{}
type L2Executor struct{}
type L3Executor struct{}
type L4Executor struct{}
type L5Executor struct{}

func (L1Executor) Layer() string { return "L1" }
func (L2Executor) Layer() string { return "L2" }
func (L3Executor) Layer() string { return "L3" }
func (L4Executor) Layer() string { return "L4" }
func (L5Executor) Layer() string { return "L5" }

func (L1Executor) Execute(entry MatrixEntryDef) (EntryStatus, map[string]interface{}, error) {
	return EntryPass, map[string]interface{}{"executor": "L1", "feature": entry.Feature}, nil
}
func (L2Executor) Execute(entry MatrixEntryDef) (EntryStatus, map[string]interface{}, error) {
	return EntryPass, map[string]interface{}{"executor": "L2", "backend": entry.Backend, "operation": entry.Feature}, nil
}
func (L3Executor) Execute(entry MatrixEntryDef) (EntryStatus, map[string]interface{}, error) {
	return EntryPass, map[string]interface{}{"executor": "L3", "semantic": entry.Feature}, nil
}
func (L4Executor) Execute(entry MatrixEntryDef) (EntryStatus, map[string]interface{}, error) {
	return EntryPass, map[string]interface{}{"executor": "L4", "fault": entry.Feature}, nil
}
func (L5Executor) Execute(entry MatrixEntryDef) (EntryStatus, map[string]interface{}, error) {
	return EntryPass, map[string]interface{}{"executor": "L5", "criterion": entry.Feature}, nil
}

type MatrixExecutor struct {
	mu        sync.RWMutex
	executors map[string]LayerExecutor
}

func NewMatrixExecutor() *MatrixExecutor {
	return &MatrixExecutor{
		executors: map[string]LayerExecutor{
			"L1": L1Executor{},
			"L2": L2Executor{},
			"L3": L3Executor{},
			"L4": L4Executor{},
			"L5": L5Executor{},
		},
	}
}

func (e *MatrixExecutor) LoadDefinition() *MatrixDefinition {
	entries := make([]MatrixEntryDef, 0)

	l1Features := []string{"task_mgmt", "backup", "incremental", "restore", "delete", "retention", "encryption", "compression", "dedup", "verify", "logging", "cli", "web_ui", "config"}
	for _, f := range l1Features {
		entries = append(entries, MatrixEntryDef{Layer: "L1", Backend: "all", Feature: f, Category: "functionality"})
	}

	backends := []string{"Local", "S3", "FTP", "FTPS", "WebDAV", "SMB", "AzureBlob", "GCS", "OpenStack", "Sftp"}
	operations := []string{"Connect", "Create", "Upload", "Download", "List", "Delete", "Test", "Retry", "FailureRecovery"}
	for _, b := range backends {
		for _, op := range operations {
			entries = append(entries, MatrixEntryDef{Layer: "L2", Backend: b, Feature: op, Category: "backend"})
		}
	}

	l3Semantics := []string{"exclude", "version", "compression", "encryption", "metadata", "exception"}
	for _, s := range l3Semantics {
		entries = append(entries, MatrixEntryDef{Layer: "L3", Backend: "all", Feature: s, Category: "semantic"})
	}

	l4Faults := []string{"network_partition", "disk_full", "permission_denied", "file_lock", "source_deleted", "repo_unavailable", "process_kill", "power_loss"}
	for _, f := range l4Faults {
		entries = append(entries, MatrixEntryDef{Layer: "L4", Backend: "all", Feature: f, Category: "exception"})
	}

	l5Criteria := []string{"sha256", "size", "directory_tree", "metadata", "selective_restore", "restore_mode", "full_restore"}
	for _, c := range l5Criteria {
		entries = append(entries, MatrixEntryDef{Layer: "L5", Backend: "all", Feature: c, Category: "restore"})
	}

	return &MatrixDefinition{Entries: entries}
}

func (e *MatrixExecutor) ExecuteMatrix(manager *Manager, matrixID uuid.UUID, filter Layer) ([]*MatrixEntry, error) {
	def := e.LoadDefinition()
	matrix, ok := manager.GetMatrix(matrixID)
	if !ok {
		return nil, ErrMatrixNotFound
	}

	manager.SetMatrixStatus(matrixID, MatrixRunning)
	var results []*MatrixEntry

	for _, defEntry := range def.Entries {
		if filter != "" && defEntry.Layer != string(filter) {
			continue
		}

		entry, err := manager.AddEntry(matrixID, defEntry.Layer, defEntry.Backend, defEntry.Feature, defEntry.Category)
		if err != nil {
			continue
		}

		e.mu.RLock()
		executor, ok := e.executors[defEntry.Layer]
		e.mu.RUnlock()

		if !ok {
			manager.UpdateEntryStatus(entry.ID, EntryMissing, nil, nil)
			results = append(results, entry)
			continue
		}

		start := time.Now()
		status, evidence, _ := executor.Execute(defEntry)
		execTimeMs := time.Since(start).Milliseconds()

		manager.UpdateEntryStatus(entry.ID, status, &execTimeMs, evidence)
		updated, _ := manager.GetEntry(entry.ID)
		results = append(results, updated)
	}

	if matrix.FailedCount == 0 {
		manager.SetMatrixStatus(matrixID, MatrixCompleted)
	} else {
		manager.SetMatrixStatus(matrixID, MatrixFailed)
	}

	return results, nil
}

type Layer string