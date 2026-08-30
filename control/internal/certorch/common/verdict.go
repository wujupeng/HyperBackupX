package common

import "fmt"

type Verdict3 string

const (
	V3Pass      Verdict3 = "pass"
	V3Fail      Verdict3 = "fail"
	V3NotTested Verdict3 = "not_tested"
)

type Verdict4 string

const (
	V4Pass              Verdict4 = "pass"
	V4Fail              Verdict4 = "fail"
	V4NotSupported      Verdict4 = "not_supported"
	V4DifferentByDesign Verdict4 = "different_by_design"
)

type CertVerdict struct {
	Status          Verdict3 `json:"status"`
	EvidenceRef     string   `json:"evidence_ref"`
	RootCause       string   `json:"root_cause,omitempty"`
	NotTestedReason string   `json:"not_tested_reason,omitempty"`
}

type CertVerdict4 struct {
	Status             Verdict4 `json:"status"`
	EvidenceRef        string   `json:"evidence_ref"`
	RootCause          string   `json:"root_cause,omitempty"`
	DesignRationale    string   `json:"design_rationale,omitempty"`
	NotSupportedReason string   `json:"not_supported_reason,omitempty"`
}

func (v CertVerdict) IsPass() bool      { return v.Status == V3Pass }
func (v CertVerdict) IsFail() bool      { return v.Status == V3Fail }
func (v CertVerdict) IsNotTested() bool { return v.Status == V3NotTested }

func (v CertVerdict) Validate() error {
	if v.Status == V3Fail && v.RootCause == "" {
		return fmt.Errorf("%w: fail verdict requires root_cause", ErrVerdictIncomplete)
	}
	if v.Status == V3NotTested && v.NotTestedReason == "" {
		return fmt.Errorf("%w: not_tested verdict requires not_tested_reason", ErrVerdictIncomplete)
	}
	return nil
}

func (v CertVerdict4) Validate() error {
	if v.Status == V4Fail && v.RootCause == "" {
		return fmt.Errorf("%w: fail verdict requires root_cause", ErrVerdictIncomplete)
	}
	if v.Status == V4DifferentByDesign && v.DesignRationale == "" {
		return fmt.Errorf("%w: different_by_design verdict requires design_rationale", ErrVerdictIncomplete)
	}
	if v.Status == V4NotSupported && v.NotSupportedReason == "" {
		return fmt.Errorf("%w: not_supported verdict requires not_supported_reason", ErrVerdictIncomplete)
	}
	return nil
}
