package fuzz

import (
	"fmt"
	"sync"
	"time"
)

type CrashType string

const (
	CrashNetworkBreak CrashType = "network_break"
	CrashProcessKill  CrashType = "process_kill"
	CrashDiskFull     CrashType = "disk_full"
)

type EnvironmentState string

const (
	EnvStateReady    EnvironmentState = "ready"
	EnvStateCrashed  EnvironmentState = "crashed"
	EnvStateRestarted EnvironmentState = "restarted"
	EnvStateResumed  EnvironmentState = "resumed"
	EnvStateCleaned  EnvironmentState = "cleaned"
)

type CrashConfig struct {
	Type       CrashType `json:"type"`
	DurationMs int       `json:"duration_ms"`
	TargetPath string    `json:"target_path,omitempty"`
}

type EnvironmentController struct {
	mu     sync.RWMutex
	state  EnvironmentState
	events []EnvironmentEvent
}

type EnvironmentEvent struct {
	Timestamp time.Time   `json:"timestamp"`
	Action    string      `json:"action"`
	State     EnvironmentState `json:"state"`
	Detail    string      `json:"detail,omitempty"`
}

func NewEnvironmentController() *EnvironmentController {
	return &EnvironmentController{
		state: EnvStateReady,
	}
}

func (ec *EnvironmentController) State() EnvironmentState {
	ec.mu.RLock()
	defer ec.mu.RUnlock()
	return ec.state
}

func (ec *EnvironmentController) InjectCrash(config CrashConfig) error {
	ec.mu.Lock()
	defer ec.mu.Unlock()

	switch config.Type {
	case CrashNetworkBreak:
		ec.state = EnvStateCrashed
		ec.recordEvent("inject_crash", fmt.Sprintf("network break for %dms", config.DurationMs))
	case CrashProcessKill:
		ec.state = EnvStateCrashed
		ec.recordEvent("inject_crash", "process killed")
	case CrashDiskFull:
		ec.state = EnvStateCrashed
		ec.recordEvent("inject_crash", fmt.Sprintf("disk full at %s", config.TargetPath))
	default:
		return fmt.Errorf("unknown crash type: %s", config.Type)
	}
	return nil
}

func (ec *EnvironmentController) Restart() error {
	ec.mu.Lock()
	defer ec.mu.Unlock()

	if ec.state != EnvStateCrashed {
		return fmt.Errorf("cannot restart from state: %s", ec.state)
	}
	ec.state = EnvStateRestarted
	ec.recordEvent("restart", "environment restarted")
	return nil
}

func (ec *EnvironmentController) Resume() error {
	ec.mu.Lock()
	defer ec.mu.Unlock()

	if ec.state != EnvStateRestarted {
		return fmt.Errorf("cannot resume from state: %s", ec.state)
	}
	ec.state = EnvStateResumed
	ec.recordEvent("resume", "resumed from journal checkpoint")
	return nil
}

func (ec *EnvironmentController) Cleanup() error {
	ec.mu.Lock()
	defer ec.mu.Unlock()

	ec.state = EnvStateCleaned
	ec.recordEvent("cleanup", "environment cleaned")
	return nil
}

func (ec *EnvironmentController) Reset() {
	ec.mu.Lock()
	defer ec.mu.Unlock()
	ec.state = EnvStateReady
	ec.events = nil
}

func (ec *EnvironmentController) Events() []EnvironmentEvent {
	ec.mu.RLock()
	defer ec.mu.RUnlock()
	result := make([]EnvironmentEvent, len(ec.events))
	copy(result, ec.events)
	return result
}

func (ec *EnvironmentController) recordEvent(action, detail string) {
	ec.events = append(ec.events, EnvironmentEvent{
		Timestamp: time.Now(),
		Action:    action,
		State:     ec.state,
		Detail:    detail,
	})
}

func (ec *EnvironmentController) SimulateFullCycle(config CrashConfig) error {
	if err := ec.InjectCrash(config); err != nil {
		return fmt.Errorf("inject crash: %w", err)
	}
	if err := ec.Restart(); err != nil {
		return fmt.Errorf("restart: %w", err)
	}
	if err := ec.Resume(); err != nil {
		return fmt.Errorf("resume: %w", err)
	}
	if err := ec.Cleanup(); err != nil {
		return fmt.Errorf("cleanup: %w", err)
	}
	return nil
}