package log
package log

import (
	"bytes"
	"log/slog"
	"strings"
	"testing"
)

func newTestHandler() (*slog.Logger, *bytes.Buffer) {
	var buf bytes.Buffer
	handler := slog.NewTextHandler(&buf, &slog.HandlerOptions{Level: slog.LevelDebug})
	redacting := NewRedactingHandler(handler)
	return slog.New(redacting), &buf
}

func TestRedactSensitiveFields(t *testing.T) {
	logger, buf := newTestHandler()

	logger.Info("test", "password", "my-secret-password", "token", "my-jwt-token", "api_key", "my-api-key")

	output := buf.String()
	if strings.Contains(output, "my-secret-password") {
		t.Error("password should be redacted")
	}
	if strings.Contains(output, "my-jwt-token") {
		t.Error("token should be redacted")
	}
	if strings.Contains(output, "my-api-key") {
		t.Error("api_key should be redacted")
	}
	if !strings.Contains(output, "[REDACTED]") {
		t.Error("output should contain [REDACTED]")
	}
}

func TestRedactDSN(t *testing.T) {
	logger, buf := newTestHandler()

	logger.Info("connecting", "dsn", "postgres://hbx:password@localhost:5433/hbx")

	output := buf.String()
	if strings.Contains(output, "password") {
		t.Error("DSN password should be redacted")
	}
	if !strings.Contains(output, "[REDACTED]") {
		t.Error("DSN should be redacted")
	}
}

func TestRedactDSNInMessage(t *testing.T) {
	logger, buf := newTestHandler()

	logger.Info("connecting to postgres://hbx:password@localhost:5433/hbx")

	output := buf.String()
	if strings.Contains(output, "password") {
		t.Error("DSN password in message should be redacted")
	}
	if !strings.Contains(output, "postgres://[REDACTED]@") {
		t.Error("DSN in message should be redacted with pattern")
	}
}

func TestRedactBearerToken(t *testing.T) {
	logger, buf := newTestHandler()

	logger.Info("auth request", "header", "Bearer eyJhbGciOiJIUzI1NiJ9.signature")

	output := buf.String()
	if strings.Contains(output, "eyJhbGciOiJIUzI1NiJ9") {
		t.Error("Bearer token should be redacted")
	}
	if !strings.Contains(output, "Bearer [REDACTED]") {
		t.Error("Bearer should be redacted with pattern")
	}
}

func TestRedactPrivateKey(t *testing.T) {
	logger, buf := newTestHandler()

	logger.Info("key loaded", "key", "-----BEGIN RSA PRIVATE KEY-----\nMIIEpAIBAAKCAQEA...\n-----END RSA PRIVATE KEY-----")

	output := buf.String()
	if strings.Contains(output, "MIIEpAIBAAKCAQEA") {
		t.Error("Private key content should be redacted")
	}
	if !strings.Contains(output, "[REDACTED-KEY]") {
		t.Error("Private key should be redacted with pattern")
	}
}

func TestNonSensitiveFieldsNotRedacted(t *testing.T) {
	logger, buf := newTestHandler()

	logger.Info("test", "user_id", "12345", "action", "login", "status", "success")

	output := buf.String()
	if !strings.Contains(output, "12345") {
		t.Error("user_id should not be redacted")
	}
	if !strings.Contains(output, "login") {
		t.Error("action should not be redacted")
	}
	if !strings.Contains(output, "success") {
		t.Error("status should not be redacted")
	}
}