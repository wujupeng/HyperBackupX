package logagg

import (
	"regexp"
	"strings"
)

// Sanitizer 日志脱敏器
type Sanitizer struct {
	patterns []*regexp.Regexp
}

// NewSanitizer 创建脱敏器
func NewSanitizer() *Sanitizer {
	patterns := []*regexp.Regexp{
		regexp.MustCompile(`(?i)(password|passwd|pwd)\s*[=:]\s*\S+`),
		regexp.MustCompile(`(?i)(secret|token|api_key|apikey)\s*[=:]\s*\S+`),
		regexp.MustCompile(`(?i)(private_key|privatekey)\s*[=:]\s*\S+`),
		regexp.MustCompile(`(?i)(credential|cred)\s*[=:]\s*\S+`),
		regexp.MustCompile(`-----BEGIN [A-Z ]*PRIVATE KEY-----[\s\S]*?-----END [A-Z ]*PRIVATE KEY-----`),
		regexp.MustCompile(`(?i)(authorization)\s*:\s*Bearer\s+\S+`),
		regexp.MustCompile(`(?i)(bearer)\s+[A-Za-z0-9\-_\.]+`),
		regexp.MustCompile(`(?i)(mongodb|redis|postgres|mysql|s3|sftp|ftp)://[^\s]+`),
		regexp.MustCompile(`(?i)(access_key|accesskey|secret_key|secretkey)\s*[=:]\s*\S+`),
		regexp.MustCompile(`(?i)(encryption_key|encryptionkey)\s*[=:]\s*\S+`),
	}

	return &Sanitizer{patterns: patterns}
}

// Sanitize 脱敏日志内容
func (s *Sanitizer) Sanitize(input string) string {
	result := input
	for _, p := range s.patterns {
		result = p.ReplaceAllStringFunc(result, func(match string) string {
			if strings.Contains(match, "-----BEGIN") {
				return "-----REDACTED PRIVATE KEY-----"
			}
			if strings.Contains(match, "://") {
				parts := strings.SplitN(match, "://", 2)
				return parts[0] + "://***REDACTED***"
			}
			if strings.Contains(match, ":") {
				idx := strings.Index(match, ":")
				return match[:idx+1] + " ***REDACTED***"
			}
			if strings.Contains(match, "=") {
				idx := strings.Index(match, "=")
				return match[:idx+1] + "***REDACTED***"
			}
			return "***REDACTED***"
		})
	}
	return result
}

// SanitizeMap 脱敏 map 中的所有字符串值
func (s *Sanitizer) SanitizeMap(m map[string]string) map[string]string {
	result := make(map[string]string, len(m))
	for k, v := range m {
		result[k] = s.Sanitize(v)
	}
	return result
}

// ContainsSensitive 检查是否包含敏感信息
func (s *Sanitizer) ContainsSensitive(input string) bool {
	for _, p := range s.patterns {
		if p.MatchString(input) {
			return true
		}
	}
	return false
}

// ValidateLogEntry 验证日志条目不含明文密钥
func (s *Sanitizer) ValidateLogEntry(message string, fields map[string]string) []string {
	var violations []string

	if s.ContainsSensitive(message) {
		violations = append(violations, "message contains sensitive data")
	}

	for k, v := range fields {
		if s.ContainsSensitive(v) {
			violations = append(violations, "field '"+k+"' contains sensitive data")
		}
	}

	return violations
}