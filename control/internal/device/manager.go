package device

import (
	"context"
	"sync"
	"time"

	"github.com/google/uuid"
)

// DeviceStatus 设备状态
type DeviceStatus string

const (
	StatusOnline   DeviceStatus = "online"
	StatusOffline  DeviceStatus = "offline"
	StatusDisabled DeviceStatus = "disabled"
)

// Device 设备信息
type Device struct {
	ID            uuid.UUID
	Hostname      string
	OsType        string
	AgentVersion  string
	HardwareTier  string
	GroupID       string
	Status        DeviceStatus
	LastSeenAt    time.Time
	RegisteredAt  time.Time
	Fingerprint   string
	PendingPolicy string
}

// Manager 设备管理器
type Manager struct {
	mu         sync.RWMutex
	devices    map[uuid.UUID]*Device
	heartbeatTimeout time.Duration
}

// NewManager 创建设备管理器
func NewManager() *Manager {
	return &Manager{
		devices:          make(map[uuid.UUID]*Device),
		heartbeatTimeout: 90 * time.Second,
	}
}

// Register 注册新设备
func (m *Manager) Register(hostname, osType, agentVersion, tier, fingerprint string) (*Device, error) {
	m.mu.Lock()
	defer m.mu.Unlock()

	id := uuid.New()
	device := &Device{
		ID:           id,
		Hostname:     hostname,
		OsType:       osType,
		AgentVersion: agentVersion,
		HardwareTier: tier,
		GroupID:      "default",
		Status:       StatusOnline,
		LastSeenAt:   time.Now().UTC(),
		RegisteredAt: time.Now().UTC(),
		Fingerprint:  fingerprint,
	}
	m.devices[id] = device
	return device, nil
}

// Heartbeat 更新设备心跳
func (m *Manager) Heartbeat(deviceID uuid.UUID) bool {
	m.mu.Lock()
	defer m.mu.Unlock()

	device, ok := m.devices[deviceID]
	if !ok {
		return false
	}
	device.LastSeenAt = time.Now().UTC()
	device.Status = StatusOnline
	return true
}

// Deregister 注销设备
func (m *Manager) Deregister(deviceID uuid.UUID) bool {
	m.mu.Lock()
	defer m.mu.Unlock()

	if _, ok := m.devices[deviceID]; !ok {
		return false
	}
	delete(m.devices, deviceID)
	return true
}

// Disable 禁用设备
func (m *Manager) Disable(deviceID uuid.UUID) bool {
	m.mu.Lock()
	defer m.mu.Unlock()

	device, ok := m.devices[deviceID]
	if !ok {
		return false
	}
	device.Status = StatusDisabled
	return true
}

// Get 获取设备
func (m *Manager) Get(deviceID uuid.UUID) (*Device, bool) {
	m.mu.RLock()
	defer m.mu.RUnlock()

	device, ok := m.devices[deviceID]
	if !ok {
		return nil, false
	}
	return device, true
}

// List 列出所有设备
func (m *Manager) List() []*Device {
	m.mu.RLock()
	defer m.mu.RUnlock()

	result := make([]*Device, 0, len(m.devices))
	for _, d := range m.devices {
		result = append(result, d)
	}
	return result
}

// ListByGroup 按分组列出设备
func (m *Manager) ListByGroup(groupID string) []*Device {
	m.mu.RLock()
	defer m.mu.RUnlock()

	var result []*Device
	for _, d := range m.devices {
		if d.GroupID == groupID {
			result = append(result, d)
		}
	}
	return result
}

// SetGroup 设置设备分组
func (m *Manager) SetGroup(deviceID uuid.UUID, groupID string) bool {
	m.mu.Lock()
	defer m.mu.Unlock()

	device, ok := m.devices[deviceID]
	if !ok {
		return false
	}
	device.GroupID = groupID
	return true
}

// CheckTimeouts 检查心跳超时，将超时设备标记为离线
func (m *Manager) CheckTimeouts() []uuid.UUID {
	m.mu.Lock()
	defer m.mu.Unlock()

	var timedOut []uuid.UUID
	now := time.Now().UTC()
	for _, d := range m.devices {
		if d.Status == StatusOnline && now.Sub(d.LastSeenAt) > m.heartbeatTimeout {
			d.Status = StatusOffline
			timedOut = append(timedOut, d.ID)
		}
	}
	return timedOut
}

// SetHeartbeatTimeout 设置心跳超时时间
func (m *Manager) SetHeartbeatTimeout(timeout time.Duration) {
	m.mu.Lock()
	defer m.mu.Unlock()
	m.heartbeatTimeout = timeout
}

// SetPendingPolicy 设置待下发策略
func (m *Manager) SetPendingPolicy(deviceID uuid.UUID, policyID string) bool {
	m.mu.Lock()
	defer m.mu.Unlock()

	device, ok := m.devices[deviceID]
	if !ok {
		return false
	}
	device.PendingPolicy = policyID
	return true
}

// GetPendingPolicyDevices 获取有待下发策略的设备
func (m *Manager) GetPendingPolicyDevices() []*Device {
	m.mu.RLock()
	defer m.mu.RUnlock()

	var result []*Device
	for _, d := range m.devices {
		if d.PendingPolicy != "" {
			result = append(result, d)
		}
	}
	return result
}

// StartTimeoutChecker 启动心跳超时检查器
func (m *Manager) StartTimeoutChecker(ctx context.Context, interval time.Duration) {
	go func() {
		ticker := time.NewTicker(interval)
		defer ticker.Stop()
		for {
			select {
			case <-ctx.Done():
				return
			case <-ticker.C:
				m.CheckTimeouts()
			}
		}
	}()
}