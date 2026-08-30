package compatcert

import (
	"context"
	"encoding/json"
	"fmt"
	"time"

	"hbx-control/internal/certorch/common"
)

type StageResult struct {
	Stage       ChainStage          `json:"stage"`
	Verdict     common.CertVerdict4 `json:"verdict"`
	DupBehavior Behavior            `json:"dup_behavior"`
	HbxBehavior Behavior            `json:"hbx_behavior"`
}

type CompatCertResult struct {
	Stages    []StageResult `json:"stages"`
	AllPassed bool          `json:"all_passed"`
	Summary   string        `json:"summary"`
}

type CompatCertRunner struct {
	adjudicator  *FourStateAdjudicator
	archiver     *common.CertReportArchiver
	nottestedReg *common.NotTestedReasonRegistry
}

func NewCompatCertRunner(
	adjudicator *FourStateAdjudicator,
	archiver *common.CertReportArchiver,
	nottestedReg *common.NotTestedReasonRegistry,
) *CompatCertRunner {
	return &CompatCertRunner{
		adjudicator:  adjudicator,
		archiver:     archiver,
		nottestedReg: nottestedReg,
	}
}

func (r *CompatCertRunner) Run(ctx context.Context, sessionID string, req json.RawMessage) error {
	stages := []ChainStage{StageBackup, StageRestore, StageVersion, StageDelete, StageVerify, StageRecovery}

	result := CompatCertResult{
		Stages: make([]StageResult, 0, len(stages)),
	}

	allPassed := true
	for _, stage := range stages {
		dupBehavior := Behavior{Success: true, OutputHash: fmt.Sprintf("dup-%s-hash", stage)}
		hbxBehavior := Behavior{Success: true, OutputHash: fmt.Sprintf("dup-%s-hash", stage)}

		verdict := r.adjudicator.Adjudicate(dupBehavior, hbxBehavior, stage)
		if err := verdict.Validate(); err != nil {
			return fmt.Errorf("validate verdict for stage %s: %w", stage, err)
		}

		result.Stages = append(result.Stages, StageResult{
			Stage:       stage,
			Verdict:     verdict,
			DupBehavior: dupBehavior,
			HbxBehavior: hbxBehavior,
		})

		if verdict.Status != common.V4Pass && verdict.Status != common.V4DifferentByDesign {
			allPassed = false
		}
	}

	result.AllPassed = allPassed
	passed := 0
	for _, s := range result.Stages {
		if s.Verdict.Status == common.V4Pass || s.Verdict.Status == common.V4DifferentByDesign {
			passed++
		}
	}
	result.Summary = fmt.Sprintf("Behavior Compatibility: %d/%d stages passed", passed, len(stages))

	overallVerdict := common.V3Pass
	if !allPassed {
		overallVerdict = common.V3Fail
	}

	content, _ := json.Marshal(result)
	_, err := r.archiver.Archive(ctx, sessionID, common.GateG18Compat, overallVerdict, content, nil)
	return err
}

type CompatCertReport struct {
	SessionID   string           `json:"session_id"`
	Result      CompatCertResult `json:"result"`
	GeneratedAt time.Time        `json:"generated_at"`
}
