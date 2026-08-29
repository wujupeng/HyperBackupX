package acceptance

import (
	"encoding/json"
	"fmt"
	"sync"
	"time"
)

type LineName string

const (
	LineFeatureCompat     LineName = "feature_compatibility"
	LineRestoreCompat     LineName = "restore_compatibility"
	LineDataIntegrity     LineName = "data_integrity"
	LineCrashRecovery     LineName = "crash_recovery"
	LineWinCompat         LineName = "win7_10_11_compatibility"
	LineRAMStability      LineName = "4gb_ram_stability"
)

var allLines = []LineName{
	LineFeatureCompat,
	LineRestoreCompat,
	LineDataIntegrity,
	LineCrashRecovery,
	LineWinCompat,
	LineRAMStability,
}

type LineStatus string

const (
	LineStatusPass    LineStatus = "pass"
	LineStatusFail    LineStatus = "fail"
	LineStatusPending LineStatus = "pending"
)

type EvidenceRef struct {
	ReportType string    `json:"report_type"`
	ReportID   string    `json:"report_id"`
	Summary    string    `json:"summary"`
}

type LineConclusion struct {
	Name        LineName     `json:"name"`
	DisplayName string       `json:"display_name"`
	Status      LineStatus   `json:"status"`
	PassRate    float64      `json:"pass_rate"`
	Evidence    []EvidenceRef `json:"evidence"`
	Detail      string       `json:"detail,omitempty"`
}

type SixLineConclusion struct {
	Lines       []LineConclusion `json:"lines"`
	AllPassed   bool             `json:"all_passed"`
	GeneratedAt time.Time        `json:"generated_at"`
}

type SignRecord struct {
	SignedBy  string    `json:"signed_by"`
	SignedAt  time.Time `json:"signed_at"`
	Comment   string    `json:"comment,omitempty"`
}

type CompatibilityReport struct {
	ID                string              `json:"id"`
	SixLineConclusion SixLineConclusion   `json:"six_line_conclusion"`
	SignRecord        *SignRecord         `json:"sign_record,omitempty"`
	CreatedAt         time.Time           `json:"created_at"`
}

type ReportGenerator struct {
	mu sync.RWMutex
}

func NewReportGenerator() *ReportGenerator {
	return &ReportGenerator{}
}

func (g *ReportGenerator) GenerateConclusion(
	featurePass, featureTotal int,
	restorePass, restoreTotal int,
	fuzzPass, fuzzTotal int,
	chaosPass, chaosTotal int,
	winPass, winTotal int,
	ramPass bool,
) *SixLineConclusion {
	g.mu.Lock()
	defer g.mu.Unlock()

	conclusion := &SixLineConclusion{
		GeneratedAt: time.Now(),
	}

	conclusion.Lines = append(conclusion.Lines, LineConclusion{
		Name:        LineFeatureCompat,
		DisplayName: "Feature Compatibility",
		Status:      judgeLine(featurePass, featureTotal),
		PassRate:    rate(featurePass, featureTotal),
		Evidence:    []EvidenceRef{{ReportType: "matrix", Summary: fmt.Sprintf("L1 matrix: %d/%d pass", featurePass, featureTotal)}},
	})

	conclusion.Lines = append(conclusion.Lines, LineConclusion{
		Name:        LineRestoreCompat,
		DisplayName: "Restore Compatibility",
		Status:      judgeLine(restorePass, restoreTotal),
		PassRate:    rate(restorePass, restoreTotal),
		Evidence:    []EvidenceRef{{ReportType: "dual_run", Summary: fmt.Sprintf("L5 dual-run: %d/%d pass", restorePass, restoreTotal)}},
	})

	conclusion.Lines = append(conclusion.Lines, LineConclusion{
		Name:        LineDataIntegrity,
		DisplayName: "Data Integrity",
		Status:      judgeLine(fuzzPass, fuzzTotal),
		PassRate:    rate(fuzzPass, fuzzTotal),
		Evidence:    []EvidenceRef{{ReportType: "fuzz", Summary: fmt.Sprintf("Fuzz verify: %d/%d pass", fuzzPass, fuzzTotal)}},
	})

	conclusion.Lines = append(conclusion.Lines, LineConclusion{
		Name:        LineCrashRecovery,
		DisplayName: "Crash Recovery",
		Status:      judgeLine(chaosPass, chaosTotal),
		PassRate:    rate(chaosPass, chaosTotal),
		Evidence:    []EvidenceRef{{ReportType: "chaos", Summary: fmt.Sprintf("Chaos detect+reject: %d/%d pass", chaosPass, chaosTotal)}},
	})

	conclusion.Lines = append(conclusion.Lines, LineConclusion{
		Name:        LineWinCompat,
		DisplayName: "Win7/10/11 Compatibility",
		Status:      judgeLine(winPass, winTotal),
		PassRate:    rate(winPass, winTotal),
		Evidence:    []EvidenceRef{{ReportType: "matrix", Summary: fmt.Sprintf("Win7/10/11 matrix: %d/%d pass", winPass, winTotal)}},
	})

	ramStatus := LineStatusFail
	if ramPass {
		ramStatus = LineStatusPass
	}
	conclusion.Lines = append(conclusion.Lines, LineConclusion{
		Name:        LineRAMStability,
		DisplayName: "4GB RAM Endpoint Stability",
		Status:      ramStatus,
		PassRate:    boolToRate(ramPass),
		Evidence:    []EvidenceRef{{ReportType: "acceptance", Summary: fmt.Sprintf("4GB RAM: memory peak <=80MB, no OOM: %v", ramPass)}},
	})

	conclusion.AllPassed = true
	for _, line := range conclusion.Lines {
		if line.Status != LineStatusPass {
			conclusion.AllPassed = false
			break
		}
	}

	return conclusion
}

func (g *ReportGenerator) GenerateReport(
	featurePass, featureTotal int,
	restorePass, restoreTotal int,
	fuzzPass, fuzzTotal int,
	chaosPass, chaosTotal int,
	winPass, winTotal int,
	ramPass bool,
) *CompatibilityReport {
	conclusion := g.GenerateConclusion(
		featurePass, featureTotal,
		restorePass, restoreTotal,
		fuzzPass, fuzzTotal,
		chaosPass, chaosTotal,
		winPass, winTotal,
		ramPass,
	)

	return &CompatibilityReport{
		ID:                fmt.Sprintf("report-%d", time.Now().UnixNano()),
		SixLineConclusion: *conclusion,
		CreatedAt:         time.Now(),
	}
}

func (g *ReportGenerator) MarshalReport(report *CompatibilityReport) ([]byte, error) {
	return json.Marshal(report)
}

func UnmarshalReport(data []byte) (*CompatibilityReport, error) {
	var report CompatibilityReport
	err := json.Unmarshal(data, &report)
	return &report, err
}

func AllLines() []LineName {
	return allLines
}

func judgeLine(pass, total int) LineStatus {
	if total == 0 {
		return LineStatusPending
	}
	if pass == total {
		return LineStatusPass
	}
	return LineStatusFail
}

func rate(pass, total int) float64 {
	if total == 0 {
		return 0.0
	}
	return float64(pass) / float64(total)
}

func boolToRate(b bool) float64 {
	if b {
		return 1.0
	}
	return 0.0
}