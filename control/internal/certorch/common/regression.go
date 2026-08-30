package common

import (
	"context"
	"os/exec"
	"strings"
)

type RegResult struct {
	Total       int      `json:"total"`
	Passed      int      `json:"passed"`
	Failed      int      `json:"failed"`
	FailedTests []string `json:"failed_tests,omitempty"`
}

type RegressionRunner struct {
	goTestDir string
	cargoDir  string
}

func NewRegressionRunner(goTestDir, cargoDir string) *RegressionRunner {
	return &RegressionRunner{goTestDir: goTestDir, cargoDir: cargoDir}
}

func (r *RegressionRunner) RunRegression(ctx context.Context) (RegResult, error) {
	result := RegResult{}

	goRes, err := r.runGoTests(ctx)
	if err != nil {
		return RegResult{}, err
	}
	result.Total += goRes.Total
	result.Passed += goRes.Passed
	result.Failed += goRes.Failed
	result.FailedTests = append(result.FailedTests, goRes.FailedTests...)

	cargoRes, err := r.runCargoTests(ctx)
	if err != nil {
		return RegResult{}, err
	}
	result.Total += cargoRes.Total
	result.Passed += cargoRes.Passed
	result.Failed += cargoRes.Failed
	result.FailedTests = append(result.FailedTests, cargoRes.FailedTests...)

	return result, nil
}

func (r *RegressionRunner) runGoTests(ctx context.Context) (RegResult, error) {
	cmd := exec.CommandContext(ctx, "go", "test", "./...", "-count=1")
	if r.goTestDir != "" {
		cmd.Dir = r.goTestDir
	}
	output, _ := cmd.CombinedOutput()
	return parseGoTestOutput(string(output)), nil
}

func (r *RegressionRunner) runCargoTests(ctx context.Context) (RegResult, error) {
	cmd := exec.CommandContext(ctx, "cargo", "test")
	if r.cargoDir != "" {
		cmd.Dir = r.cargoDir
	}
	output, _ := cmd.CombinedOutput()
	return parseCargoTestOutput(string(output)), nil
}

func parseGoTestOutput(output string) RegResult {
	lines := strings.Split(output, "\n")
	result := RegResult{}
	for _, line := range lines {
		line = strings.TrimSpace(line)
		if strings.HasPrefix(line, "ok") {
			result.Passed++
		} else if strings.HasPrefix(line, "FAIL") {
			result.Failed++
			if parts := strings.Fields(line); len(parts) > 1 {
				result.FailedTests = append(result.FailedTests, parts[1])
			}
		}
	}
	result.Total = result.Passed + result.Failed
	return result
}

func parseCargoTestOutput(output string) RegResult {
	lines := strings.Split(output, "\n")
	result := RegResult{}
	for _, line := range lines {
		line = strings.TrimSpace(line)
		if strings.Contains(line, "test result: ok") {
			if parts := strings.Fields(line); len(parts) >= 4 {
				result.Passed++
			}
		} else if strings.Contains(line, "test result: FAILED") {
			result.Failed++
		}
	}
	result.Total = result.Passed + result.Failed
	return result
}
