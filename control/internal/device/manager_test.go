package device

import (
	"testing"
	"time"

	"github.com/google/uuid"
)

func TestRegisterAndGet(t *testing.T) {
	m := NewManager()
	d, err := m.Register("host1", "windows", "0.1.0", "standard", "fp-001")
	if err != nil {
		t.Fatalf("Register failed: %v", err)
	}
	if d.Hostname != "host1" {
		t.Fatalf("Expected hostname host1, got %s", d.Hostname)
	}
	if d.Status != StatusOnline {
		t.Fatalf("Expected online, got %s", d.Status)
	}

	got, ok := m.Get(d.ID)
	if !ok {
		t.Fatal("Device not found")
	}
	if got.ID != d.ID {
		t.Fatal("ID mismatch")
	}
}

func TestHeartbeat(t *testing.T) {
	m := NewManager()
	d, _ := m.Register("host1", "windows", "0.1.0", "standard", "fp-001")

	if !m.Heartbeat(d.ID) {
		t.Fatal("Heartbeat failed")
	}

	got, _ := m.Get(d.ID)
	if got.Status != StatusOnline {
		t.Fatal("Should be online after heartbeat")
	}

	unknownID := uuid.New()
	if m.Heartbeat(unknownID) {
		t.Fatal("Heartbeat should fail for unknown device")
	}
}

func TestDeregister(t *testing.T) {
	m := NewManager()
	d, _ := m.Register("host1", "windows", "0.1.0", "standard", "fp-001")

	if !m.Deregister(d.ID) {
		t.Fatal("Deregister failed")
	}
	if _, ok := m.Get(d.ID); ok {
		t.Fatal("Device should be removed")
	}
}

func TestDisable(t *testing.T) {
	m := NewManager()
	d, _ := m.Register("host1", "windows", "0.1.0", "standard", "fp-001")

	if !m.Disable(d.ID) {
		t.Fatal("Disable failed")
	}
	got, _ := m.Get(d.ID)
	if got.Status != StatusDisabled {
		t.Fatalf("Expected disabled, got %s", got.Status)
	}
}

func TestListAndListByGroup(t *testing.T) {
	m := NewManager()
	d1, _ := m.Register("host1", "windows", "0.1.0", "standard", "fp-001")
	d2, _ := m.Register("host2", "linux", "0.1.0", "standard", "fp-002")
	m.SetGroup(d2.ID, "group-b")
	_ = d1

	all := m.List()
	if len(all) != 2 {
		t.Fatalf("Expected 2 devices, got %d", len(all))
	}

	groupDefault := m.ListByGroup("default")
	if len(groupDefault) != 1 {
		t.Fatalf("Expected 1 in default group, got %d", len(groupDefault))
	}

	groupB := m.ListByGroup("group-b")
	if len(groupB) != 1 {
		t.Fatalf("Expected 1 in group-b, got %d", len(groupB))
	}
}

func TestCheckTimeouts(t *testing.T) {
	m := NewManager()
	m.SetHeartbeatTimeout(50 * time.Millisecond)

	d, _ := m.Register("host1", "windows", "0.1.0", "standard", "fp-001")
	time.Sleep(100 * time.Millisecond)

	timedOut := m.CheckTimeouts()
	if len(timedOut) != 1 {
		t.Fatalf("Expected 1 timed out, got %d", len(timedOut))
	}
	if timedOut[0] != d.ID {
		t.Fatal("Wrong device timed out")
	}

	got, _ := m.Get(d.ID)
	if got.Status != StatusOffline {
		t.Fatalf("Expected offline, got %s", got.Status)
	}
}

func TestPendingPolicy(t *testing.T) {
	m := NewManager()
	d, _ := m.Register("host1", "windows", "0.1.0", "standard", "fp-001")

	if !m.SetPendingPolicy(d.ID, "policy-123") {
		t.Fatal("SetPendingPolicy failed")
	}

	pending := m.GetPendingPolicyDevices()
	if len(pending) != 1 {
		t.Fatalf("Expected 1 pending, got %d", len(pending))
	}
	if pending[0].PendingPolicy != "policy-123" {
		t.Fatal("Wrong pending policy")
	}
}