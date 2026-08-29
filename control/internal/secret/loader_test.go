package main
package secret

import (
	"os"
	"testing"
)

func setTestEnv(t *testing.T, key, val string) {
	t.Helper()
	old := os.Getenv(key)
	os.Setenv(key, val)
	t.Cleanup(func() { os.Setenv(key, old) })
}

func TestLoadFromEnvironment(t *testing.T) {
	setTestEnv(t, "HBX_JWT_SECRET", "this-is-a-very-strong-jwt-secret-32+bytes!")
	setTestEnv(t, "HBX_DB_PASSWORD", "this-is-a-strong-db-password-24+bytes!")
	setTestEnv(t, "HBX_ADMIN_PASSWORD", "strong-admin-pwd-16+bytes!")
	setTestEnv(t, "HBX_AGENT_TOKEN_PEPPER", "this-is-a-strong-agent-pepper-32+bytes!")
	setTestEnv(t, "HBX_ENV_FILE", "/nonexistent/path/to/skip/file")

	loader := NewSecretLoader()
	bundle, err := loader.Load()
	if err != nil {
		t.Fatalf("Load should succeed with all env vars set: %v", err)
	}
	if len(bundle.JWTSecret) == 0 {
		t.Fatal("JWTSecret should not be empty")
	}
	defer bundle.Zeroize()
}

func TestLoadMissingSecretFails(t *testing.T) {
	setTestEnv(t, "HBX_JWT_SECRET", "")
	setTestEnv(t, "HBX_DB_PASSWORD", "")
	setTestEnv(t, "HBX_ADMIN_PASSWORD", "")
	setTestEnv(t, "HBX_AGENT_TOKEN_PEPPER", "")
	setTestEnv(t, "HBX_ENV_FILE", "/nonexistent/path/to/skip/file")

	loader := NewSecretLoader()
	_, err := loader.Load()
	if err != ErrSecretNotConfigured {
		t.Fatalf("Load should fail with ErrSecretNotConfigured, got: %v", err)
	}
}

func TestLoadPartialSecretFails(t *testing.T) {
	setTestEnv(t, "HBX_JWT_SECRET", "some-secret")
	setTestEnv(t, "HBX_DB_PASSWORD", "")
	setTestEnv(t, "HBX_ADMIN_PASSWORD", "")
	setTestEnv(t, "HBX_AGENT_TOKEN_PEPPER", "")
	setTestEnv(t, "HBX_ENV_FILE", "/nonexistent/path/to/skip/file")

	loader := NewSecretLoader()
	_, err := loader.Load()
	if err != ErrSecretNotConfigured {
		t.Fatalf("Load should fail with partial secrets, got: %v", err)
	}
}

func TestValidateStrength(t *testing.T) {
	tests := []struct {
		name    string
		bundle  *SecretBundle
		wantErr bool
	}{
		{
			name: "all strong",
			bundle: &SecretBundle{
				JWTSecret:        ZeroizingKey(make([]byte, 32)),
				DBPassword:       ZeroizingKey(make([]byte, 24)),
				AdminPassword:    ZeroizingKey(make([]byte, 16)),
				AgentTokenPepper: ZeroizingKey(make([]byte, 32)),
			},
			wantErr: false,
		},
		{
			name: "jwt too short",
			bundle: &SecretBundle{
				JWTSecret:        ZeroizingKey(make([]byte, 31)),
				DBPassword:       ZeroizingKey(make([]byte, 24)),
				AdminPassword:    ZeroizingKey(make([]byte, 16)),
				AgentTokenPepper: ZeroizingKey(make([]byte, 32)),
			},
			wantErr: true,
		},
		{
			name: "db too short",
			bundle: &SecretBundle{
				JWTSecret:        ZeroizingKey(make([]byte, 32)),
				DBPassword:       ZeroizingKey(make([]byte, 23)),
				AdminPassword:    ZeroizingKey(make([]byte, 16)),
				AgentTokenPepper: ZeroizingKey(make([]byte, 32)),
			},
			wantErr: true,
		},
		{
			name: "admin too short",
			bundle: &SecretBundle{
				JWTSecret:        ZeroizingKey(make([]byte, 32)),
				DBPassword:       ZeroizingKey(make([]byte, 24)),
				AdminPassword:    ZeroizingKey(make([]byte, 15)),
				AgentTokenPepper: ZeroizingKey(make([]byte, 32)),
			},
			wantErr: true,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			err := ValidateStrength(tt.bundle)
			if (err != nil) != tt.wantErr {
				t.Errorf("ValidateStrength() error = %v, wantErr %v", err, tt.wantErr)
			}
		})
	}
}

func TestZeroizingKey(t *testing.T) {
	key := ZeroizingKey([]byte("sensitive-data"))
	if key.String() != "[REDACTED]" {
		t.Errorf("String() should return [REDACTED], got %s", key.String())
	}
	key.Zeroize()
	for i, b := range key {
		if b != 0 {
			t.Errorf("byte at index %d should be zero after Zeroize()", i)
		}
	}
}

func TestGenerateStrongSecret(t *testing.T) {
	s, err := GenerateStrongSecret(48)
	if err != nil {
		t.Fatalf("GenerateStrongSecret failed: %v", err)
	}
	if len(s) < 48 {
		t.Errorf("generated secret too short: %d", len(s))
	}
}