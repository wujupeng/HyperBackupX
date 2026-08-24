package logagg

import (
	"context"
	"fmt"
	"sync"
	"time"
)

// RetentionPolicy 日志保留策略
type RetentionPolicy struct {
	DailyRetention   int
	WeeklyRetention  int
	MonthlyRetention int
}

// DefaultRetentionPolicy 默认保留策略（7天日 + 4周 + 12月）
func DefaultRetentionPolicy() RetentionPolicy {
	return RetentionPolicy{
		DailyRetention:   7,
		WeeklyRetention:  4,
		MonthlyRetention: 12,
	}
}

// Partition 分区信息
type Partition struct {
	Name       string
	Table      string
	StartDate  time.Time
	EndDate    time.Time
	SizeBytes  int64
	IsArchived bool
}

// RetentionManager 日志保留管理器
type RetentionManager struct {
	mu       sync.RWMutex
	policy   RetentionPolicy
	partitions []Partition
}

// NewRetentionManager 创建保留管理器
func NewRetentionManager(policy RetentionPolicy) *RetentionManager {
	return &RetentionManager{policy: policy}
}

// AddPartition 添加分区
func (m *RetentionManager) AddPartition(p Partition) {
	m.mu.Lock()
	defer m.mu.Unlock()
	m.partitions = append(m.partitions, p)
}

// ListPartitions 列出所有分区
func (m *RetentionManager) ListPartitions() []Partition {
	m.mu.RLock()
	defer m.mu.RUnlock()
	result := make([]Partition, len(m.partitions))
	copy(result, m.partitions)
	return result
}

// GetExpiredPartitions 获取需要清理的过期分区
func (m *RetentionManager) GetExpiredPartitions(now time.Time) []Partition {
	m.mu.RLock()
	defer m.mu.RUnlock()

	var expired []Partition
	dailyCutoff := now.AddDate(0, 0, -m.policy.DailyRetention)
	weeklyCutoff := now.AddDate(0, 0, -7*m.policy.WeeklyRetention)
	monthlyCutoff := now.AddDate(0, -m.policy.MonthlyRetention, 0)

	for _, p := range m.partitions {
		if p.IsArchived {
			continue
		}
		age := now.Sub(p.StartDate)
		if age > 365*24*time.Hour && p.EndDate.Before(monthlyCutoff) {
			expired = append(expired, p)
		} else if age > 30*24*time.Hour && p.EndDate.Before(weeklyCutoff) {
			expired = append(expired, p)
		} else if p.EndDate.Before(dailyCutoff) {
			expired = append(expired, p)
		}
	}
	return expired
}

// ArchivePartition 归档分区
func (m *RetentionManager) ArchivePartition(name string) bool {
	m.mu.Lock()
	defer m.mu.Unlock()

	for i := range m.partitions {
		if m.partitions[i].Name == name {
			m.partitions[i].IsArchived = true
			return true
		}
	}
	return false
}

// CleanupExpired 清理过期分区
func (m *RetentionManager) CleanupExpired(ctx context.Context, dropFn func(string) error) (int, error) {
	now := time.Now().UTC()
	expired := m.GetExpiredPartitions(now)

	cleaned := 0
	for _, p := range expired {
		select {
		case <-ctx.Done():
			return cleaned, ctx.Err()
		default:
		}
		if err := dropFn(p.Name); err != nil {
			return cleaned, fmt.Errorf("failed to drop %s: %w", p.Name, err)
		}
		m.ArchivePartition(p.Name)
		cleaned++
	}
	return cleaned, nil
}

// SetPolicy 更新保留策略
func (m *RetentionManager) SetPolicy(policy RetentionPolicy) {
	m.mu.Lock()
	defer m.mu.Unlock()
	m.policy = policy
}

// GetPolicy 获取保留策略
func (m *RetentionManager) GetPolicy() RetentionPolicy {
	m.mu.RLock()
	defer m.mu.RUnlock()
	return m.policy
}

// GeneratePartitionName 生成分区名（按天）
func GeneratePartitionName(baseTable string, date time.Time) string {
	return fmt.Sprintf("%s_%s", baseTable, date.Format("20060102"))
}

// EnsurePartition 确保分区存在（为 pg_partman 提供兼容接口）
func (m *RetentionManager) EnsurePartition(baseTable string, date time.Time) Partition {
	name := GeneratePartitionName(baseTable, date)
	start := time.Date(date.Year(), date.Month(), date.Day(), 0, 0, 0, 0, time.UTC)
	end := start.AddDate(0, 0, 1)

	m.mu.RLock()
	for _, p := range m.partitions {
		if p.Name == name {
			m.mu.RUnlock()
			return p
		}
	}
	m.mu.RUnlock()

	partition := Partition{
		Name:      name,
		Table:     baseTable,
		StartDate: start,
		EndDate:   end,
	}
	m.AddPartition(partition)
	return partition
}