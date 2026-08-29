package acceptance

import (
	"fmt"
	"sync"
	"time"
)

type SignError struct {
	FailedLines []LineName `json:"failed_lines"`
	Message     string     `json:"message"`
}

func (e *SignError) Error() string {
	return e.Message
}

type SignGate struct {
	mu sync.RWMutex
}

func NewSignGate() *SignGate {
	return &SignGate{}
}

func (g *SignGate) Sign(report *CompatibilityReport, signedBy, comment string) (*CompatibilityReport, error) {
	g.mu.Lock()
	defer g.mu.Unlock()

	if report == nil {
		return nil, fmt.Errorf("report is nil")
	}

	var failedLines []LineName
	for _, line := range report.SixLineConclusion.Lines {
		if line.Status != LineStatusPass {
			failedLines = append(failedLines, line.Name)
		}
	}

	if len(failedLines) > 0 {
		return nil, &SignError{
			FailedLines: failedLines,
			Message:     fmt.Sprintf("cannot sign: %d line(s) not passed: %v", len(failedLines), failedLines),
		}
	}

	report.SignRecord = &SignRecord{
		SignedBy:  signedBy,
		SignedAt:  time.Now(),
		Comment:   comment,
	}

	return report, nil
}

func (g *SignGate) CanSign(conclusion *SixLineConclusion) (bool, []LineName) {
	var failedLines []LineName
	for _, line := range conclusion.Lines {
		if line.Status != LineStatusPass {
			failedLines = append(failedLines, line.Name)
		}
	}
	return len(failedLines) == 0, failedLines
}

func (g *SignGate) CheckAllLines(conclusion *SixLineConclusion) map[LineName]bool {
	result := make(map[LineName]bool)
	for _, line := range conclusion.Lines {
		result[line.Name] = line.Status == LineStatusPass
	}
	return result
}