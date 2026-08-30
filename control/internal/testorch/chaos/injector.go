package chaos

import (
	"encoding/json"
	"fmt"
	"math/rand"
	"sync"
	"time"
)

type FaultType string

const (
	FaultUploadNetworkBreak  FaultType = "upload_network_break"
	FaultKillAgent           FaultType = "kill_agent"
	FaultWindowsRestart      FaultType = "windows_restart"
	FaultDeleteVolume        FaultType = "delete_volume"
	FaultModifyChunk         FaultType = "modify_chunk"
	FaultControlPlaneCrash   FaultType = "control_plane_crash"
	FaultStorageCrash        FaultType = "storage_crash"
	FaultDatabaseRestart     FaultType = "database_restart"
	FaultMachineReboot       FaultType = "machine_reboot"
	FaultRepositoryCorruption FaultType = "repository_corruption"
)

var allFaultTypes = []FaultType{
	FaultUploadNetworkBreak,
	FaultKillAgent,
	FaultWindowsRestart,
	FaultDeleteVolume,
	FaultModifyChunk,
	FaultControlPlaneCrash,
	FaultStorageCrash,
	FaultDatabaseRestart,
	FaultMachineReboot,
	FaultRepositoryCorruption,
}

type FaultConfig struct {
	Type       FaultType              `json:"type"`
	Target     string                 `json:"target"`
	Parameters map[string]interface{} `json:"parameters"`
	Seed       int64                  `json:"seed"`
}

type InjectedFault struct {
	Config    FaultConfig `json:"config"`
	InjectedAt time.Time  `json:"injected_at"`
	Detail    string      `json:"detail"`
}

type ChaosFaultInjector struct {
	mu     sync.RWMutex
	rand   *rand.Rand
	seed   int64
	faults []InjectedFault
}

func NewFaultInjector(seed int64) *ChaosFaultInjector {
	return &ChaosFaultInjector{
		rand: rand.New(rand.NewSource(seed)),
		seed: seed,
	}
}

func (i *ChaosFaultInjector) Inject(ft FaultType, target string) (*InjectedFault, error) {
	i.mu.Lock()
	defer i.mu.Unlock()

	if !validateFaultType(ft) {
		return nil, fmt.Errorf("unknown fault type: %s", ft)
	}

	params := i.generateFaultParams(ft)
	config := FaultConfig{
		Type:       ft,
		Target:     target,
		Parameters: params,
		Seed:       i.seed,
	}

	fault := &InjectedFault{
		Config:    config,
		InjectedAt: time.Now(),
		Detail:    i.describeFault(ft, target, params),
	}

	i.faults = append(i.faults, *fault)
	return fault, nil
}

func (i *ChaosFaultInjector) generateFaultParams(ft FaultType) map[string]interface{} {
	switch ft {
	case FaultUploadNetworkBreak:
		return map[string]interface{}{
			"duration_ms":   i.rand.Intn(60000) + 5000,
			"target_host":   fmt.Sprintf("storage.%d.example.com", i.rand.Intn(10)),
			"break_point":   []string{"before_upload", "during_upload", "after_upload"}[i.rand.Intn(3)],
		}
	case FaultKillAgent:
		return map[string]interface{}{
			"signal":     []string{"SIGTERM", "SIGKILL"}[i.rand.Intn(2)],
			"delay_ms":   i.rand.Intn(10000),
			"restart":    i.rand.Intn(2) == 1,
		}
	case FaultWindowsRestart:
		return map[string]interface{}{
			"force":      i.rand.Intn(2) == 1,
			"timeout_ms": i.rand.Intn(30000) + 5000,
			"phase":      []string{"backup", "restore", "verify"}[i.rand.Intn(3)],
		}
	case FaultDeleteVolume:
		return map[string]interface{}{
			"volume_id": fmt.Sprintf("vol-%08d", i.rand.Intn(1000000)),
			"force":     i.rand.Intn(2) == 1,
		}
	case FaultModifyChunk:
		return map[string]interface{}{
			"chunk_index": i.rand.Intn(256),
			"modify_type": []string{"flip_bit", "zero_fill", "random_fill"}[i.rand.Intn(3)],
			"offset":      i.rand.Intn(4096),
		}
	case FaultControlPlaneCrash:
		return map[string]interface{}{
			"signal":   "SIGKILL",
			"delay_ms": i.rand.Intn(5000),
		}
	case FaultStorageCrash:
		return map[string]interface{}{
			"signal":   "SIGKILL",
			"delay_ms": i.rand.Intn(5000),
		}
	case FaultDatabaseRestart:
		return map[string]interface{}{
			"command":  "pg_ctl restart",
			"delay_ms": i.rand.Intn(3000),
		}
	case FaultMachineReboot:
		return map[string]interface{}{
			"command":  []string{"reboot", "shutdown /r"}[i.rand.Intn(2)],
			"force":    i.rand.Intn(2) == 1,
			"delay_ms": i.rand.Intn(10000),
		}
	case FaultRepositoryCorruption:
		return map[string]interface{}{
			"corruption_type": []string{"chunk_corrupt", "manifest_delete", "metadata_damage"}[i.rand.Intn(3)],
			"chunk_index":     i.rand.Intn(256),
		}
	default:
		return map[string]interface{}{}
	}
}

func (i *ChaosFaultInjector) describeFault(ft FaultType, target string, params map[string]interface{}) string {
	switch ft {
	case FaultUploadNetworkBreak:
		return fmt.Sprintf("network break injected on %s", target)
	case FaultKillAgent:
		return fmt.Sprintf("agent process killed on %s", target)
	case FaultWindowsRestart:
		return fmt.Sprintf("Windows restart triggered on %s", target)
	case FaultDeleteVolume:
		return fmt.Sprintf("volume deleted on %s", target)
	case FaultModifyChunk:
		return fmt.Sprintf("chunk modified on %s", target)
	case FaultControlPlaneCrash:
		return fmt.Sprintf("control plane crash on %s", target)
	case FaultStorageCrash:
		return fmt.Sprintf("storage crash on %s", target)
	case FaultDatabaseRestart:
		return fmt.Sprintf("database restart on %s", target)
	case FaultMachineReboot:
		return fmt.Sprintf("machine reboot on %s", target)
	case FaultRepositoryCorruption:
		return fmt.Sprintf("repository corruption on %s", target)
	default:
		return fmt.Sprintf("unknown fault on %s", target)
	}
}

func (i *ChaosFaultInjector) Seed() int64 {
	return i.seed
}

func (i *ChaosFaultInjector) Reset() {
	i.mu.Lock()
	defer i.mu.Unlock()
	i.rand = rand.New(rand.NewSource(i.seed))
	i.faults = nil
}

func (i *ChaosFaultInjector) Faults() []InjectedFault {
	i.mu.RLock()
	defer i.mu.RUnlock()
	result := make([]InjectedFault, len(i.faults))
	copy(result, i.faults)
	return result
}

func (i *ChaosFaultInjector) InjectAll(target string) ([]InjectedFault, error) {
	var faults []InjectedFault
	for _, ft := range allFaultTypes {
		f, err := i.Inject(ft, target)
		if err != nil {
			return nil, fmt.Errorf("inject %s: %w", ft, err)
		}
		faults = append(faults, *f)
	}
	return faults, nil
}

func (i *ChaosFaultInjector) MarshalFaults(faults []InjectedFault) ([]byte, error) {
	return json.Marshal(faults)
}

func UnmarshalFaults(data []byte) ([]InjectedFault, error) {
	var faults []InjectedFault
	err := json.Unmarshal(data, &faults)
	return faults, err
}

func AllFaultTypes() []FaultType {
	return allFaultTypes
}

func validateFaultType(ft FaultType) bool {
	for _, valid := range allFaultTypes {
		if ft == valid {
			return true
		}
	}
	return false
}

func ValidateFaultType(ft FaultType) bool {
	return validateFaultType(ft)
}