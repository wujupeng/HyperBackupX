package audit

import (
	"testing"
)

func TestSanitizeStringPassword(t *testing.T) {
	s := NewSanitizer()
	result := s.SanitizeString("user password=secret123 logged in")
	if containsStr(result, "secret123") {
		t.Fatal("Password should be redacted")
	}
}

func TestSanitizeStringApiKey(t *testing.T) {
	s := NewSanitizer()
	result := s.SanitizeString("config api_key=abc123def")
	if containsStr(result, "abc123def") {
		t.Fatal("API key should be redacted")
	}
}

func TestSanitizeStringPrivateKey(t *testing.T) {
	s := NewSanitizer()
	input := "key=-----BEGIN EC PRIVATE KEY-----\nMHcCAQEE\n-----END EC PRIVATE KEY-----"
	result := s.SanitizeString(input)
	if containsStr(result, "MHcCAQEE") {
		t.Fatal("Private key content should be redacted")
	}
}

func TestSanitizeStringConnectionString(t *testing.T) {
	s := NewSanitizer()
	result := s.SanitizeString("postgres://user:pass@localhost:5432/db")
	if containsStr(result, "user:pass@localhost") {
		t.Fatal("Connection string should be redacted")
	}
}

func TestSanitizeStringClean(t *testing.T) {
	s := NewSanitizer()
	input := "backup job completed, 1024 files, 512MB stored"
	result := s.SanitizeString(input)
	if result != input {
		t.Fatal("Clean string should not be modified")
	}
}

func TestSanitizeDetail(t *testing.T) {
	s := NewSanitizer()
	detail := map[string]interface{}{
		"status":   "completed",
		"config":   "password=mysecret",
		"nested":   map[string]interface{}{"token": "abc=secret"},
		"list":     []interface{}{"clean", "password=pass2"},
	}
	result := s.SanitizeDetail(detail)

	if result["status"] != "completed" {
		t.Fatal("Clean value should not change")
	}
	if result["config"] == "password=mysecret" {
		t.Fatal("Sensitive config should be redacted")
	}
}

func TestSanitizeDetailNil(t *testing.T) {
	s := NewSanitizer()
	if s.SanitizeDetail(nil) != nil {
		t.Fatal("Nil should return nil")
	}
}

func TestContainsSensitive(t *testing.T) {
	s := NewSanitizer()
	if !s.ContainsSensitive("password=secret") {
		t.Fatal("Should detect sensitive")
	}
	if s.ContainsSensitive("backup completed") {
		t.Fatal("Should not detect in clean text")
	}
}

func TestValidateEntry(t *testing.T) {
	s := NewSanitizer()
	entry := Entry{
		Action: "login",
		Detail: map[string]interface{}{
			"status": "ok",
			"config": "password=secret",
		},
	}
	violations := s.ValidateEntry(entry)
	if len(violations) != 1 {
		t.Fatalf("Expected 1 violation, got %d", len(violations))
	}
	if violations[0] != "detail.config" {
		t.Fatalf("Expected detail.config, got %s", violations[0])
	}
}

func TestSanitizeEntry(t *testing.T) {
	s := NewSanitizer()
	entry := Entry{
		Action: "config_update",
		Detail: map[string]interface{}{
			"config": "api_key=abc123",
		},
	}
	s.SanitizeEntry(&entry)
	configStr := entry.Detail["config"].(string)
	if containsStr(configStr, "abc123") {
		t.Fatal("Entry should be sanitized")
	}
}

func containsStr(s, substr string) bool {
	return len(s) >= len(substr) && indexOfStr(s, substr) >= 0
}

func indexOfStr(s, substr string) int {
	for i := 0; i <= len(s)-len(substr); i++ {
		if s[i:i+len(substr)] == substr {
			return i
		}
	}
	return -1
}