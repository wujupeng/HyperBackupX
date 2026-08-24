package upgrade

import (
	"context"
	"crypto/sha256"
	"encoding/hex"
	"errors"
	"fmt"
	"io"
	"net/http"
	"os"
	"path/filepath"
	"sync"
	"time"

	"github.com/google/uuid"
)

// UpgradeStatus 升级状态
type UpgradeStatus string

const (
	StatusDownloading   UpgradeStatus = "downloading"
	StatusWaiting       UpgradeStatus = "waiting"
	StatusReplacing     UpgradeStatus = "replacing"
	StatusRestarting    UpgradeStatus = "restarting"
	StatusCompleted     UpgradeStatus = "completed"
	StatusFailed        UpgradeStatus = "failed"
	StatusRolledBack    UpgradeStatus = "rolled_back"
)

// UpgradeTask 升级任务
type UpgradeTask struct {
	ID           uuid.UUID
	DeviceID     uuid.UUID
	FromVersion  string
	ToVersion    string
	DownloadURL  string
	Checksum     string
	Status       UpgradeStatus
	StartedAt    time.Time
	CompletedAt  *time.Time
	ErrorMsg     string
	BackupPath   string
	NewBinaryPath string
}

// Manager 升级管理器
type Manager struct {
	mu     sync.RWMutex
	tasks  map[uuid.UUID]*UpgradeTask
	client *http.Client
	workDir string
}

// NewManager 创建升级管理器
func NewManager(workDir string) *Manager {
	if workDir == "" {
		workDir = filepath.Join(os.TempDir(), "hbx-upgrades")
	}
	return &Manager{
		tasks:   make(map[uuid.UUID]*UpgradeTask),
		client:  &http.Client{Timeout: 30 * time.Minute},
		workDir: workDir,
	}
}

// CreateUpgradeTask 创建升级任务
func (m *Manager) CreateUpgradeTask(deviceID uuid.UUID, fromVersion, toVersion, downloadURL, checksum string) *UpgradeTask {
	m.mu.Lock()
	defer m.mu.Unlock()

	task := &UpgradeTask{
		ID:          uuid.New(),
		DeviceID:    deviceID,
		FromVersion: fromVersion,
		ToVersion:   toVersion,
		DownloadURL: downloadURL,
		Checksum:    checksum,
		Status:      StatusDownloading,
		StartedAt:   time.Now().UTC(),
	}
	m.tasks[task.ID] = task
	return task
}

// DownloadBinary 下载新版本二进制
func (m *Manager) DownloadBinary(ctx context.Context, taskID uuid.UUID) error {
	m.mu.Lock()
	task, ok := m.tasks[taskID]
	if !ok {
		m.mu.Unlock()
		return errors.New("task not found")
	}
	m.mu.Unlock()

	os.MkdirAll(m.workDir, 0755)

	binaryPath := filepath.Join(m.workDir, fmt.Sprintf("agent-%s-%s", task.ToVersion, task.ID.String()))
	task.NewBinaryPath = binaryPath

	resp, err := m.client.Get(task.DownloadURL)
	if err != nil {
		m.markFailed(taskID, fmt.Sprintf("download failed: %v", err))
		return err
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		m.markFailed(taskID, fmt.Sprintf("download returned %d", resp.StatusCode))
		return fmt.Errorf("download returned %d", resp.StatusCode)
	}

	file, err := os.Create(binaryPath)
	if err != nil {
		m.markFailed(taskID, fmt.Sprintf("create file failed: %v", err))
		return err
	}
	defer file.Close()

	hasher := sha256.New()
	writer := io.MultiWriter(file, hasher)

	if _, err := io.Copy(writer, resp.Body); err != nil {
		m.markFailed(taskID, fmt.Sprintf("write failed: %v", err))
		return err
	}

	downloadedChecksum := hex.EncodeToString(hasher.Sum(nil))
	if task.Checksum != "" && downloadedChecksum != task.Checksum {
		os.Remove(binaryPath)
		m.markFailed(taskID, fmt.Sprintf("checksum mismatch: expected %s, got %s", task.Checksum, downloadedChecksum))
		return errors.New("checksum mismatch")
	}

	m.mu.Lock()
	task.Status = StatusWaiting
	m.mu.Unlock()

	return nil
}

// BackupCurrentBinary 备份当前二进制（用于回滚）
func (m *Manager) BackupCurrentBinary(taskID uuid.UUID, currentPath string) error {
	m.mu.Lock()
	task, ok := m.tasks[taskID]
	if !ok {
		m.mu.Unlock()
		return errors.New("task not found")
	}
	m.mu.Unlock()

	backupPath := filepath.Join(m.workDir, fmt.Sprintf("agent-backup-%s-%s", task.FromVersion, task.ID.String()))
	task.BackupPath = backupPath

	src, err := os.Open(currentPath)
	if err != nil {
		return fmt.Errorf("open current binary failed: %w", err)
	}
	defer src.Close()

	dst, err := os.Create(backupPath)
	if err != nil {
		return fmt.Errorf("create backup failed: %w", err)
	}
	defer dst.Close()

	if _, err := io.Copy(dst, src); err != nil {
		return fmt.Errorf("backup copy failed: %w", err)
	}

	return nil
}

// ReplaceBinary 替换二进制文件
func (m *Manager) ReplaceBinary(taskID uuid.UUID, targetPath string) error {
	m.mu.Lock()
	task, ok := m.tasks[taskID]
	if !ok {
		m.mu.Unlock()
		return errors.New("task not found")
	}
	task.Status = StatusReplacing
	m.mu.Unlock()

	if task.BackupPath == "" {
		if err := m.BackupCurrentBinary(taskID, targetPath); err != nil {
			m.markFailed(taskID, fmt.Sprintf("backup failed: %v", err))
			return err
		}
	}

	if err := os.Rename(task.NewBinaryPath, targetPath); err != nil {
		m.markFailed(taskID, fmt.Sprintf("replace failed: %v", err))
		return err
	}

	m.mu.Lock()
	task.Status = StatusRestarting
	m.mu.Unlock()

	return nil
}

// CompleteUpgrade 完成升级
func (m *Manager) CompleteUpgrade(taskID uuid.UUID) {
	m.mu.Lock()
	defer m.mu.Unlock()

	task, ok := m.tasks[taskID]
	if !ok {
		return
	}
	now := time.Now().UTC()
	task.Status = StatusCompleted
	task.CompletedAt = &now
}

// Rollback 回滚到之前的版本
func (m *Manager) Rollback(taskID uuid.UUID, targetPath string) error {
	m.mu.Lock()
	task, ok := m.tasks[taskID]
	if !ok {
		m.mu.Unlock()
		return errors.New("task not found")
	}
	m.mu.Unlock()

	if task.BackupPath == "" {
		return errors.New("no backup available for rollback")
	}

	if _, err := os.Stat(task.BackupPath); err != nil {
		return fmt.Errorf("backup file not found: %w", err)
	}

	if err := os.Rename(task.BackupPath, targetPath); err != nil {
		return fmt.Errorf("rollback replace failed: %w", err)
	}

	m.mu.Lock()
	task.Status = StatusRolledBack
	now := time.Now().UTC()
	task.CompletedAt = &now
	m.mu.Unlock()

	return nil
}

// GetTask 获取升级任务
func (m *Manager) GetTask(id uuid.UUID) (*UpgradeTask, bool) {
	m.mu.RLock()
	defer m.mu.RUnlock()

	task, ok := m.tasks[id]
	if !ok {
		return nil, false
	}
	return task, true
}

// ListTasks 列出升级任务
func (m *Manager) ListTasks() []*UpgradeTask {
	m.mu.RLock()
	defer m.mu.RUnlock()

	result := make([]*UpgradeTask, 0, len(m.tasks))
	for _, t := range m.tasks {
		result = append(result, t)
	}
	return result
}

// ListTasksByDevice 按设备列出升级任务
func (m *Manager) ListTasksByDevice(deviceID uuid.UUID) []*UpgradeTask {
	m.mu.RLock()
	defer m.mu.RUnlock()

	var result []*UpgradeTask
	for _, t := range m.tasks {
		if t.DeviceID == deviceID {
			result = append(result, t)
		}
	}
	return result
}

// CleanupOldBackups 清理旧的备份文件
func (m *Manager) CleanupOldBackups(maxAge time.Duration) int {
	m.mu.RLock()
	defer m.mu.RUnlock()

	cleaned := 0
	cutoff := time.Now().Add(-maxAge)

	for _, task := range m.tasks {
		if task.BackupPath != "" && task.CompletedAt != nil {
			if task.CompletedAt.Before(cutoff) {
				if err := os.Remove(task.BackupPath); err == nil {
					cleaned++
				}
			}
		}
	}
	return cleaned
}

func (m *Manager) markFailed(taskID uuid.UUID, errMsg string) {
	m.mu.Lock()
	defer m.mu.Unlock()

	task, ok := m.tasks[taskID]
	if !ok {
		return
	}
	task.Status = StatusFailed
	task.ErrorMsg = errMsg
	now := time.Now().UTC()
	task.CompletedAt = &now
}