package chaos

import (
	"reflect"
	"testing"
)

func TestInjectAllFaultTypes(t *testing.T) {
	injector := NewFaultInjector(42)

	for _, ft := range AllFaultTypes() {
		fault, err := injector.Inject(ft, "test-target")
		if err != nil {
			t.Errorf("inject %s failed: %v", ft, err)
		}
		if fault.Config.Type != ft {
			t.Errorf("expected type %s, got %s", ft, fault.Config.Type)
		}
	}
}

func TestFaultTypeCount(t *testing.T) {
	types := AllFaultTypes()
	if len(types) != 5 {
		t.Errorf("expected 5 fault types, got %d", len(types))
	}
}

func TestSeedReproducibility(t *testing.T) {
	inj1 := NewFaultInjector(12345)
	inj2 := NewFaultInjector(12345)

	for _, ft := range AllFaultTypes() {
		f1, _ := inj1.Inject(ft, "target")
		f2, _ := inj2.Inject(ft, "target")

		if !reflect.DeepEqual(f1.Config.Parameters, f2.Config.Parameters) {
			t.Errorf("fault %s: same seed should produce same params", ft)
		}
	}
}

func TestDifferentSeedsProduceDifferentResults(t *testing.T) {
	inj1 := NewFaultInjector(100)
	inj2 := NewFaultInjector(200)

	f1, _ := inj1.Inject(FaultModifyChunk, "target")
	f2, _ := inj2.Inject(FaultModifyChunk, "target")

	if reflect.DeepEqual(f1.Config.Parameters, f2.Config.Parameters) {
		t.Error("different seeds should produce different params")
	}
}

func TestFaultParameters(t *testing.T) {
	injector := NewFaultInjector(42)

	tests := []struct {
		ft       FaultType
		paramKey string
	}{
		{FaultUploadNetworkBreak, "duration_ms"},
		{FaultKillAgent, "signal"},
		{FaultWindowsRestart, "force"},
		{FaultDeleteVolume, "volume_id"},
		{FaultModifyChunk, "chunk_index"},
	}

	for _, tt := range tests {
		fault, err := injector.Inject(tt.ft, "target")
		if err != nil {
			t.Fatalf("inject %s failed: %v", tt.ft, err)
		}
		if _, ok := fault.Config.Parameters[tt.paramKey]; !ok {
			t.Errorf("fault %s should have parameter %s", tt.ft, tt.paramKey)
		}
	}
}

func TestInvalidFaultType(t *testing.T) {
	injector := NewFaultInjector(42)
	_, err := injector.Inject(FaultType("unknown"), "target")
	if err == nil {
		t.Error("should error on unknown fault type")
	}
}

func TestInjectAll(t *testing.T) {
	injector := NewFaultInjector(42)
	faults, err := injector.InjectAll("test-target")
	if err != nil {
		t.Fatalf("InjectAll failed: %v", err)
	}
	if len(faults) != 5 {
		t.Errorf("expected 5 faults, got %d", len(faults))
	}
}

func TestFaultsHistory(t *testing.T) {
	injector := NewFaultInjector(42)
	injector.Inject(FaultKillAgent, "target1")
	injector.Inject(FaultModifyChunk, "target2")

	faults := injector.Faults()
	if len(faults) != 2 {
		t.Errorf("expected 2 faults in history, got %d", len(faults))
	}
}

func TestReset(t *testing.T) {
	injector := NewFaultInjector(42)
	injector.Inject(FaultKillAgent, "target")
	injector.Reset()

	if len(injector.Faults()) != 0 {
		t.Error("after reset, faults should be empty")
	}
}

func TestResetRestoresSeed(t *testing.T) {
	inj1 := NewFaultInjector(42)
	f1, _ := inj1.Inject(FaultModifyChunk, "target")

	inj2 := NewFaultInjector(42)
	inj2.Inject(FaultModifyChunk, "target")
	inj2.Reset()
	f2, _ := inj2.Inject(FaultModifyChunk, "target")

	if !reflect.DeepEqual(f1.Config.Parameters, f2.Config.Parameters) {
		t.Error("after reset, same seed should produce same params")
	}
}

func TestValidateFaultType(t *testing.T) {
	if !ValidateFaultType(FaultKillAgent) {
		t.Error("FaultKillAgent should be valid")
	}
	if ValidateFaultType(FaultType("unknown")) {
		t.Error("unknown type should be invalid")
	}
}

func TestMarshalUnmarshalFaults(t *testing.T) {
	injector := NewFaultInjector(42)
	injector.Inject(FaultKillAgent, "target1")
	injector.Inject(FaultModifyChunk, "target2")

	original := injector.Faults()
	data, err := injector.MarshalFaults(original)
	if err != nil {
		t.Fatalf("marshal failed: %v", err)
	}

	restored, err := UnmarshalFaults(data)
	if err != nil {
		t.Fatalf("unmarshal failed: %v", err)
	}

	if len(restored) != len(original) {
		t.Errorf("expected %d faults, got %d", len(original), len(restored))
	}
}

func TestFaultDetail(t *testing.T) {
	injector := NewFaultInjector(42)

	tests := []struct {
		ft          FaultType
		expectSubstr string
	}{
		{FaultUploadNetworkBreak, "network break"},
		{FaultKillAgent, "agent process killed"},
		{FaultWindowsRestart, "Windows restart"},
		{FaultDeleteVolume, "volume deleted"},
		{FaultModifyChunk, "chunk modified"},
	}

	for _, tt := range tests {
		fault, _ := injector.Inject(tt.ft, "test-target")
		if !contains(fault.Detail, tt.expectSubstr) {
			t.Errorf("fault %s detail should contain %q, got %q", tt.ft, tt.expectSubstr, fault.Detail)
		}
	}
}

func contains(s, substr string) bool {
	return len(s) >= len(substr) && (s == substr || len(substr) == 0 ||
		(s[:len(substr)] == substr) ||
		contains(s[1:], substr))
}