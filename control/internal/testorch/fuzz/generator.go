package fuzz

import (
	"encoding/json"
	"fmt"
	"math/rand"
	"sync"

)

type PerturbationType string

const (
	PerturbFileContent  PerturbationType = "file_content"
	PerturbDirectory    PerturbationType = "directory"
	PerturbPermission   PerturbationType = "permission"
	PerturbFilename     PerturbationType = "filename"
	PerturbFileSize     PerturbationType = "file_size"
	PerturbFileDelete   PerturbationType = "file_delete"
	PerturbFileModify   PerturbationType = "file_modify"
	PerturbNetworkBreak PerturbationType = "network_break"
	PerturbProcessKill  PerturbationType = "process_kill"
	PerturbDiskFull     PerturbationType = "disk_full"
)

var allPerturbationTypes = []PerturbationType{
	PerturbFileContent,
	PerturbDirectory,
	PerturbPermission,
	PerturbFilename,
	PerturbFileSize,
	PerturbFileDelete,
	PerturbFileModify,
	PerturbNetworkBreak,
	PerturbProcessKill,
	PerturbDiskFull,
}

type Perturbation struct {
	Type       PerturbationType     `json:"type"`
	TargetPath string               `json:"target_path,omitempty"`
	Parameters map[string]interface{} `json:"parameters,omitempty"`
	Sequence   int                  `json:"sequence"`
}

type ScenarioConfig struct {
	Name           string        `json:"name"`
	Seed           int64         `json:"seed"`
	Iterations     int           `json:"iterations"`
	PerturbTypes   []PerturbationType `json:"perturb_types"`
	PipelineStages []string      `json:"pipeline_stages"`
}

type FuzzPerturbationGenerator struct {
	mu   sync.RWMutex
	rand *rand.Rand
	seed int64
}

func NewPerturbationGenerator(seed int64) *FuzzPerturbationGenerator {
	return &FuzzPerturbationGenerator{
		rand: rand.New(rand.NewSource(seed)),
		seed: seed,
	}
}

func (g *FuzzPerturbationGenerator) Generate(count int, types []PerturbationType) []Perturbation {
	g.mu.Lock()
	defer g.mu.Unlock()

	if len(types) == 0 {
		types = allPerturbationTypes
	}

	result := make([]Perturbation, count)
	for i := 0; i < count; i++ {
		pt := types[g.rand.Intn(len(types))]
		result[i] = Perturbation{
			Type:       pt,
			Sequence:   i,
			Parameters: g.generateParams(pt),
		}
	}
	return result
}

func (g *FuzzPerturbationGenerator) generateParams(pt PerturbationType) map[string]interface{} {
	switch pt {
	case PerturbFileContent:
		return map[string]interface{}{
			"offset":     g.rand.Intn(4096),
			"length":     g.rand.Intn(1024) + 1,
			"fill_byte":  g.rand.Intn(256),
		}
	case PerturbDirectory:
		return map[string]interface{}{
			"action":     []string{"create", "delete", "rename"}[g.rand.Intn(3)],
			"depth":      g.rand.Intn(5) + 1,
		}
	case PerturbPermission:
		return map[string]interface{}{
			"mode": []string{"0000", "0444", "0666", "0777"}[g.rand.Intn(4)],
		}
	case PerturbFilename:
		return map[string]interface{}{
			"new_name": fmt.Sprintf("fuzz_renamed_%d", g.rand.Intn(100000)),
		}
	case PerturbFileSize:
		return map[string]interface{}{
			"action":  []string{"truncate", "extend"}[g.rand.Intn(2)],
			"new_size": g.rand.Intn(10485760),
		}
	case PerturbFileDelete:
		return map[string]interface{}{
			"recursive": g.rand.Intn(2) == 1,
		}
	case PerturbFileModify:
		return map[string]interface{}{
			"positions": g.rand.Intn(100) + 1,
			"byte_range": g.rand.Intn(256),
		}
	case PerturbNetworkBreak:
		return map[string]interface{}{
			"duration_ms":   g.rand.Intn(30000) + 1000,
			"target_host":   fmt.Sprintf("10.0.0.%d", g.rand.Intn(254)+1),
		}
	case PerturbProcessKill:
		return map[string]interface{}{
			"signal":  []string{"SIGTERM", "SIGKILL"}[g.rand.Intn(2)],
			"delay_ms": g.rand.Intn(5000),
		}
	case PerturbDiskFull:
		return map[string]interface{}{
			"fill_percent": g.rand.Intn(100) + 1,
			"target_path":  "/tmp",
		}
	default:
		return map[string]interface{}{}
	}
}

func (g *FuzzPerturbationGenerator) Seed() int64 {
	return g.seed
}

func (g *FuzzPerturbationGenerator) Reset() {
	g.mu.Lock()
	defer g.mu.Unlock()
	g.rand = rand.New(rand.NewSource(g.seed))
}

func (g *FuzzPerturbationGenerator) GenerateScenario(config ScenarioConfig) ([]Perturbation, error) {
	if config.Iterations <= 0 {
		return nil, fmt.Errorf("iterations must be positive")
	}
	gen := NewPerturbationGenerator(config.Seed)
	return gen.Generate(config.Iterations, config.PerturbTypes), nil
}

func (g *FuzzPerturbationGenerator) MarshalPerturbations(perts []Perturbation) ([]byte, error) {
	return json.Marshal(perts)
}

func UnmarshalPerturbations(data []byte) ([]Perturbation, error) {
	var perts []Perturbation
	err := json.Unmarshal(data, &perts)
	return perts, err
}

func AllPerturbationTypes() []PerturbationType {
	return allPerturbationTypes
}

func ValidatePerturbationType(pt PerturbationType) bool {
	for _, valid := range allPerturbationTypes {
		if pt == valid {
			return true
		}
	}
	return false
}