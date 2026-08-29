package compat

import "errors"

var (
	ErrRepoNotFound = errors.New("compat repository not found")
	ErrJobNotFound  = errors.New("compat job not found")
	ErrInvalidState = errors.New("invalid execution state transition")
)