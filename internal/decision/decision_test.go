package decision

import (
	"strings"
	"testing"
	"time"
)

func TestRedactReceipt(t *testing.T) {
	receipt := DecisionReceipt{
		Decision:  "Use api_key=sk-or-12345",
		Rationale: "Bearer token-abc and email user@example.com",
		Alternatives: []string{
			"Option with secret=mysecret",
			"IP 192.168.1.1 is used",
		},
		Category: CategoryTool,
	}

	redacted := RedactReceipt(receipt)

	if redacted.Decision == receipt.Decision {
		t.Error("Decision should be redacted")
	}
	if !strings.Contains(redacted.Decision, "REDACTED") {
		t.Errorf("Decision should contain REDACTED, got %s", redacted.Decision)
	}

	if redacted.Rationale == receipt.Rationale {
		t.Error("Rationale should be redacted")
	}
	if !strings.Contains(redacted.Rationale, "REDACTED") {
		t.Errorf("Rationale should contain REDACTED, got %s", redacted.Rationale)
	}

	if len(redacted.Alternatives) != len(receipt.Alternatives) {
		t.Errorf("Alternatives length mismatch: %d vs %d", len(redacted.Alternatives), len(receipt.Alternatives))
	}
	for i, alt := range redacted.Alternatives {
		if alt == receipt.Alternatives[i] {
			t.Errorf("Alternative %d should be redacted", i)
		}
	}
}

func TestRedactString(t *testing.T) {
	tests := []struct {
		name     string
		input    string
		contains string
	}{
		{"API key", "api_key=secret123", "REDACTED"},
		{"Bearer token", "Authorization: Bearer abc123", "REDACTED"},
		{"Email", "user@example.com", "@***.***"},
		{"IP address", "192.168.1.1", "***.***.***"},
		{"No sensitive data", "hello world", "hello world"},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			result := RedactString(tt.input)
			if !strings.Contains(result, tt.contains) {
				t.Errorf("RedactString(%q) = %q, want to contain %q", tt.input, result, tt.contains)
			}
		})
	}
}

func TestRedactSlice(t *testing.T) {
	decisions := []DecisionReceipt{
		{Decision: "Use api_key=sk-or-123", Category: CategoryTool},
		{Decision: "Use token=abc", Category: CategoryModel},
	}

	redacted := RedactSlice(decisions)

	if len(redacted) != 2 {
		t.Errorf("Expected 2 decisions, got %d", len(redacted))
	}
	for i, d := range redacted {
		if d.Decision == decisions[i].Decision {
			t.Errorf("Decision %d should be redacted", i)
		}
	}
}

func TestCostSummary(t *testing.T) {
	decisions := []DecisionReceipt{
		{Cost: Cost{Tokens: 100, Duration: 1.5, TokensUSD: 0.01, Attempts: 1, RetryCount: 0}},
		{Cost: Cost{Tokens: 200, Duration: 2.5, TokensUSD: 0.025, Attempts: 2, RetryCount: 1}},
	}

	summary := CostSummary(decisions)

	if summary.Tokens != 300 {
		t.Errorf("Total Tokens = %d, want 300", summary.Tokens)
	}
	if summary.Duration != 4.0 {
		t.Errorf("Total Duration = %f, want 4.0", summary.Duration)
	}
	if summary.TokensUSD < 0.0349 || summary.TokensUSD > 0.0351 {
		t.Errorf("Total TokensUSD = %f, want ~0.035", summary.TokensUSD)
	}
	if summary.Attempts != 2 {
		t.Errorf("Max Attempts = %d, want 2", summary.Attempts)
	}
	if summary.RetryCount != 1 {
		t.Errorf("Total RetryCount = %d, want 1", summary.RetryCount)
	}
}

func TestDecisionReceipt_Summary(t *testing.T) {
	r := DecisionReceipt{
		Decision:  "test decision",
		Rationale: "test rationale",
		Category:  CategoryTool,
	}
	summary := r.Summary()
	expected := "[tool] test decision (rationale: test rationale)"
	if summary != expected {
		t.Errorf("Summary() = %q, want %q", summary, expected)
	}
}

func TestDecisionReceipt_Getters(t *testing.T) {
	now := time.Now()
	r := DecisionReceipt{
		Timestamp:    now,
		Decision:     "test",
		Rationale:    "why",
		Alternatives: []string{"a", "b"},
		Cost:         Cost{Tokens: 10},
		Category:     CategoryModel,
	}

	if r.GetTimestamp() != now {
		t.Error("GetTimestamp failed")
	}
	if r.GetDecision() != "test" {
		t.Error("GetDecision failed")
	}
	if r.GetRationale() != "why" {
		t.Error("GetRationale failed")
	}
	if len(r.GetAlternatives()) != 2 {
		t.Error("GetAlternatives failed")
	}
	if r.GetCost().Tokens != 10 {
		t.Error("GetCost failed")
	}
	if r.GetCategory() != CategoryModel {
		t.Error("GetCategory failed")
	}
}

func TestCategory_Constants(t *testing.T) {
	categories := []Category{
		CategoryTool, CategoryModel, CategoryPlan, CategoryRetry,
		CategoryIntent, CategoryStrategy, CategoryAmbiguous,
	}

	for _, c := range categories {
		if string(c) == "" {
			t.Errorf("Category %v has empty string value", c)
		}
	}
}

func TestLogger_Basic(t *testing.T) {
	logger := NewLogger(10)
	defer logger.Close()

	logger.Log(DecisionReceipt{Decision: "test1", Category: CategoryTool})
	logger.Log(DecisionReceipt{Decision: "test2", Category: CategoryTool})

	time.Sleep(50 * time.Millisecond)

	snapshot := logger.Snapshot()
	if len(snapshot) != 2 {
		t.Errorf("Expected 2 decisions in snapshot, got %d", len(snapshot))
	}
}

func TestLogger_Overflow(t *testing.T) {
	l := NewLogger(2)
	defer l.Close()

	l.Log(DecisionReceipt{Decision: "1", Category: CategoryTool})
	l.Log(DecisionReceipt{Decision: "2", Category: CategoryTool})
	l.Log(DecisionReceipt{Decision: "3", Category: CategoryTool})
	l.Log(DecisionReceipt{Decision: "4", Category: CategoryTool})

	time.Sleep(50 * time.Millisecond)

	snapshot := l.Snapshot()
	if len(snapshot) != 4 {
		t.Errorf("Expected 4 decisions in snapshot (including ring buffer), got %d", len(snapshot))
	}
}

func TestLogger_Close(t *testing.T) {
	l := NewLogger(10)
	l.Log(DecisionReceipt{Decision: "test"})
	l.Close()
	// Should not panic on second close
	l.Close()
}

func TestLogger_CloseLogsAfterClose(t *testing.T) {
	l := NewLogger(10)
	l.Close()
	// Should not panic
	l.Log(DecisionReceipt{Decision: "after close"})
}
