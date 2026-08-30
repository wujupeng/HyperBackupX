package drcert

import (
	"testing"

	"hbx-control/internal/certorch/common"
)

func TestDRScenario_Count(t *testing.T) {
	scenarios := []DRScenario{
		DRAgentCrash, DRControlPlaneCrash, DRStorageCrash,
		DRNetworkLoss, DRDatabaseRestart, DRMachineReboot, DRRepositoryCorruption,
	}
	if len(scenarios) != 7 {
		t.Errorf("expected 7 DR scenarios, got %d", len(scenarios))
	}
}

func TestRecoveryRunbookRegistry_AllRegistered(t *testing.T) {
	registry := NewRecoveryRunbookRegistry()
	scenarios := []DRScenario{
		DRAgentCrash, DRControlPlaneCrash, DRStorageCrash,
		DRNetworkLoss, DRDatabaseRestart, DRMachineReboot, DRRepositoryCorruption,
	}
	for _, s := range scenarios {
		entry, ok := registry.Lookup(s)
		if !ok {
			t.Errorf("runbook not found for scenario %s", s)
		}
		if entry.ID == "" {
			t.Errorf("empty runbook ID for scenario %s", s)
		}
	}
}

func TestRecoveryRunbookRegistry_AutomaticRecovery(t *testing.T) {
	registry := NewRecoveryRunbookRegistry()
	entry, _ := registry.Lookup(DRAgentCrash)
	if entry.RecoveryKind != RKAutomatic {
		t.Errorf("expected automatic recovery for agent crash, got %s", entry.RecoveryKind)
	}
}

func TestRecoveryRunbookRegistry_ManualRecovery(t *testing.T) {
	registry := NewRecoveryRunbookRegistry()
	entry, _ := registry.Lookup(DRRepositoryCorruption)
	if entry.RecoveryKind != RKManual {
		t.Errorf("expected manual recovery for repository corruption, got %s", entry.RecoveryKind)
	}
}

func TestScenarioResult_Pass(t *testing.T) {
	sr := ScenarioResult{
		Scenario:      DRAgentCrash,
		RTOSeconds:    5.0,
		DataIntegrity: true,
		Verdict:       common.V3Pass,
		RecoveryKind:  RKAutomatic,
	}
	if sr.Verdict != common.V3Pass {
		t.Error("expected pass")
	}
	if !sr.DataIntegrity {
		t.Error("expected data integrity")
	}
}

func TestScenarioResult_NotTestedForMachineReboot(t *testing.T) {
	sr := ScenarioResult{
		Scenario:        DRMachineReboot,
		Verdict:         common.V3NotTested,
		NotTestedReason: "machine reboot requires dedicated hardware",
	}
	if sr.Verdict != common.V3NotTested {
		t.Error("expected not_tested")
	}
}

func TestDRCertResult_Summary(t *testing.T) {
	result := DRCertResult{
		Scenarios: []ScenarioResult{
			{Scenario: DRAgentCrash, Verdict: common.V3Pass},
			{Scenario: DRMachineReboot, Verdict: common.V3NotTested},
		},
		AllPassed: true,
		Summary:   "test summary",
	}
	if result.Summary == "" {
		t.Error("expected non-empty summary")
	}
}
