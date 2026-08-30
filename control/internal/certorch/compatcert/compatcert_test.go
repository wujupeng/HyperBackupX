package compatcert

import (
	"testing"

	"hbx-control/internal/certorch/common"
)

func TestFourStateAdjudicator_Pass(t *testing.T) {
	registry := NewDiffByDesignRegistry()
	adj := NewFourStateAdjudicator(registry)

	dup := Behavior{Success: true, OutputHash: "abc123"}
	hbx := Behavior{Success: true, OutputHash: "abc123"}

	verdict := adj.Adjudicate(dup, hbx, StageRestore)
	if verdict.Status != common.V4Pass {
		t.Errorf("expected pass, got %s", verdict.Status)
	}
}

func TestFourStateAdjudicator_Fail(t *testing.T) {
	registry := NewDiffByDesignRegistry()
	adj := NewFourStateAdjudicator(registry)

	dup := Behavior{Success: true, OutputHash: "abc123"}
	hbx := Behavior{Success: false, ErrorMsg: "restore failed"}

	verdict := adj.Adjudicate(dup, hbx, StageRestore)
	if verdict.Status != common.V4Fail {
		t.Errorf("expected fail, got %s", verdict.Status)
	}
	if verdict.RootCause == "" {
		t.Error("expected non-empty root cause")
	}
}

func TestFourStateAdjudicator_DifferentByDesign(t *testing.T) {
	registry := NewDiffByDesignRegistry()
	adj := NewFourStateAdjudicator(registry)

	dup := Behavior{Success: true, OutputHash: "abc123"}
	hbx := Behavior{Success: true, OutputHash: "def456"}

	verdict := adj.Adjudicate(dup, hbx, StageBackup)
	if verdict.Status != common.V4DifferentByDesign {
		t.Errorf("expected different_by_design, got %s", verdict.Status)
	}
	if verdict.DesignRationale == "" {
		t.Error("expected non-empty design rationale")
	}
}

func TestFourStateAdjudicator_HashMismatch(t *testing.T) {
	registry := NewDiffByDesignRegistry()
	adj := NewFourStateAdjudicator(registry)

	dup := Behavior{Success: true, OutputHash: "abc123"}
	hbx := Behavior{Success: true, OutputHash: "def456"}

	verdict := adj.Adjudicate(dup, hbx, StageRestore)
	if verdict.Status != common.V4Fail {
		t.Errorf("expected fail for hash mismatch, got %s", verdict.Status)
	}
}

func TestDiffByDesignRegistry_Register(t *testing.T) {
	registry := NewDiffByDesignRegistry()
	registry.Register(StageVerify, "HBX uses BLAKE3 in addition to SHA256")

	rationale, ok := registry.Lookup(StageVerify)
	if !ok {
		t.Error("expected to find registered entry")
	}
	if rationale == "" {
		t.Error("expected non-empty rationale")
	}
}

func TestDiffByDesignRegistry_NotRegistered(t *testing.T) {
	registry := NewDiffByDesignRegistry()
	_, ok := registry.Lookup(StageDelete)
	if ok {
		t.Error("expected not to find unregistered entry")
	}
}

func TestCompatCertResult_Summary(t *testing.T) {
	result := CompatCertResult{
		Stages: []StageResult{
			{Stage: StageBackup, Verdict: common.CertVerdict4{Status: common.V4Pass}},
			{Stage: StageRestore, Verdict: common.CertVerdict4{Status: common.V4Pass}},
		},
		AllPassed: true,
		Summary:   "test summary",
	}
	if result.Summary == "" {
		t.Error("expected non-empty summary")
	}
	if !result.AllPassed {
		t.Error("expected all passed")
	}
}
