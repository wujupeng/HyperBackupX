package testorch

import "errors"

var (
	ErrMatrixNotFound = errors.New("compatibility matrix not found")
	ErrEntryNotFound  = errors.New("matrix entry not found")
	ErrCaseNotFound   = errors.New("test case not found")
	ErrDualRunNotFound = errors.New("dual run not found")
)