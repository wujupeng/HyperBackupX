package monitor

import (
	"context"
	"fmt"
	"testing"
	"time"
)

type mockChannel struct {
	name     string
	sent     []*Alert
	failNext bool
}

func (m *mockChannel) Name() string { return m.name }
func (m *mockChannel) Send(_ context.Context, alert *Alert) error {
	if m.failNext {
		m.failNext = false
		return fmt.Errorf("send failed")
	}
	m.sent = append(m.sent, alert)
	return nil
}

func TestAddAndRemoveRule(t *testing.T) {
	e := NewAlertEngine()
	rule := &AlertRule{ID: "r1", Name: "high-cpu", Metric: "cpu_usage", Operator: ">", Threshold: 90, Enabled: true, Severity: SeverityWarning}
	e.AddRule(rule)

	ctx := context.Background()
	alerts := e.Evaluate(ctx, "cpu_usage", 95, "device-1")
	if len(alerts) != 1 {
		t.Fatalf("Expected 1 alert, got %d", len(alerts))
	}

	e.RemoveRule("r1")
	alerts = e.Evaluate(ctx, "cpu_usage", 95, "device-1")
	if len(alerts) != 0 {
		t.Fatalf("Expected 0 alerts after remove, got %d", len(alerts))
	}
}

func TestEvaluateNoMatch(t *testing.T) {
	e := NewAlertEngine()
	e.AddRule(&AlertRule{ID: "r1", Metric: "cpu_usage", Operator: ">", Threshold: 90, Enabled: true, Severity: SeverityWarning})

	alerts := e.Evaluate(context.Background(), "cpu_usage", 50, "device-1")
	if len(alerts) != 0 {
		t.Fatalf("Expected 0 alerts, got %d", len(alerts))
	}
}

func TestAlertSuppression(t *testing.T) {
	e := NewAlertEngine()
	e.AddRule(&AlertRule{
		ID: "r1", Metric: "cpu_usage", Operator: ">", Threshold: 90,
		Enabled: true, Severity: SeverityWarning, SuppressFor: 1 * time.Hour,
	})

	ctx := context.Background()
	first := e.Evaluate(ctx, "cpu_usage", 95, "device-1")
	if len(first) != 1 {
		t.Fatalf("Expected 1 alert, got %d", len(first))
	}

	second := e.Evaluate(ctx, "cpu_usage", 95, "device-1")
	if len(second) != 0 {
		t.Fatalf("Expected 0 alerts (suppressed), got %d", len(second))
	}
}

func TestAlertResolve(t *testing.T) {
	e := NewAlertEngine()
	e.AddRule(&AlertRule{ID: "r1", Metric: "cpu_usage", Operator: ">", Threshold: 90, Enabled: true, Severity: SeverityWarning})

	ctx := context.Background()
	e.Evaluate(ctx, "cpu_usage", 95, "device-1")

	if len(e.GetActiveAlerts()) != 1 {
		t.Fatal("Expected 1 active alert")
	}

	e.Resolve("r1", "device-1", "cpu_usage")
	if len(e.GetActiveAlerts()) != 0 {
		t.Fatal("Expected 0 active alerts after resolve")
	}
}

func TestNotificationChannels(t *testing.T) {
	e := NewAlertEngine()
	ch1 := &mockChannel{name: "ch1"}
	ch2 := &mockChannel{name: "ch2"}
	e.AddChannel(ch1)
	e.AddChannel(ch2)

	e.AddRule(&AlertRule{ID: "r1", Metric: "cpu", Operator: ">", Threshold: 80, Enabled: true, Severity: SeverityCritical})
	e.Evaluate(context.Background(), "cpu", 90, "device-1")

	if len(ch1.sent) != 1 {
		t.Fatalf("Expected ch1 to receive 1 alert, got %d", len(ch1.sent))
	}
	if len(ch2.sent) != 1 {
		t.Fatalf("Expected ch2 to receive 1 alert, got %d", len(ch2.sent))
	}
}

func TestRetryPendingNotifications(t *testing.T) {
	e := NewAlertEngine()
	ch := &mockChannel{name: "ch", failNext: true}
	e.AddChannel(ch)

	e.AddRule(&AlertRule{ID: "r1", Metric: "cpu", Operator: ">", Threshold: 80, Enabled: true, Severity: SeverityCritical})
	e.Evaluate(context.Background(), "cpu", 90, "device-1")

	if len(ch.sent) != 0 {
		t.Fatal("Expected 0 sent (failed)")
	}

	retried := e.RetryPendingNotifications(context.Background())
	if retried != 1 {
		t.Fatalf("Expected 1 retried, got %d", retried)
	}
}

func TestMultipleRulesSameMetric(t *testing.T) {
	e := NewAlertEngine()
	e.AddRule(&AlertRule{ID: "r1", Metric: "cpu", Operator: ">", Threshold: 80, Enabled: true, Severity: SeverityWarning})
	e.AddRule(&AlertRule{ID: "r2", Metric: "cpu", Operator: ">", Threshold: 95, Enabled: true, Severity: SeverityCritical})

	alerts := e.Evaluate(context.Background(), "cpu", 96, "device-1")
	if len(alerts) != 2 {
		t.Fatalf("Expected 2 alerts, got %d", len(alerts))
	}
}

func TestDisabledRule(t *testing.T) {
	e := NewAlertEngine()
	e.AddRule(&AlertRule{ID: "r1", Metric: "cpu", Operator: ">", Threshold: 80, Enabled: false, Severity: SeverityWarning})

	alerts := e.Evaluate(context.Background(), "cpu", 90, "device-1")
	if len(alerts) != 0 {
		t.Fatalf("Expected 0 alerts for disabled rule, got %d", len(alerts))
	}
}