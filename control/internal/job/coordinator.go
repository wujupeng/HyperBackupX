package job

import (
	"context"
	"errors"
	"fmt"
	"sync"
	"time"

	"github.com/google/uuid"
)

// JobStatus 任务状态
type JobStatus string

const (
	StatusPending   JobStatus = "pending"
	StatusRunning   JobStatus = "running"
	StatusCompleted JobStatus = "completed"
	StatusFailed    JobStatus = "failed"
	StatusCancelled JobStatus = "cancelled"
)

// Job 备份任务
type Job struct {
	ID          uuid.UUID
	DeviceID    uuid.UUID
	Name        string
	PolicyID    *uuid.UUID
	Status      JobStatus
	SourceConfig  map[string]interface{}
	DestConfig    map[string]interface{}
	CreatedAt   time.Time
	StartedAt   *time.Time
	CompletedAt *time.Time
	ErrorMsg    string
}

// TaskResult 任务执行结果
type TaskResult struct {
	JobID          uuid.UUID
	DeviceID       uuid.UUID
	Status         JobStatus
	BytesProcessed uint64
	BytesStored    uint64
	FileCount      uint32
	ChunkCount     uint32
	DedupRatio     float64
	VersionID      *string
	ErrorMsg       string
	CompletedAt    time.Time
}

// Command 下发给 Agent 的命令
type Command struct {
	ID        uuid.UUID
	DeviceID  uuid.UUID
	Type      CommandType
	Payload   map[string]interface{}
	CreatedAt time.Time
}

// CommandType 命令类型
type CommandType string

const (
	CmdPolicyUpdate  CommandType = "policy_update"
	CmdTriggerBackup CommandType = "trigger_backup"
	CmdUpgradeAgent  CommandType = "upgrade_agent"
	CmdPauseTask     CommandType = "pause_task"
	CmdResumeTask    CommandType = "resume_task"
)

// Coordinator 任务编排器
type Coordinator struct {
	mu       sync.RWMutex
	jobs     map[uuid.UUID]*Job
	commands map[uuid.UUID][]Command
	results  map[uuid.UUID][]TaskResult
}

// NewCoordinator 创建任务编排器
func NewCoordinator() *Coordinator {
	return &Coordinator{
		jobs:     make(map[uuid.UUID]*Job),
		commands: make(map[uuid.UUID][]Command),
		results:  make(map[uuid.UUID][]TaskResult),
	}
}

// CreateJob 创建任务
func (c *Coordinator) CreateJob(deviceID uuid.UUID, name string, sourceConfig, destConfig map[string]interface{}) (*Job, error) {
	c.mu.Lock()
	defer c.mu.Unlock()

	if name == "" {
		return nil, errors.New("job name is required")
	}

	id := uuid.New()
	job := &Job{
		ID:          id,
		DeviceID:    deviceID,
		Name:        name,
		Status:      StatusPending,
		SourceConfig: sourceConfig,
		DestConfig:   destConfig,
		CreatedAt:   time.Now().UTC(),
	}
	c.jobs[id] = job
	return job, nil
}

// TriggerJob 手动触发任务
func (c *Coordinator) TriggerJob(jobID uuid.UUID) (*Command, error) {
	c.mu.Lock()
	defer c.mu.Unlock()

	job, ok := c.jobs[jobID]
	if !ok {
		return nil, errors.New("job not found")
	}
	if job.Status != StatusPending {
		return nil, fmt.Errorf("job is not pending (current: %s)", job.Status)
	}

	cmd := Command{
		ID:        uuid.New(),
		DeviceID:  job.DeviceID,
		Type:      CmdTriggerBackup,
		Payload:   map[string]interface{}{"job_id": jobID.String()},
		CreatedAt: time.Now().UTC(),
	}
	c.commands[job.DeviceID] = append(c.commands[job.DeviceID], cmd)

	job.Status = StatusRunning
	now := time.Now().UTC()
	job.StartedAt = &now

	return &cmd, nil
}

// GetJob 获取任务
func (c *Coordinator) GetJob(id uuid.UUID) (*Job, bool) {
	c.mu.RLock()
	defer c.mu.RUnlock()

	job, ok := c.jobs[id]
	if !ok {
		return nil, false
	}
	return job, true
}

// ListJobs 列出任务
func (c *Coordinator) ListJobs() []*Job {
	c.mu.RLock()
	defer c.mu.RUnlock()

	result := make([]*Job, 0, len(c.jobs))
	for _, j := range c.jobs {
		result = append(result, j)
	}
	return result
}

// ListJobsByDevice 按设备列出任务
func (c *Coordinator) ListJobsByDevice(deviceID uuid.UUID) []*Job {
	c.mu.RLock()
	defer c.mu.RUnlock()

	var result []*Job
	for _, j := range c.jobs {
		if j.DeviceID == deviceID {
			result = append(result, j)
		}
	}
	return result
}

// RecordResult 记录任务结果
func (c *Coordinator) RecordResult(result TaskResult) error {
	c.mu.Lock()
	defer c.mu.Unlock()

	job, ok := c.jobs[result.JobID]
	if !ok {
		return errors.New("job not found")
	}

	job.Status = result.Status
	now := result.CompletedAt
	job.CompletedAt = &now
	job.ErrorMsg = result.ErrorMsg

	c.results[result.JobID] = append(c.results[result.JobID], result)
	return nil
}

// GetPendingCommands 获取设备的待执行命令
func (c *Coordinator) GetPendingCommands(deviceID uuid.UUID) []Command {
	c.mu.Lock()
	defer c.mu.Unlock()

	cmds := c.commands[deviceID]
	result := make([]Command, len(cmds))
	copy(result, cmds)
	c.commands[deviceID] = []Command{}
	return result
}

// DistributePolicy 分发策略到设备
func (c *Coordinator) DistributePolicy(deviceID uuid.UUID, policyID uuid.UUID, policyPayload map[string]interface{}) *Command {
	c.mu.Lock()
	defer c.mu.Unlock()

	cmd := Command{
		ID:        uuid.New(),
		DeviceID:  deviceID,
		Type:      CmdPolicyUpdate,
		Payload:   map[string]interface{}{"policy_id": policyID.String(), "policy": policyPayload},
		CreatedAt: time.Now().UTC(),
	}
	c.commands[deviceID] = append(c.commands[deviceID], cmd)
	return &cmd
}

// DistributePolicyToGroup 分发策略到设备组
func (c *Coordinator) DistributePolicyToGroup(deviceIDs []uuid.UUID, policyID uuid.UUID, policyPayload map[string]interface{}) []Command {
	var cmds []Command
	for _, deviceID := range deviceIDs {
		cmd := c.DistributePolicy(deviceID, policyID, policyPayload)
		cmds = append(cmds, *cmd)
	}
	return cmds
}

// AggregateStatus 汇总任务状态
type StatusSummary struct {
	Total     int
	Pending   int
	Running   int
	Completed int
	Failed    int
	Cancelled int
}

func (c *Coordinator) AggregateStatus() StatusSummary {
	c.mu.RLock()
	defer c.mu.RUnlock()

	summary := StatusSummary{Total: len(c.jobs)}
	for _, j := range c.jobs {
		switch j.Status {
		case StatusPending:
			summary.Pending++
		case StatusRunning:
			summary.Running++
		case StatusCompleted:
			summary.Completed++
		case StatusFailed:
			summary.Failed++
		case StatusCancelled:
			summary.Cancelled++
		}
	}
	return summary
}

// CancelJob 取消任务
func (c *Coordinator) CancelJob(jobID uuid.UUID) error {
	c.mu.Lock()
	defer c.mu.Unlock()

	job, ok := c.jobs[jobID]
	if !ok {
		return errors.New("job not found")
	}
	if job.Status == StatusCompleted || job.Status == StatusFailed {
		return errors.New("cannot cancel finished job")
	}
	job.Status = StatusCancelled
	return nil
}

// StartResultProcessor 启动结果处理器（占位，实际实现需要消息队列）
func (c *Coordinator) StartResultProcessor(ctx context.Context) {
	_ = ctx
}