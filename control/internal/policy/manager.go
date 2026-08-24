package policy

import (
	"errors"
	"sort"
	"sync"
	"time"

	"github.com/google/uuid"
)

// Policy 策略定义
type Policy struct {
	ID          uuid.UUID
	Name        string
	Description string
	Template    map[string]interface{}
	Priority    int
	Version     int
	ScopeType   string
	ScopeID     uuid.UUID
	Status      string
	CreatedAt   time.Time
	UpdatedAt   time.Time
}

// PolicyVersion 策略版本
type PolicyVersion struct {
	PolicyID  uuid.UUID
	Version   int
	Template  map[string]interface{}
	CreatedAt time.Time
	CreatedBy string
}

// Manager 策略管理器
type Manager struct {
	mu       sync.RWMutex
	policies map[uuid.UUID]*Policy
	versions map[uuid.UUID][]PolicyVersion
}

// NewManager 创建策略管理器
func NewManager() *Manager {
	return &Manager{
		policies: make(map[uuid.UUID]*Policy),
		versions: make(map[uuid.UUID][]PolicyVersion),
	}
}

// Create 创建策略
func (m *Manager) Create(name, description string, template map[string]interface{}, priority int, scopeType string, scopeID uuid.UUID, createdBy string) (*Policy, error) {
	m.mu.Lock()
	defer m.mu.Unlock()

	if name == "" {
		return nil, errors.New("policy name is required")
	}

	id := uuid.New()
	now := time.Now().UTC()
	policy := &Policy{
		ID:          id,
		Name:        name,
		Description: description,
		Template:    template,
		Priority:    priority,
		Version:     1,
		ScopeType:   scopeType,
		ScopeID:     scopeID,
		Status:      "active",
		CreatedAt:   now,
		UpdatedAt:   now,
	}
	m.policies[id] = policy

	m.versions[id] = []PolicyVersion{
		{
			PolicyID:  id,
			Version:   1,
			Template:  copyTemplate(template),
			CreatedAt: now,
			CreatedBy: createdBy,
		},
	}

	return policy, nil
}

// Get 获取策略
func (m *Manager) Get(id uuid.UUID) (*Policy, bool) {
	m.mu.RLock()
	defer m.mu.RUnlock()

	p, ok := m.policies[id]
	if !ok {
		return nil, false
	}
	return p, true
}

// List 列出所有策略
func (m *Manager) List() []*Policy {
	m.mu.RLock()
	defer m.mu.RUnlock()

	result := make([]*Policy, 0, len(m.policies))
	for _, p := range m.policies {
		result = append(result, p)
	}
	sort.Slice(result, func(i, j int) bool {
		if result[i].Priority != result[j].Priority {
			return result[i].Priority > result[j].Priority
		}
		return result[i].Name < result[j].Name
	})
	return result
}

// Update 更新策略（创建新版本）
func (m *Manager) Update(id uuid.UUID, template map[string]interface{}, updatedBy string) (*Policy, error) {
	m.mu.Lock()
	defer m.mu.Unlock()

	p, ok := m.policies[id]
	if !ok {
		return nil, errors.New("policy not found")
	}

	p.Version++
	p.Template = template
	p.UpdatedAt = time.Now().UTC()

	m.versions[id] = append(m.versions[id], PolicyVersion{
		PolicyID:  id,
		Version:   p.Version,
		Template:  copyTemplate(template),
		CreatedAt: p.UpdatedAt,
		CreatedBy: updatedBy,
	})

	return p, nil
}

// Delete 删除策略
func (m *Manager) Delete(id uuid.UUID) bool {
	m.mu.Lock()
	defer m.mu.Unlock()

	if _, ok := m.policies[id]; !ok {
		return false
	}
	delete(m.policies, id)
	delete(m.versions, id)
	return true
}

// SetPriority 设置策略优先级
func (m *Manager) SetPriority(id uuid.UUID, priority int) bool {
	m.mu.Lock()
	defer m.mu.Unlock()

	p, ok := m.policies[id]
	if !ok {
		return false
	}
	p.Priority = priority
	p.UpdatedAt = time.Now().UTC()
	return true
}

// ListVersions 列出策略版本
func (m *Manager) ListVersions(id uuid.UUID) []PolicyVersion {
	m.mu.RLock()
	defer m.mu.RUnlock()

	versions, ok := m.versions[id]
	if !ok {
		return []PolicyVersion{}
	}
	result := make([]PolicyVersion, len(versions))
	copy(result, versions)
	sort.Slice(result, func(i, j int) bool {
		return result[i].Version > result[j].Version
	})
	return result
}

// Rollback 回滚策略到指定版本
func (m *Manager) Rollback(id uuid.UUID, targetVersion int, rolledBy string) (*Policy, error) {
	m.mu.Lock()
	defer m.mu.Unlock()

	p, ok := m.policies[id]
	if !ok {
		return nil, errors.New("policy not found")
	}

	versions := m.versions[id]
	var target *PolicyVersion
	for i := range versions {
		if versions[i].Version == targetVersion {
			target = &versions[i]
			break
		}
	}
	if target == nil {
		return nil, errors.New("target version not found")
	}

	p.Version++
	p.Template = copyTemplate(target.Template)
	p.UpdatedAt = time.Now().UTC()

	m.versions[id] = append(m.versions[id], PolicyVersion{
		PolicyID:  id,
		Version:   p.Version,
		Template:  copyTemplate(target.Template),
		CreatedAt: p.UpdatedAt,
		CreatedBy: rolledBy,
	})

	return p, nil
}

// CalculateImpactScope 计算策略影响范围
// scopeType: "device" → 影响单个设备; "group" → 影响该组所有设备; "global" → 影响所有设备
func (m *Manager) CalculateImpactScope(id uuid.UUID, listDevicesByGroup func(string) []uuid.UUID, listAllDevices func() []uuid.UUID) ([]uuid.UUID, error) {
	m.mu.RLock()
	defer m.mu.RUnlock()

	p, ok := m.policies[id]
	if !ok {
		return nil, errors.New("policy not found")
	}

	switch p.ScopeType {
	case "device":
		return []uuid.UUID{p.ScopeID}, nil
	case "group":
		return listDevicesByGroup(p.ScopeID.String()), nil
	case "global":
		return listAllDevices(), nil
	default:
		return nil, errors.New("unknown scope type")
	}
}

func copyTemplate(t map[string]interface{}) map[string]interface{} {
	if t == nil {
		return nil
	}
	result := make(map[string]interface{}, len(t))
	for k, v := range t {
		result[k] = v
	}
	return result
}