package testorch

import (
	"fmt"
	"sync"
	"time"

	"github.com/google/uuid"
)

type ReferenceInstance struct {
	ID        uuid.UUID
	RunID     string
	Port      int
	Namespace string
	Status    string
	StartedAt time.Time
}

type BehaviorSample struct {
	VersionStructure map[string]interface{}
	FileManifest     []string
	ExceptionDecisions map[string]interface{}
	CollectedAt      time.Time
}

type DuplicatiReferenceManager struct {
	mu        sync.RWMutex
	instances map[uuid.UUID]*ReferenceInstance
	nextPort  int
}

func NewDuplicatiReferenceManager() *DuplicatiReferenceManager {
	return &DuplicatiReferenceManager{
		instances: make(map[uuid.UUID]*ReferenceInstance),
		nextPort:  8200,
	}
}

func (m *DuplicatiReferenceManager) StartInstance(runID string) (*ReferenceInstance, error) {
	m.mu.Lock()
	defer m.mu.Unlock()

	port := m.nextPort
	m.nextPort++

	instance := &ReferenceInstance{
		ID:        uuid.New(),
		RunID:     runID,
		Port:      port,
		Namespace: fmt.Sprintf("duplicati-ref/%s/", runID),
		Status:    "running",
		StartedAt: time.Now().UTC(),
	}
	m.instances[instance.ID] = instance
	return instance, nil
}

func (m *DuplicatiReferenceManager) StopInstance(id uuid.UUID) bool {
	m.mu.Lock()
	defer m.mu.Unlock()
	inst, ok := m.instances[id]
	if !ok {
		return false
	}
	inst.Status = "stopped"
	return true
}

func (m *DuplicatiReferenceManager) HealthCheck(id uuid.UUID) bool {
	m.mu.RLock()
	defer m.mu.RUnlock()
	inst, ok := m.instances[id]
	return ok && inst.Status == "running"
}

func (m *DuplicatiReferenceManager) AllocateNamespace(runID string, isDuplicati bool) string {
	if isDuplicati {
		return fmt.Sprintf("duplicati-ref/%s/", runID)
	}
	return fmt.Sprintf("hbx-compat/%s/", runID)
}

func (m *DuplicatiReferenceManager) SampleBehavior(instanceID uuid.UUID, operation string) (*BehaviorSample, error) {
	m.mu.RLock()
	defer m.mu.RUnlock()
	inst, ok := m.instances[instanceID]
	if !ok {
		return nil, fmt.Errorf("instance not found")
	}
	if inst.Status != "running" {
		return nil, fmt.Errorf("instance not running")
	}

	return &BehaviorSample{
		VersionStructure: map[string]interface{}{
			"operation": operation,
			"namespace": inst.Namespace,
		},
		FileManifest:     []string{},
		ExceptionDecisions: map[string]interface{}{},
		CollectedAt:      time.Now().UTC(),
	}, nil
}

func (m *DuplicatiReferenceManager) ListInstances() []*ReferenceInstance {
	m.mu.RLock()
	defer m.mu.RUnlock()
	result := make([]*ReferenceInstance, 0, len(m.instances))
	for _, inst := range m.instances {
		result = append(result, inst)
	}
	return result
}