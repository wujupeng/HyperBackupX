package compatcert


import (
	"hbx-control/internal/certorch/common"
)

type ChainStage string

const (
	StageBackup   ChainStage = "backup"
	StageRestore  ChainStage = "restore"
	StageVersion  ChainStage = "version"
	StageDelete   ChainStage = "delete"
	StageVerify   ChainStage = "verify"
	StageRecovery ChainStage = "recovery"
)

type Behavior struct {
	Success      bool     `json:"success"`
	OutputHash   string   `json:"output_hash"`
	VersionList  []string `json:"version_list,omitempty"`
	DeleteResult string   `json:"delete_result,omitempty"`
	VerifyResult string   `json:"verify_result,omitempty"`
	ErrorMsg     string   `json:"error_msg,omitempty"`
}

type DiffByDesignRegistry struct {
	entries map[ChainStage]string
}

func NewDiffByDesignRegistry() *DiffByDesignRegistry {
	r := &DiffByDesignRegistry{
		entries: make(map[ChainStage]string),
	}
	r.entries[StageBackup] = "HBX ChunkingProfile Adaptive vs Duplicati fixed chunking"
	return r
}

func (r *DiffByDesignRegistry) Register(stage ChainStage, rationale string) {
	r.entries[stage] = rationale
}

func (r *DiffByDesignRegistry) Lookup(stage ChainStage) (string, bool) {
	v, ok := r.entries[stage]
	return v, ok
}

type FourStateAdjudicator struct {
	diffByDesign *DiffByDesignRegistry
}

func NewFourStateAdjudicator(registry *DiffByDesignRegistry) *FourStateAdjudicator {
	return &FourStateAdjudicator{diffByDesign: registry}
}

func (a *FourStateAdjudicator) Adjudicate(dupBehavior, hbxBehavior Behavior, stage ChainStage) common.CertVerdict4 {
	if rationale, ok := a.diffByDesign.Lookup(stage); ok {
		return common.CertVerdict4{
			Status:          common.V4DifferentByDesign,
			DesignRationale: rationale,
		}
	}

	if !hbxBehavior.Success && hbxBehavior.ErrorMsg != "" && !dupBehavior.Success {
		return common.CertVerdict4{
			Status:             common.V4NotSupported,
			NotSupportedReason: hbxBehavior.ErrorMsg,
		}
	}

	if dupBehavior.Success && !hbxBehavior.Success {
		return common.CertVerdict4{
			Status:    common.V4Fail,
			RootCause: hbxBehavior.ErrorMsg,
		}
	}

	if dupBehavior.OutputHash != hbxBehavior.OutputHash && dupBehavior.OutputHash != "" {
		return common.CertVerdict4{
			Status:    common.V4Fail,
			RootCause: "output hash mismatch",
		}
	}

	return common.CertVerdict4{
		Status: common.V4Pass,
	}
}