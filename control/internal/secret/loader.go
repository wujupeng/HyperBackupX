package main
package secret

import (
	"bufio"
	"crypto/rand"
	"encoding/base64"
	"fmt"
	"os"
	"strings"
)

var (
	ErrSecretNotConfigured = fmt.Errorf("密钥未配置，请通过 Secret 管理通道注入 (环境变量 / Vault / EnvironmentFile)")
	ErrSecretTooWeak       = fmt.Errorf("密钥强度不足，不满足最小要求")
)

type ZeroizingKey []byte

func (k ZeroizingKey) Zeroize() {
	for i := range k {
		k[i] = 0
	}
}

func (k ZeroizingKey) Bytes() []byte {
	return []byte(k)
}

func (k ZeroizingKey) String() string {
	return "[REDACTED]"
}

type SecretBundle struct {
	JWTSecret       ZeroizingKey
	DBPassword      ZeroizingKey
	AdminPassword   ZeroizingKey
	AgentTokenPepper ZeroizingKey
}

func (b *SecretBundle) Zeroize() {
	b.JWTSecret.Zeroize()
	b.DBPassword.Zeroize()
	b.AdminPassword.Zeroize()
	b.AgentTokenPepper.Zeroize()
}

type SecretLoader struct {
	envFile string
}

func NewSecretLoader() *SecretLoader {
	envFile := os.Getenv("HBX_ENV_FILE")
	if envFile == "" {
		envFile = "/etc/hbx/control.env"
	}
	return &SecretLoader{envFile: envFile}
}

func (l *SecretLoader) Load() (*SecretBundle, error) {
	envVars := l.loadEnvironmentFile()

	bundle := &SecretBundle{
		JWTSecret:        ZeroizingKey(l.getSecret("HBX_JWT_SECRET", envVars)),
		DBPassword:       ZeroizingKey(l.getSecret("HBX_DB_PASSWORD", envVars)),
		AdminPassword:    ZeroizingKey(l.getSecret("HBX_ADMIN_PASSWORD", envVars)),
		AgentTokenPepper: ZeroizingKey(l.getSecret("HBX_AGENT_TOKEN_PEPPER", envVars)),
	}

	if len(bundle.JWTSecret) == 0 || len(bundle.DBPassword) == 0 ||
		len(bundle.AdminPassword) == 0 || len(bundle.AgentTokenPepper) == 0 {
		bundle.Zeroize()
		return nil, ErrSecretNotConfigured
	}

	return bundle, nil
}

func (l *SecretLoader) getSecret(key string, envVars map[string]string) string {
	if v := os.Getenv(key); v != "" {
		return v
	}
	if v, ok := envVars[key]; ok {
		return v
	}
	return ""
}

func (l *SecretLoader) loadEnvironmentFile() map[string]string {
	result := make(map[string]string)

	f, err := os.Open(l.envFile)
	if err != nil {
		return result
	}
	defer f.Close()

	scanner := bufio.NewScanner(f)
	for scanner.Scan() {
		line := strings.TrimSpace(scanner.Text())
		if line == "" || strings.HasPrefix(line, "#") {
			continue
		}
		if idx := strings.Index(line, "="); idx > 0 {
			key := strings.TrimSpace(line[:idx])
			val := strings.TrimSpace(line[idx+1:])
			val = strings.Trim(val, `"'`)
			result[key] = val
		}
	}

	return result
}

func ValidateStrength(bundle *SecretBundle) error {
	if len(bundle.JWTSecret) < 32 {
		return fmt.Errorf("%w: JWT Secret 需要 ≥32 字节，当前 %d 字节", ErrSecretTooWeak, len(bundle.JWTSecret))
	}
	if len(bundle.DBPassword) < 24 {
		return fmt.Errorf("%w: DB 密码需要 ≥24 字节，当前 %d 字节", ErrSecretTooWeak, len(bundle.DBPassword))
	}
	if len(bundle.AdminPassword) < 16 {
		return fmt.Errorf("%w: Admin 密码需要 ≥16 字节，当前 %d 字节", ErrSecretTooWeak, len(bundle.AdminPassword))
	}
	if len(bundle.AgentTokenPepper) < 32 {
		return fmt.Errorf("%w: Agent Token Pepper 需要 ≥32 字节，当前 %d 字节", ErrSecretTooWeak, len(bundle.AgentTokenPepper))
	}
	return nil
}

func GenerateStrongSecret(numBytes int) (string, error) {
	b := make([]byte, numBytes)
	if _, err := rand.Read(b); err != nil {
		return "", fmt.Errorf("generate random bytes: %w", err)
	}
	return base64.StdEncoding.EncodeToString(b), nil
}