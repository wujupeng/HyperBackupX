package monitor

import (
	"context"
	"fmt"
	"sync"
	"time"
)

// AlertSeverity 告警级别
type AlertSeverity string

const (
	SeverityInfo     AlertSeverity = "info"
	SeverityWarning  AlertSeverity = "warning"
	SeverityCritical AlertSeverity = "critical"
)

// AlertState 告警状态
type AlertState string

const (
	StateActive    AlertState = "active"
	StateSuppressed AlertState = "suppressed"
	StateResolved  AlertState = "resolved"
)

// Alert 告警
type Alert struct {
	ID          string
	RuleID      string
	DeviceID    string
	Severity    AlertSeverity
	State       AlertState
	Message     string
	TriggeredAt time.Time
	SuppressedUntil time.Time
}

// AlertRule 告警规则
type AlertRule struct {
	ID          string
	Name        string
	Severity    AlertSeverity
	Metric      string
	Operator    string
	Threshold   float64
	SuppressFor time.Duration
	Enabled     bool
}

// NotificationChannel 通知通道
type NotificationChannel interface {
	Send(ctx context.Context, alert *Alert) error
	Name() string
}

// AlertEngine 告警引擎
type AlertEngine struct {
	mu         sync.RWMutex
	rules      map[string]*AlertRule
	activeAlerts map[string]*Alert
	channels   []NotificationChannel
	suppressWindow time.Duration
	pendingNotifications []*Alert
}

// NewAlertEngine 创建告警引擎
func NewAlertEngine() *AlertEngine {
	return &AlertEngine{
		rules:          make(map[string]*AlertRule),
		activeAlerts:   make(map[string]*Alert),
		suppressWindow: 5 * time.Minute,
	}
}

// AddRule 添加告警规则
func (e *AlertEngine) AddRule(rule *AlertRule) {
	e.mu.Lock()
	defer e.mu.Unlock()
	e.rules[rule.ID] = rule
}

// RemoveRule 移除告警规则
func (e *AlertEngine) RemoveRule(ruleID string) {
	e.mu.Lock()
	defer e.mu.Unlock()
	delete(e.rules, ruleID)
}

// AddChannel 添加通知通道
func (e *AlertEngine) AddChannel(ch NotificationChannel) {
	e.mu.Lock()
	defer e.mu.Unlock()
	e.channels = append(e.channels, ch)
}

// Evaluate 评估指标，触发告警
func (e *AlertEngine) Evaluate(ctx context.Context, metric string, value float64, deviceID string) []*Alert {
	e.mu.Lock()
	defer e.mu.Unlock()

	var triggered []*Alert
	now := time.Now().UTC()

	for _, rule := range e.rules {
		if !rule.Enabled || rule.Metric != metric {
			continue
		}

		if !matchesCondition(rule.Operator, value, rule.Threshold) {
			continue
		}

		alertKey := fmt.Sprintf("%s:%s:%s", rule.ID, deviceID, metric)
		existing, hasActive := e.activeAlerts[alertKey]

		if hasActive && now.Before(existing.SuppressedUntil) {
			continue
		}

		alert := &Alert{
			ID:            fmt.Sprintf("alert-%d", now.UnixNano()),
			RuleID:        rule.ID,
			DeviceID:      deviceID,
			Severity:      rule.Severity,
			State:         StateActive,
			Message:       fmt.Sprintf("%s: %s=%.2f (threshold %.2f)", rule.Name, metric, value, rule.Threshold),
			TriggeredAt:   now,
			SuppressedUntil: now.Add(rule.SuppressFor),
		}

		e.activeAlerts[alertKey] = alert
		triggered = append(triggered, alert)

		for _, ch := range e.channels {
			if err := ch.Send(ctx, alert); err != nil {
				e.pendingNotifications = append(e.pendingNotifications, alert)
			}
		}
	}

	return triggered
}

// Resolve 解决告警
func (e *AlertEngine) Resolve(ruleID, deviceID, metric string) {
	e.mu.Lock()
	defer e.mu.Unlock()

	alertKey := fmt.Sprintf("%s:%s:%s", ruleID, deviceID, metric)
	if alert, ok := e.activeAlerts[alertKey]; ok {
		alert.State = StateResolved
		delete(e.activeAlerts, alertKey)
	}
}

// GetActiveAlerts 获取活跃告警
func (e *AlertEngine) GetActiveAlerts() []*Alert {
	e.mu.RLock()
	defer e.mu.RUnlock()

	result := make([]*Alert, 0, len(e.activeAlerts))
	for _, a := range e.activeAlerts {
		result = append(result, a)
	}
	return result
}

// RetryPendingNotifications 重试失败的通知
func (e *AlertEngine) RetryPendingNotifications(ctx context.Context) int {
	e.mu.Lock()
	pending := e.pendingNotifications
	e.pendingNotifications = nil
	e.mu.Unlock()

	retried := 0
	for _, alert := range pending {
		for _, ch := range e.channels {
			if err := ch.Send(ctx, alert); err == nil {
				retried++
			} else {
				e.mu.Lock()
				e.pendingNotifications = append(e.pendingNotifications, alert)
				e.mu.Unlock()
			}
		}
	}
	return retried
}

// SetSuppressWindow 设置抑制窗口
func (e *AlertEngine) SetSuppressWindow(d time.Duration) {
	e.mu.Lock()
	defer e.mu.Unlock()
	e.suppressWindow = d
}

func matchesCondition(operator string, value, threshold float64) bool {
	switch operator {
	case ">":
		return value > threshold
	case ">=":
		return value >= threshold
	case "<":
		return value < threshold
	case "<=":
		return value <= threshold
	case "==":
		return value == threshold
	default:
		return false
	}
}

// EmailChannel 邮件通知通道
type EmailChannel struct {
	Addr    string
	From    string
	To      []string
}

func (c *EmailChannel) Name() string { return "email" }
func (c *EmailChannel) Send(ctx context.Context, alert *Alert) error {
	_ = ctx
	_ = alert
	return nil
}

// WebhookChannel Webhook 通知通道
type WebhookChannel struct {
	URL string
}

func (c *WebhookChannel) Name() string { return "webhook" }
func (c *WebhookChannel) Send(ctx context.Context, alert *Alert) error {
	_ = ctx
	_ = alert
	return nil
}

// IMChannel 企业 IM 通知通道
type IMChannel struct {
	Type string
	URL  string
}

func (c *IMChannel) Name() string { return "im" }
func (c *IMChannel) Send(ctx context.Context, alert *Alert) error {
	_ = ctx
	_ = alert
	return nil
}