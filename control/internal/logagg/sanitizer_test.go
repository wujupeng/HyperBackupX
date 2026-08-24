package logagg

import (
	"testing"
)

func TestSanitizePassword(t *testing.T) {
	s := NewSanitizer()
	input := "user login with password=secret123"
	output := s.Sanitize(input)
	if output == input {
		t.Fatal("Password should be redacted")
	}
	if contains(output, "secret123") {
		t.Fatal("Password value should not appear in output")
	}
}

func TestSanitizeApiKey(t *testing.T) {
	s := NewSanitizer()
	input := "config api_key=abc123def456"
	output := s.Sanitize(input)
	if contains(output, "abc123def456") {
		t.Fatal("API key should be redacted")
	}
}

func TestSanitizePrivateKey(t *testing.T) {
	s := NewSanitizer()
	input := "-----BEGIN EC PRIVATE KEY-----\nMHcCAQEE\n-----END EC PRIVATE KEY-----"
	output := s.Sanitize(input)
	if output == input {
		t.Fatal("Private key should be redacted")
	}
}

func TestSanitizeBearerToken(t *testing.T) {
	s := NewSanitizer()
	input := "Authorization: Bearer eyJhbGciOiJIUzI1NiJ9"
	output := s.Sanitize(input)
	if contains(output, "eyJhbGciOiJIUzI1NiJ9") {
		t.Fatal("Bearer token should be redacted")
	}
}

func TestSanitizeConnectionString(t *testing.T) {
	s := NewSanitizer()
	input := "postgres://user:pass@localhost:5432/db"
	output := s.Sanitize(input)
	if contains(output, "user:pass@localhost") {
		t.Fatal("Connection string should be redacted")
	}
}

func TestSanitizeNoMatch(t *testing.T) {
	s := NewSanitizer()
	input := "backup completed successfully, 1024 files processed"
	output := s.Sanitize(input)
	if output != input {
		t.Fatal("Clean log should not be modified")
	}
}

func TestSanitizeMap(t *testing.T) {
	s := NewSanitizer()
	m := map[string]string{
		"status": "ok",
		"config": "password=mysecret",
	}
	result := s.SanitizeMap(m)
	if result["config"] == m["config"] {
		t.Fatal("Sensitive value should be redacted")
	}
	if result["status"] != "ok" {
		t.Fatal("Clean value should not change")
	}
}

func TestContainsSensitive(t *testing.T) {
	s := NewSanitizer()
	if !s.ContainsSensitive("password=secret") {
		t.Fatal("Should detect sensitive data")
	}
	if s.ContainsSensitive("backup completed") {
		t.Fatal("Should not detect sensitive in clean log")
	}
}

func TestValidateLogEntry(t *testing.T) {
	s := NewSanitizer()
	violations := s.ValidateLogEntry("login with password=secret", nil)
	if len(violations) != 1 {
		t.Fatalf("Expected 1 violation, got %d", len(violations))
	}

	violations = s.ValidateLogEntry("backup ok", map[string]string{
		"config": "api_key=abc123",
	})
	if len(violations) != 1 {
		t.Fatalf("Expected 1 violation, got %d", len(violations))
	}

	violations = s.ValidateLogEntry("backup ok", map[string]string{
		"status": "completed",
	})
	if len(violations) != 0 {
		t.Fatalf("Expected 0 violations, got %d", len(violations))
	}
}

func contains(s, substr string) bool {
	return len(s) >= len(substr) && (indexOf(s, substr) >= 0)
}

func indexOf(s, substr string) int {
	for i := 0; i <= len(s)-len(substr); i++ {
		if s[i:i+len(substr)] == substr {
			return i
		}
	}
	return -1
}