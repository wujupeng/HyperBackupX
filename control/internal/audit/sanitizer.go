package audit

import (
	"regexp"
	"strings"
)

// Sanitizer 审计详情脱敏器
type Sanitizer struct {
	patterns []*regexp.Regexp
}

// NewSanitizer 创建审计脱敏器
func NewSanitizer() *Sanitizer {
	patterns := []*regexp.Regexp{
		regexp.MustCompile(`(?i)(password|passwd|pwd)\s*[=:]\s*\S+`),
		regexp.MustCompile(`(?i)(secret|token|api_key|apikey|access_key|secret_key)\s*[=:]\s*\S+`),
		regexp.MustCompile(`(?i)(private_key|privatekey|encryption_key)\s*[=:]\s*\S+`),
		regexp.MustCompile(`(?i)(credential|cred)\s*[=:]\s*\S+`),
		regexp.MustCompile(`-----BEGIN [A-Z ]*PRIVATE KEY-----[\s\S]*?-----END [A-Z ]*PRIVATE KEY-----`),
		regexp.MustCompile(`(?i)(authorization)\s*:\s*Bearer\s+\S+`),
		regexp.MustCompile(`(?i)(bearer)\s+[A-Za-z0-9\-_\.]+`),
		regexp.MustCompile(`(?i)(mongodb|redis|postgres|mysql|s3|sftp|ftp)://[^\s]+`),
		regexp.MustCompile(`(?i)(connection_string|connstr)\s*[=:]\s*\S+`),
	}

	return &Sanitizer{patterns: patterns}
}

// SanitizeString 脱敏字符串
func (s *Sanitizer) SanitizeString(input string) string {
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

// SanitizeDetail 脱敏审计详情 map
func (s *Sanitizer) SanitizeDetail(detail map[string]interface{}) map[string]interface{} {
	if detail == nil {
		return nil
	}
	result := make(map[string]interface{}, len(detail))
	for k, v := range detail {
		result[k] = s.sanitizeValue(v)
	}
	return result
}

func (s *Sanitizer) sanitizeValue(v interface{}) interface{} {
	switch val := v.(type) {
	case string:
		return s.SanitizeString(val)
	case map[string]interface{}:
		return s.SanitizeDetail(val)
	case []interface{}:
		result := make([]interface{}, len(val))
		for i, item := range val {
			result[i] = s.sanitizeValue(item)
		}
		return result
	default:
		return v
	}
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

// ValidateEntry 验证审计条目不含明文敏感数据
// 返回违规字段列表
func (s *Sanitizer) ValidateEntry(entry Entry) []string {
	var violations []string

	for key, val := range entry.Detail {
		if str, ok := val.(string); ok {
			if s.ContainsSensitive(str) {
				violations = append(violations, "detail."+key)
			}
		}
	}

	return violations
}

// SanitizeEntry 脱敏审计条目（原地修改）
func (s *Sanitizer) SanitizeEntry(entry *Entry) {
	entry.Detail = s.SanitizeDetail(entry.Detail)
}