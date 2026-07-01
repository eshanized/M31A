package narrative

import (
	"testing"
	"time"
)

func TestCategoryString(t *testing.T) {
	tests := []struct {
		cat  Category
		want string
	}{
		{CategoryOrienting, "Orienting"},
		{CategoryUnderstanding, "Understanding"},
		{CategoryDiscussing, "Discussing"},
		{CategoryPlanning, "Planning"},
		{CategoryResearching, "Researching"},
		{CategoryExecuting, "Executing"},
		{CategoryVerifying, "Verifying"},
		{CategoryRecovering, "Recovering"},
		{CategoryShipping, "Shipping"},
		{CategoryLearning, "Learning"},
		{CategoryAlerting, "Alerting"},
		{Category(99), "Unknown"},
	}
	for _, tt := range tests {
		if got := tt.cat.String(); got != tt.want {
			t.Errorf("Category(%d).String() = %q, want %q", tt.cat, got, tt.want)
		}
	}
}

func TestRawEventGetString(t *testing.T) {
	e := RawEvent{
		Type: EventTaskStart,
		Data: map[string]interface{}{
			"description": "implement auth",
			"count":       42,
			"flag":        true,
		},
	}
	if got := e.GetString("description"); got != "implement auth" {
		t.Errorf("GetString(description) = %q", got)
	}
	if got := e.GetString("missing"); got != "" {
		t.Errorf("GetString(missing) = %q, want empty", got)
	}
	if got := e.GetString("count"); got != "" {
		t.Errorf("GetString(count) = %q, want empty (int not string)", got)
	}
}

func TestRawEventGetInt(t *testing.T) {
	e := RawEvent{
		Type: EventTaskStart,
		Data: map[string]interface{}{
			"count":   42,
			"float":   3.14,
			"missing": "text",
		},
	}
	if got := e.GetInt("count"); got != 42 {
		t.Errorf("GetInt(count) = %d, want 42", got)
	}
	if got := e.GetInt("float"); got != 3 {
		t.Errorf("GetInt(float) = %d, want 3", got)
	}
	if got := e.GetInt("missing"); got != 0 {
		t.Errorf("GetInt(missing) = %d, want 0", got)
	}
}

func TestRawEventGetBool(t *testing.T) {
	e := RawEvent{
		Type: EventTaskStart,
		Data: map[string]interface{}{
			"flag":    true,
			"notflag": "yes",
		},
	}
	if got := e.GetBool("flag"); !got {
		t.Errorf("GetBool(flag) = false, want true")
	}
	if got := e.GetBool("notflag"); got {
		t.Errorf("GetBool(notflag) = true, want false")
	}
	if got := e.GetBool("missing"); got {
		t.Errorf("GetBool(missing) = true, want false")
	}
}

func TestNewRawEvent(t *testing.T) {
	before := time.Now()
	e := NewRawEvent(EventTaskStart, nil)
	after := time.Now()

	if e.Type != EventTaskStart {
		t.Errorf("Type = %q, want %q", e.Type, EventTaskStart)
	}
	if e.Data == nil {
		t.Error("Data should be initialized")
	}
	if e.Timestamp.Before(before) || e.Timestamp.After(after) {
		t.Error("Timestamp out of range")
	}
}

func TestNarrativeObjectIsZero(t *testing.T) {
	var n NarrativeObject
	if !n.IsZero() {
		t.Error("Zero value should be IsZero")
	}
	n.Type = NarrativeTaskComplete
	if n.IsZero() {
		t.Error("Non-zero value should not be IsZero")
	}
}

func TestNarrativeObjectDisplayDuration(t *testing.T) {
	tests := []struct {
		cat  Category
		want time.Duration
	}{
		{CategoryAlerting, 5 * time.Second},
		{CategoryRecovering, 2 * time.Second},
		{CategoryExecuting, 1 * time.Second},
		{CategoryShipping, 2 * time.Second},
		{CategoryLearning, 3 * time.Second},
		{CategoryPlanning, 1 * time.Second},
	}
	for _, tt := range tests {
		n := NarrativeObject{Category: tt.cat}
		if got := n.DisplayDuration(); got != tt.want {
			t.Errorf("Category(%d).DisplayDuration() = %v, want %v", tt.cat, got, tt.want)
		}
	}
}

func TestNarrativeObjectCustomDuration(t *testing.T) {
	n := NarrativeObject{
		Category: CategoryExecuting,
		Duration: 10 * time.Second,
	}
	if got := n.DisplayDuration(); got != 10*time.Second {
		t.Errorf("Custom duration = %v, want 10s", got)
	}
}

func TestPriorityOrdering(t *testing.T) {
	if PriorityActionRequired >= PriorityPhase {
		t.Error("ActionRequired should be < Phase")
	}
	if PriorityPhase >= PriorityOrientation {
		t.Error("Phase should be < Orientation")
	}
	if PriorityOrientation >= PriorityTask {
		t.Error("Orientation should be < Task")
	}
	if PriorityTask >= PriorityCost {
		t.Error("Task should be < Cost")
	}
}
