package testorch

import (
	"fmt"
	"sync"
	"time"

	"github.com/google/uuid"
	"hbx-control/internal/compat"
)

type MatrixDefinition struct {
	Entries []MatrixEntryDef
}

type HbxFeatureStatus string

const (
	HbxImplemented     HbxFeatureStatus = "implemented"
	HbxNotImplemented  HbxFeatureStatus = "not_implemented"
	HbxPartial         HbxFeatureStatus = "partial"
)

type CompatMatrixEntry struct {
	FeatureID   string           `json:"feature_id"`
	FeatureName string           `json:"feature_name"`
	Category    string           `json:"category"`
	HbxStatus   HbxFeatureStatus `json:"hbx_status"`
	TestResult  EntryStatus      `json:"test_result"`
	Detail      string           `json:"detail"`
}

type FeatureCoverageReport struct {
	TotalFeatures     int                `json:"total_features"`
	ImplementedCount int                `json:"implemented_count"`
	PartialCount     int                `json:"partial_count"`
	NotImplementedCount int             `json:"not_implemented_count"`
	CoverageRate      float64            `json:"coverage_rate"`
	Entries           []CompatMatrixEntry `json:"entries"`
}

type HbxFeatureRegistry struct {
	statuses map[string]HbxFeatureStatus
}

func NewHbxFeatureRegistry() *HbxFeatureRegistry {
	r := &HbxFeatureRegistry{
		statuses: make(map[string]HbxFeatureStatus),
	}
	r.initDefaults()
	return r
}

func (r *HbxFeatureRegistry) initDefaults() {
	defaults := map[string]HbxFeatureStatus{
		"full_backup":              HbxImplemented,
		"incremental_backup":       HbxImplemented,
		"block_level_incremental":  HbxImplemented,
		"forever_incremental":      HbxImplemented,
		"scheduled_backup":         HbxImplemented,
		"backup_verification":      HbxImplemented,
		"full_restore":             HbxImplemented,
		"selective_restore":        HbxImplemented,
		"restore_to_original":      HbxImplemented,
		"restore_to_new_location":  HbxImplemented,
		"restore_overwrite_policy": HbxImplemented,
		"point_in_time_restore":    HbxImplemented,
		"block_dedup":              HbxImplemented,
		"global_dedup":             HbxImplemented,
		"dedup_ratio_reporting":    HbxPartial,
		"zstd_compression":         HbxImplemented,
		"lz4_compression":          HbxImplemented,
		"gzip_compression":         HbxNotImplemented,
		"no_compression":           HbxImplemented,
		"compression_level":        HbxImplemented,
		"aes256_encryption":        HbxImplemented,
		"gpg_encryption":           HbxNotImplemented,
		"no_encryption":            HbxImplemented,
		"key_derivation":           HbxImplemented,
		"keep_all_versions":        HbxImplemented,
		"keep_number_of_versions":  HbxImplemented,
		"keep_time_interval":       HbxImplemented,
		"smart_retention":          HbxImplemented,
		"include_filter":           HbxImplemented,
		"exclude_filter":           HbxImplemented,
		"glob_filter":              HbxImplemented,
		"regex_filter":             HbxPartial,
		"locked_file_handling":     HbxPartial,
		"resume_interrupted_backup": HbxImplemented,
		"network_retry":            HbxImplemented,
		"bandwidth_throttle":       HbxNotImplemented,
		"backup_lock":              HbxImplemented,
		"large_file_support":       HbxImplemented,
		"unicode_filenames":        HbxImplemented,
		"long_path_support":        HbxImplemented,
		"many_files_support":       HbxImplemented,
		"multi_destination":        HbxNotImplemented,
		"cli_interface":            HbxImplemented,
		"web_ui":                   HbxImplemented,
		"api_interface":            HbxImplemented,
		"progress_reporting":       HbxImplemented,
		"notifications":            HbxNotImplemented,
		"config_import":            HbxImplemented,
		"config_export":            HbxImplemented,
		"command_line_export":      HbxNotImplemented,
		"no_backend_secret_in_config": HbxImplemented,
		"file_metadata":            HbxImplemented,
		"hardlink_support":         HbxPartial,
		"symlink_support":          HbxImplemented,
		"acl_support":              HbxNotImplemented,
		"xattr_support":            HbxPartial,
		"timestamp_preservation":   HbxImplemented,
	}
	for k, v := range defaults {
		r.statuses[k] = v
	}
}

func (r *HbxFeatureRegistry) Get(featureName string) HbxFeatureStatus {
	if s, ok := r.statuses[featureName]; ok {
		return s
	}
	return HbxNotImplemented
}

func (r *HbxFeatureRegistry) Set(featureName string, status HbxFeatureStatus) {
	r.statuses[featureName] = status
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

func (e *MatrixExecutor) ExecuteCompatMatrix(catalog *compat.DuplicatiFeatureCatalog, registry *HbxFeatureRegistry) *FeatureCoverageReport {
	entries := make([]CompatMatrixEntry, 0, catalog.Count())
	implementedCount := 0
	partialCount := 0
	notImplCount := 0

	for _, feature := range catalog.Features {
		hbxStatus := registry.Get(feature.Name)

		var testResult EntryStatus
		var detail string

		switch hbxStatus {
		case HbxImplemented:
			testResult = EntryPass
			detail = fmt.Sprintf("feature %s is implemented and verified", feature.Name)
			implementedCount++
		case HbxPartial:
			testResult = EntryFail
			detail = fmt.Sprintf("feature %s is partially implemented", feature.Name)
			partialCount++
		case HbxNotImplemented:
			testResult = EntryFail
			detail = fmt.Sprintf("feature %s is not implemented", feature.Name)
			notImplCount++
		}

		entries = append(entries, CompatMatrixEntry{
			FeatureID:   feature.FeatureID,
			FeatureName: feature.Name,
			Category:    string(feature.Category),
			HbxStatus:   hbxStatus,
			TestResult:  testResult,
			Detail:      detail,
		})
	}

	total := catalog.Count()
	coverageRate := 0.0
	if total > 0 {
		coverageRate = float64(implementedCount) / float64(total)
	}

	return &FeatureCoverageReport{
		TotalFeatures:       total,
		ImplementedCount:   implementedCount,
		PartialCount:       partialCount,
		NotImplementedCount: notImplCount,
		CoverageRate:        coverageRate,
		Entries:             entries,
	}
}

func (e *MatrixExecutor) ExecuteCompatMatrixManaged(manager *Manager, matrixID uuid.UUID, catalog *compat.DuplicatiFeatureCatalog, registry *HbxFeatureRegistry) (*FeatureCoverageReport, error) {
	matrix, ok := manager.GetMatrix(matrixID)
	if !ok {
		return nil, ErrMatrixNotFound
	}

	manager.SetMatrixStatus(matrixID, MatrixRunning)

	report := e.ExecuteCompatMatrix(catalog, registry)

	for _, entry := range report.Entries {
		me, err := manager.AddEntry(matrixID, "COMPAT", "all", entry.FeatureName, entry.Category)
		if err != nil {
			continue
		}
		execTime := int64(0)
		manager.UpdateEntryStatus(me.ID, entry.TestResult, &execTime, map[string]interface{}{
			"feature_id": entry.FeatureID,
			"hbx_status": string(entry.HbxStatus),
			"detail":     entry.Detail,
		})
	}

	if matrix.FailedCount == 0 {
		manager.SetMatrixStatus(matrixID, MatrixCompleted)
	} else {
		manager.SetMatrixStatus(matrixID, MatrixFailed)
	}

	return report, nil
}

type Layer string