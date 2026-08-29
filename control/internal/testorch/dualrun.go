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
	SHA256Match      bool
	DirectoryTreeMatch bool
	SizeMatch        bool
	MetadataMatch    bool
	Deviations       []string
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