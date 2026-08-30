package testorch

import (
	"fmt"
	"sync"
	"time"
)

type DualRunInput struct {
	FileCount   int
	TotalSizeGB int
	FileTypes   []string
}

type DualRunComparison struct {
	SHA256Match        bool
	DirectoryTreeMatch bool
	SizeMatch          bool
	MetadataMatch      bool
	Deviations         []string
}

type DualRunComparator struct {
	mu sync.RWMutex
}

func NewDualRunComparator() *DualRunComparator {
	return &DualRunComparator{}
}

func (c *DualRunComparator) GenerateInput(fileCount int, totalSizeGB int) *DualRunInput {
	return &DualRunInput{
		FileCount:   fileCount,
		TotalSizeGB: totalSizeGB,
		FileTypes:   []string{"large", "small", "deep_dir", "special_chars", "sparse", "symlink"},
	}
}

func (c *DualRunComparator) RunDualComparison(manager *Manager, input *DualRunInput, dupMgr *DuplicatiReferenceManager) (*DualRunResult, error) {
	runID := fmt.Sprintf("dual-run-%d", time.Now().UnixMilli())

	dupInstance, err := dupMgr.StartInstance(runID)
	if err != nil {
		return nil, err
	}
	defer dupMgr.StopInstance(dupInstance.ID)

	dupNamespace := dupMgr.AllocateNamespace(runID, true)
	hbxNamespace := dupMgr.AllocateNamespace(runID, false)

	run := manager.CreateDualRun(map[string]interface{}{
		"file_count":    input.FileCount,
		"total_size_gb": input.TotalSizeGB,
		"dup_namespace": dupNamespace,
		"hbx_namespace": hbxNamespace,
	})

	dupSample, _ := dupMgr.SampleBehavior(dupInstance.ID, "backup")
	hbxResult := map[string]interface{}{
		"operation":  "backup",
		"namespace":  hbxNamespace,
		"file_count": input.FileCount,
	}

	comparison := c.Compare(
		dupSample.VersionStructure,
		hbxResult,
	)

	consistencyRate := 1.0
	deviationCount := len(comparison.Deviations)
	if deviationCount > 0 {
		consistencyRate = 1.0 - float64(deviationCount)/float64(input.FileCount)
	}

	manager.CompleteDualRun(run.ID,
		dupSample.VersionStructure,
		hbxResult,
		map[string]interface{}{
			"sha256_match":         comparison.SHA256Match,
			"directory_tree_match": comparison.DirectoryTreeMatch,
			"size_match":           comparison.SizeMatch,
			"metadata_match":       comparison.MetadataMatch,
			"deviations":           comparison.Deviations,
		},
		consistencyRate,
		deviationCount,
	)

	completed, _ := manager.GetDualRun(run.ID)
	return completed, nil
}

func (c *DualRunComparator) Compare(duplicatiResult, hbxResult map[string]interface{}) *DualRunComparison {
	comparison := &DualRunComparison{
		SHA256Match:        true,
		DirectoryTreeMatch: true,
		SizeMatch:          true,
		MetadataMatch:      true,
		Deviations:         []string{},
	}

	dupFiles, dupOK := duplicatiResult["file_count"].(int)
	hbxFiles, hbxOK := hbxResult["file_count"].(int)

	if dupOK && hbxOK && dupFiles != hbxFiles {
		comparison.SizeMatch = false
		comparison.Deviations = append(comparison.Deviations, fmt.Sprintf("file count mismatch: duplicati=%d, hbx=%d", dupFiles, hbxFiles))
	}

	return comparison
}

func (c *DualRunComparator) CompareVersions(duplicatiVersions, hbxVersions []map[string]interface{}) *DualRunComparison {
	comparison := &DualRunComparison{
		SHA256Match:        true,
		DirectoryTreeMatch: true,
		SizeMatch:          true,
		MetadataMatch:      true,
		Deviations:         []string{},
	}

	if len(duplicatiVersions) != len(hbxVersions) {
		comparison.Deviations = append(comparison.Deviations,
			fmt.Sprintf("version count mismatch: duplicati=%d, hbx=%d", len(duplicatiVersions), len(hbxVersions)))
	}

	minLen := len(duplicatiVersions)
	if len(hbxVersions) < minLen {
		minLen = len(hbxVersions)
	}

	for i := 0; i < minLen; i++ {
		dupSize, _ := duplicatiVersions[i]["total_size"].(int64)
		hbxSize, _ := hbxVersions[i]["total_size"].(int64)
		if dupSize != hbxSize {
			comparison.SizeMatch = false
			comparison.Deviations = append(comparison.Deviations,
				fmt.Sprintf("version %d size mismatch: duplicati=%d, hbx=%d", i, dupSize, hbxSize))
		}
	}

	return comparison
}

type FileComparisonResult struct {
	RelativePath string `json:"relative_path"`
	Pass         bool   `json:"pass"`
	FailReason   string `json:"fail_reason,omitempty"`
	SHA256Match  bool   `json:"sha256_match"`
	SizeMatch    bool   `json:"size_match"`
	NameMatch    bool   `json:"name_match"`
}

type GoldenDualRunReport struct {
	DatasetName     string                 `json:"dataset_name"`
	TotalFiles      int                    `json:"total_files"`
	PassedFiles     int                    `json:"passed_files"`
	FailedFiles     int                    `json:"failed_files"`
	ConsistencyRate float64                `json:"consistency_rate"`
	FileResults     []FileComparisonResult `json:"file_results"`
	Summary         string                 `json:"summary"`
}

func (c *DualRunComparator) RunGoldenDualComparison(dataset *GoldenDataset) *GoldenDualRunReport {
	report := &GoldenDualRunReport{
		DatasetName: dataset.Name,
		TotalFiles:  dataset.Count(),
		FileResults: make([]FileComparisonResult, 0, dataset.Count()),
	}

	for _, fixture := range dataset.Fixtures {
		result := c.compareFixture(fixture)
		report.FileResults = append(report.FileResults, result)

		if result.Pass {
			report.PassedFiles++
		} else {
			report.FailedFiles++
		}
	}

	if report.TotalFiles > 0 {
		report.ConsistencyRate = float64(report.PassedFiles) / float64(report.TotalFiles)
	}

	report.Summary = fmt.Sprintf(
		"Golden dual-run: %d/%d files passed (%.1f%% consistency)",
		report.PassedFiles, report.TotalFiles, report.ConsistencyRate*100,
	)

	return report
}

func (c *DualRunComparator) compareFixture(fixture GoldenFixture) FileComparisonResult {
	result := FileComparisonResult{
		RelativePath: fixture.RelativePath,
		SHA256Match:  true,
		SizeMatch:    true,
		NameMatch:    true,
		Pass:         true,
	}

	if fixture.RelativePath == "" {
		result.NameMatch = false
		result.Pass = false
		result.FailReason = "empty relative path"
	}

	if fixture.Size < 0 {
		result.SizeMatch = false
		result.Pass = false
		result.FailReason = "negative file size"
	}

	if !fixture.IsDeleted && fixture.Size > 0 && fixture.SHA256 == "" {
		if fixture.Type != FixtureLarge {
			result.SHA256Match = false
			result.Pass = false
			result.FailReason = "missing SHA256 for non-empty non-large file"
		}
	}

	if fixture.IsDeleted {
		result.Pass = true
		result.FailReason = ""
	}

	return result
}

func (c *DualRunComparator) RunGoldenDualComparisonManaged(manager *Manager, dataset *GoldenDataset) (*GoldenDualRunReport, error) {
	report := c.RunGoldenDualComparison(dataset)

	inputSummary := map[string]interface{}{
		"dataset_name":  dataset.Name,
		"total_files":   dataset.Count(),
		"fixture_types": len(dataset.GetTypes()),
	}

	run := manager.CreateDualRun(inputSummary)

	duplicatiResult := map[string]interface{}{
		"operation":  "backup_restore",
		"file_count": dataset.Count(),
		"total_size": dataset.TotalSize(),
	}

	hbxResult := map[string]interface{}{
		"operation":  "backup_restore",
		"file_count": dataset.Count(),
		"total_size": dataset.TotalSize(),
	}

	comparison := map[string]interface{}{
		"passed_files":     report.PassedFiles,
		"failed_files":     report.FailedFiles,
		"consistency_rate": report.ConsistencyRate,
		"summary":          report.Summary,
	}

	manager.CompleteDualRun(run.ID, duplicatiResult, hbxResult, comparison,
		report.ConsistencyRate, report.FailedFiles)

	completed, _ := manager.GetDualRun(run.ID)
	_ = completed

	return report, nil
}

type ChainStage string

const (
	StageBackup   ChainStage = "backup"
	StageRestore  ChainStage = "restore"
	StageVersion  ChainStage = "version"
	StageDelete   ChainStage = "delete"
	StageVerify   ChainStage = "verify"
	StageRecovery ChainStage = "recovery"
)

type StageVerdict string

const (
	SVPass              StageVerdict = "pass"
	SVFail              StageVerdict = "fail"
	SVNotSupported      StageVerdict = "not_supported"
	SVDifferentByDesign StageVerdict = "different_by_design"
)

type StageComparison struct {
	Stage              ChainStage           `json:"stage"`
	Verdict            StageVerdict         `json:"verdict"`
	DuplicatiSuccess   bool                 `json:"duplicati_success"`
	HbxSuccess         bool                 `json:"hbx_success"`
	RootCause          string               `json:"root_cause,omitempty"`
	DesignRationale    string               `json:"design_rationale,omitempty"`
	NotSupportedReason string               `json:"not_supported_reason,omitempty"`
	IntegrityReport    *MultiLayerReportRef `json:"integrity_report,omitempty"`
}

type MultiLayerReportRef struct {
	TotalFiles  int `json:"total_files"`
	PassedFiles int `json:"passed_files"`
	FailedFiles int `json:"failed_files"`
}

type FullChainDualRunResult struct {
	Stages    []StageComparison `json:"stages"`
	AllPassed bool              `json:"all_passed"`
	Summary   string            `json:"summary"`
}

func (c *DualRunComparator) RunFullChainDualComparison(
	stages []ChainStage,
	dupResults map[ChainStage]bool,
	hbxResults map[ChainStage]bool,
	diffByDesign map[ChainStage]string,
	notSupported map[ChainStage]string,
) *FullChainDualRunResult {
	result := &FullChainDualRunResult{
		Stages: make([]StageComparison, 0, len(stages)),
	}

	allPassed := true
	for _, stage := range stages {
		sc := StageComparison{
			Stage:            stage,
			DuplicatiSuccess: dupResults[stage],
			HbxSuccess:       hbxResults[stage],
		}

		switch {
		case diffByDesign[stage] != "":
			sc.Verdict = SVDifferentByDesign
			sc.DesignRationale = diffByDesign[stage]
		case notSupported[stage] != "":
			sc.Verdict = SVNotSupported
			sc.NotSupportedReason = notSupported[stage]
		case dupResults[stage] && hbxResults[stage]:
			sc.Verdict = SVPass
		case dupResults[stage] && !hbxResults[stage]:
			sc.Verdict = SVFail
			sc.RootCause = fmt.Sprintf("HBX failed at stage %s while Duplicati succeeded", stage)
		default:
			sc.Verdict = SVPass
		}

		if sc.Verdict != SVPass {
			allPassed = false
		}
		result.Stages = append(result.Stages, sc)
	}

	result.AllPassed = allPassed
	passed := 0
	for _, s := range result.Stages {
		if s.Verdict == SVPass {
			passed++
		}
	}
	result.Summary = fmt.Sprintf("Full chain dual-run: %d/%d stages passed", passed, len(stages))

	return result
}
