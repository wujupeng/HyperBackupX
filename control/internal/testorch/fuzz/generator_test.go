package fuzz

import (
	"reflect"
	"testing"
)

func TestGenerateAllPerturbationTypes(t *testing.T) {
	gen := NewPerturbationGenerator(42)
	perts := gen.Generate(100, nil)

	if len(perts) != 100 {
		t.Fatalf("expected 100 perturbations, got %d", len(perts))
	}

	seen := make(map[PerturbationType]bool)
	for _, p := range perts {
		if !ValidatePerturbationType(p.Type) {
			t.Errorf("invalid perturbation type: %s", p.Type)
		}
		seen[p.Type] = true
	}

	if len(seen) < 5 {
		t.Errorf("expected at least 5 distinct perturbation types, got %d", len(seen))
	}
}

func TestSeedReproducibility(t *testing.T) {
	gen1 := NewPerturbationGenerator(12345)
	gen2 := NewPerturbationGenerator(12345)

	perts1 := gen1.Generate(50, nil)
	perts2 := gen2.Generate(50, nil)

	if !reflect.DeepEqual(perts1, perts2) {
		t.Error("same seed should produce identical perturbation sequences")
	}
}

func TestDifferentSeedsProduceDifferentResults(t *testing.T) {
	gen1 := NewPerturbationGenerator(100)
	gen2 := NewPerturbationGenerator(200)

	perts1 := gen1.Generate(50, nil)
	perts2 := gen2.Generate(50, nil)

	if reflect.DeepEqual(perts1, perts2) {
		t.Error("different seeds should produce different sequences")
	}
}

func TestPerturbationSequence(t *testing.T) {
	gen := NewPerturbationGenerator(42)
	perts := gen.Generate(10, nil)

	for i, p := range perts {
		if p.Sequence != i {
			t.Errorf("perturbation %d has sequence %d", i, p.Sequence)
		}
	}
}

func TestSpecificPerturbationType(t *testing.T) {
	gen := NewPerturbationGenerator(42)
	perts := gen.Generate(20, []PerturbationType{PerturbFileContent})

	for _, p := range perts {
		if p.Type != PerturbFileContent {
			t.Errorf("expected file_content, got %s", p.Type)
		}
		if _, ok := p.Parameters["offset"]; !ok {
			t.Error("file_content perturbation should have offset parameter")
		}
		if _, ok := p.Parameters["length"]; !ok {
			t.Error("file_content perturbation should have length parameter")
		}
	}
}

func TestPerturbationParameters(t *testing.T) {
	gen := NewPerturbationGenerator(42)

	tests := []struct {
		pt       PerturbationType
		paramKey string
	}{
		{PerturbFileContent, "offset"},
		{PerturbDirectory, "action"},
		{PerturbPermission, "mode"},
		{PerturbFilename, "new_name"},
		{PerturbFileSize, "new_size"},
		{PerturbFileDelete, "recursive"},
		{PerturbFileModify, "positions"},
		{PerturbNetworkBreak, "duration_ms"},
		{PerturbProcessKill, "signal"},
		{PerturbDiskFull, "fill_percent"},
	}

	for _, tt := range tests {
		perts := gen.Generate(1, []PerturbationType{tt.pt})
		if len(perts) != 1 {
			t.Fatalf("expected 1 perturbation for %s, got %d", tt.pt, len(perts))
		}
		if _, ok := perts[0].Parameters[tt.paramKey]; !ok {
			t.Errorf("perturbation %s should have parameter %s", tt.pt, tt.paramKey)
		}
	}
}

func TestAllPerturbationTypesCount(t *testing.T) {
	types := AllPerturbationTypes()
	if len(types) != 10 {
		t.Errorf("expected 10 perturbation types, got %d", len(types))
	}
}

func TestValidatePerturbationType(t *testing.T) {
	valid := ValidatePerturbationType(PerturbFileContent)
	if !valid {
		t.Error("PerturbFileContent should be valid")
	}

	invalid := ValidatePerturbationType(PerturbationType("unknown"))
	if invalid {
		t.Error("unknown type should be invalid")
	}
}

func TestReset(t *testing.T) {
	gen := NewPerturbationGenerator(42)
	first := gen.Generate(10, nil)

	gen.Reset()
	second := gen.Generate(10, nil)

	if !reflect.DeepEqual(first, second) {
		t.Error("after reset, same seed should produce same sequence")
	}
}

func TestMarshalUnmarshalPerturbations(t *testing.T) {
	gen := NewPerturbationGenerator(42)
	original := gen.Generate(5, nil)

	data, err := gen.MarshalPerturbations(original)
	if err != nil {
		t.Fatalf("marshal failed: %v", err)
	}

	restored, err := UnmarshalPerturbations(data)
	if err != nil {
		t.Fatalf("unmarshal failed: %v", err)
	}

	if len(original) != len(restored) {
		t.Fatalf("expected %d perturbations, got %d", len(original), len(restored))
	}
	for i, orig := range original {
		if orig.Type != restored[i].Type {
			t.Errorf("perturbation %d: type mismatch", i)
		}
		if orig.Sequence != restored[i].Sequence {
			t.Errorf("perturbation %d: sequence mismatch", i)
		}
	}
}

func TestGenerateScenario(t *testing.T) {
	gen := NewPerturbationGenerator(0)
	config := ScenarioConfig{
		Name:       "test-scenario",
		Seed:       42,
		Iterations: 10,
	}

	perts, err := gen.GenerateScenario(config)
	if err != nil {
		t.Fatalf("GenerateScenario failed: %v", err)
	}
	if len(perts) != 10 {
		t.Errorf("expected 10 perturbations, got %d", len(perts))
	}
}

func TestGenerateScenarioZeroIterations(t *testing.T) {
	gen := NewPerturbationGenerator(0)
	config := ScenarioConfig{
		Name:       "test-scenario",
		Seed:       42,
		Iterations: 0,
	}

	_, err := gen.GenerateScenario(config)
	if err == nil {
		t.Error("should error on zero iterations")
	}
}

func TestEnvironmentControllerState(t *testing.T) {
	ec := NewEnvironmentController()
	if ec.State() != EnvStateReady {
		t.Errorf("initial state should be ready, got %s", ec.State())
	}
}

func TestEnvironmentControllerCrashRestart(t *testing.T) {
	ec := NewEnvironmentController()

	err := ec.InjectCrash(CrashConfig{Type: CrashNetworkBreak, DurationMs: 5000})
	if err != nil {
		t.Fatalf("inject crash failed: %v", err)
	}
	if ec.State() != EnvStateCrashed {
		t.Errorf("state should be crashed, got %s", ec.State())
	}

	err = ec.Restart()
	if err != nil {
		t.Fatalf("restart failed: %v", err)
	}
	if ec.State() != EnvStateRestarted {
		t.Errorf("state should be restarted, got %s", ec.State())
	}

	err = ec.Resume()
	if err != nil {
		t.Fatalf("resume failed: %v", err)
	}
	if ec.State() != EnvStateResumed {
		t.Errorf("state should be resumed, got %s", ec.State())
	}

	err = ec.Cleanup()
	if err != nil {
		t.Fatalf("cleanup failed: %v", err)
	}
	if ec.State() != EnvStateCleaned {
		t.Errorf("state should be cleaned, got %s", ec.State())
	}
}

func TestEnvironmentControllerInvalidTransition(t *testing.T) {
	ec := NewEnvironmentController()

	err := ec.Restart()
	if err == nil {
		t.Error("should not restart from ready state")
	}

	err = ec.Resume()
	if err == nil {
		t.Error("should not resume from ready state")
	}
}

func TestEnvironmentControllerFullCycle(t *testing.T) {
	ec := NewEnvironmentController()

	err := ec.SimulateFullCycle(CrashConfig{Type: CrashProcessKill, DurationMs: 1000})
	if err != nil {
		t.Fatalf("full cycle failed: %v", err)
	}

	events := ec.Events()
	if len(events) != 4 {
		t.Errorf("expected 4 events, got %d", len(events))
	}
}

func TestEnvironmentControllerReset(t *testing.T) {
	ec := NewEnvironmentController()
	ec.InjectCrash(CrashConfig{Type: CrashDiskFull, DurationMs: 1000})
	ec.Reset()

	if ec.State() != EnvStateReady {
		t.Errorf("after reset state should be ready, got %s", ec.State())
	}
	if len(ec.Events()) != 0 {
		t.Error("after reset events should be empty")
	}
}

func TestEnvironmentControllerAllCrashTypes(t *testing.T) {
	crashTypes := []CrashType{CrashNetworkBreak, CrashProcessKill, CrashDiskFull}

	for _, ct := range crashTypes {
		ec := NewEnvironmentController()
		err := ec.InjectCrash(CrashConfig{Type: ct, DurationMs: 1000})
		if err != nil {
			t.Errorf("crash type %s failed: %v", ct, err)
		}
		if ec.State() != EnvStateCrashed {
			t.Errorf("crash type %s should set state to crashed", ct)
		}
	}
}

func TestEnvironmentControllerUnknownCrashType(t *testing.T) {
	ec := NewEnvironmentController()
	err := ec.InjectCrash(CrashConfig{Type: CrashType("unknown")})
	if err == nil {
		t.Error("should error on unknown crash type")
	}
}