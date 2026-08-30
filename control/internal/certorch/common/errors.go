package common

import "errors"

var (
	ErrVerdictIncomplete    = errors.New("certorch: verdict incomplete, required field missing")
	ErrNotFrozen            = errors.New("certorch: target not frozen yet")
	ErrFrozenImmutable      = errors.New("certorch: frozen target is immutable")
	ErrSoakNotPassed        = errors.New("certorch: soak test did not pass, cannot freeze")
	ErrNotG17Context        = errors.New("certorch: freeze only allowed in G17 context")
	ErrLeakDetected         = errors.New("certorch: secret leak detected in report content")
	ErrSessionNotFound      = errors.New("certorch: certification session not found")
	ErrGateNotFound         = errors.New("certorch: gate runner not registered")
	ErrSessionAlreadyActive = errors.New("certorch: session already active for this gate")
)