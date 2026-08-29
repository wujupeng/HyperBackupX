package log
package log

import (
	"context"
	"log/slog"
	"regexp"
	"strings"
)

var (
	sensitiveFields = map[string]bool{
		"password":      true,
		"passwd":        true,
		"secret":        true,
		"token":         true,
		"authorization": true,
		"dsn":           true,
		"private_key":   true,
		"api_key":       true,
		"jwt_secret":    true,
		"db_password":   true,
	}

	dsnPattern   = regexp.MustCompile(`postgres://[^@]+@`)
	bearerPattern = regexp.MustCompile(`Bearer\s+\S+`)
	keyPattern   = regexp.MustCompile(`-----BEGIN [A-Z ]*PRIVATE KEY-----[\s\S]*?-----END [A-Z ]*PRIVATE KEY-----`)
)

type RedactingHandler struct {
	inner slog.Handler
}

func NewRedactingHandler(inner slog.Handler) *RedactingHandler {
	return &RedactingHandler{inner: inner}
}

func (h *RedactingHandler) Enabled(ctx context.Context, level slog.Level) bool {
	return h.inner.Enabled(ctx, level)
}

func (h *RedactingHandler) Handle(ctx context.Context, r slog.Record) error {
	filtered := slog.NewRecord(r.Time, r.Level, redactMessage(r.Message), r.PC)
	r.Attrs(func(a slog.Attr) bool {
		filtered.AddAttrs(redactAttr(a))
		return true
	})
	return h.inner.Handle(ctx, filtered)
}

func (h *RedactingHandler) WithAttrs(attrs []slog.Attr) slog.Handler {
	redacted := make([]slog.Attr, len(attrs))
	for i, a := range attrs {
		redacted[i] = redactAttr(a)
	}
	return &RedactingHandler{inner: h.inner.WithAttrs(redacted)}
}

func (h *RedactingHandler) WithGroup(name string) slog.Handler {
	return &RedactingHandler{inner: h.inner.WithGroup(name)}
}

func redactAttr(a slog.Attr) slog.Attr {
	if sensitiveFields[strings.ToLower(a.Key)] {
		return slog.String(a.Key, "[REDACTED]")
	}
	if a.Value.Kind() == slog.KindString {
		return slog.String(a.Key, redactMessage(a.Value.String()))
	}
	return a
}

func redactMessage(msg string) string {
	msg = dsnPattern.ReplaceAllString(msg, "postgres://[REDACTED]@")
	msg = bearerPattern.ReplaceAllString(msg, "Bearer [REDACTED]")
	msg = keyPattern.ReplaceAllString(msg, "[REDACTED-KEY]")
	return msg
}

func Install() {
	defaultHandler := slog.Default().Handler()
	slog.SetDefault(slog.New(NewRedactingHandler(defaultHandler)))
}