package policy

import (
	"testing"

	"github.com/google/uuid"
)

func TestCreateAndGet(t *testing.T) {
	m := NewManager()
	scopeID := uuid.New()
	p, err := m.Create("backup-daily", "Daily backup", map[string]interface{}{"mode": "daily"}, 10, "group", scopeID, "admin")
	if err != nil {
		t.Fatalf("Create failed: %v", err)
	}
	if p.Version != 1 {
		t.Fatalf("Expected version 1, got %d", p.Version)
	}

	got, ok := m.Get(p.ID)
	if !ok {
		t.Fatal("Policy not found")
	}
	if got.Name != "backup-daily" {
		t.Fatalf("Expected name backup-daily, got %s", got.Name)
	}
}

func TestCreateEmptyName(t *testing.T) {
	m := NewManager()
	_, err := m.Create("", "desc", nil, 0, "global", uuid.Nil, "admin")
	if err == nil {
		t.Fatal("Should fail with empty name")
	}
}

func TestUpdateCreatesNewVersion(t *testing.T) {
	m := NewManager()
	p, _ := m.Create("p1", "desc", map[string]interface{}{"k": "v1"}, 0, "global", uuid.Nil, "admin")

	updated, err := m.Update(p.ID, map[string]interface{}{"k": "v2"}, "admin")
	if err != nil {
		t.Fatalf("Update failed: %v", err)
	}
	if updated.Version != 2 {
		t.Fatalf("Expected version 2, got %d", updated.Version)
	}

	versions := m.ListVersions(p.ID)
	if len(versions) != 2 {
		t.Fatalf("Expected 2 versions, got %d", len(versions))
	}
}

func TestDelete(t *testing.T) {
	m := NewManager()
	p, _ := m.Create("p1", "desc", nil, 0, "global", uuid.Nil, "admin")

	if !m.Delete(p.ID) {
		t.Fatal("Delete failed")
	}
	if _, ok := m.Get(p.ID); ok {
		t.Fatal("Policy should be deleted")
	}
}

func TestSetPriority(t *testing.T) {
	m := NewManager()
	p, _ := m.Create("p1", "desc", nil, 5, "global", uuid.Nil, "admin")

	if !m.SetPriority(p.ID, 100) {
		t.Fatal("SetPriority failed")
	}
	got, _ := m.Get(p.ID)
	if got.Priority != 100 {
		t.Fatalf("Expected priority 100, got %d", got.Priority)
	}
}

func TestListSortedByPriority(t *testing.T) {
	m := NewManager()
	m.Create("low", "", nil, 1, "global", uuid.Nil, "admin")
	m.Create("high", "", nil, 100, "global", uuid.Nil, "admin")
	m.Create("mid", "", nil, 50, "global", uuid.Nil, "admin")

	list := m.List()
	if list[0].Name != "high" {
		t.Fatalf("Expected high first, got %s", list[0].Name)
	}
	if list[1].Name != "mid" {
		t.Fatalf("Expected mid second, got %s", list[1].Name)
	}
}

func TestRollback(t *testing.T) {
	m := NewManager()
	p, _ := m.Create("p1", "desc", map[string]interface{}{"k": "v1"}, 0, "global", uuid.Nil, "admin")
	m.Update(p.ID, map[string]interface{}{"k": "v2"}, "admin")
	m.Update(p.ID, map[string]interface{}{"k": "v3"}, "admin")

	rolled, err := m.Rollback(p.ID, 1, "admin")
	if err != nil {
		t.Fatalf("Rollback failed: %v", err)
	}
	if rolled.Version != 4 {
		t.Fatalf("Expected version 4 after rollback, got %d", rolled.Version)
	}
	if rolled.Template["k"] != "v1" {
		t.Fatalf("Expected v1 after rollback to version 1, got %v", rolled.Template["k"])
	}
}

func TestRollbackVersionNotFound(t *testing.T) {
	m := NewManager()
	p, _ := m.Create("p1", "desc", nil, 0, "global", uuid.Nil, "admin")

	_, err := m.Rollback(p.ID, 99, "admin")
	if err == nil {
		t.Fatal("Should fail to rollback to non-existent version")
	}
}

func TestCalculateImpactScopeDevice(t *testing.T) {
	m := NewManager()
	deviceID := uuid.New()
	p, _ := m.Create("p1", "desc", nil, 0, "device", deviceID, "admin")

	scope, err := m.CalculateImpactScope(p.ID,
		func(string) []uuid.UUID { return nil },
		func() []uuid.UUID { return nil },
	)
	if err != nil {
		t.Fatalf("CalculateImpactScope failed: %v", err)
	}
	if len(scope) != 1 || scope[0] != deviceID {
		t.Fatalf("Expected [deviceID], got %v", scope)
	}
}

func TestCalculateImpactScopeGroup(t *testing.T) {
	m := NewManager()
	groupID := uuid.New()
	p, _ := m.Create("p1", "desc", nil, 0, "group", groupID, "admin")

	d1, d2 := uuid.New(), uuid.New()
	scope, err := m.CalculateImpactScope(p.ID,
		func(gid string) []uuid.UUID {
			if gid == groupID.String() {
				return []uuid.UUID{d1, d2}
			}
			return nil
		},
		func() []uuid.UUID { return nil },
	)
	if err != nil {
		t.Fatalf("CalculateImpactScope failed: %v", err)
	}
	if len(scope) != 2 {
		t.Fatalf("Expected 2 devices, got %d", len(scope))
	}
}

func TestCalculateImpactScopeGlobal(t *testing.T) {
	m := NewManager()
	p, _ := m.Create("p1", "desc", nil, 0, "global", uuid.Nil, "admin")

	scope, err := m.CalculateImpactScope(p.ID,
		func(string) []uuid.UUID { return nil },
		func() []uuid.UUID { return []uuid.UUID{uuid.New(), uuid.New(), uuid.New()} },
	)
	if err != nil {
		t.Fatalf("CalculateImpactScope failed: %v", err)
	}
	if len(scope) != 3 {
		t.Fatalf("Expected 3 devices, got %d", len(scope))
	}
}