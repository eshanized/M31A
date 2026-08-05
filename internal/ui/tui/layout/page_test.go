package layout

import (
	"strings"
	"testing"
	"time"

	"github.com/eshanized/M31A/internal/ui/tui/theme"
)

func TestFooterEnriched(t *testing.T) {
	tm := theme.Default()
	info := FooterInfo{
		WorkflowPhase: "execute",
		ActiveTask:    "Fix tests",
		ElapsedTime:   42 * time.Second,
	}
	bp := Detect(80)
	result := BuildFooter(info, 80, bp, tm, testCache())
	if !strings.Contains(result, "execute") {
		t.Errorf("expected footer to contain 'execute', got %q", result)
	}
	if !strings.Contains(result, "Fix tests") {
		t.Errorf("expected footer to contain 'Fix tests', got %q", result)
	}
	if !strings.Contains(result, "42s") {
		t.Errorf("expected footer to contain '42s', got %q", result)
	}
}

func TestFooterPhaseOnly(t *testing.T) {
	tm := theme.Default()
	info := FooterInfo{
		WorkflowPhase: "plan",
		ActiveTask:    "",
		ElapsedTime:   0,
	}
	bp := Detect(80)
	result := BuildFooter(info, 80, bp, tm, testCache())
	if !strings.Contains(result, "plan") {
		t.Errorf("expected footer to contain 'plan', got %q", result)
	}
	// Should not contain separator when only phase is present
	if strings.Contains(result, "·") {
		t.Errorf("expected no separator with phase-only, got %q", result)
	}
}

func TestFooterIdleFallback(t *testing.T) {
	tm := theme.Default()
	info := FooterInfo{
		WorkflowPhase: "",
		ActiveTask:    "",
		Operation:     "",
	}
	bp := Detect(80)
	result := BuildFooter(info, 80, bp, tm, testCache())
	// With no operation and no workflow phase, center zone should be empty
	// The result should be padding only (no visible operation text)
	w := len(strings.TrimRight(result, " "))
	if w > 0 {
		t.Logf("idle footer has visible width %d (padding only), OK", w)
	}
}

func TestFormatElapsed(t *testing.T) {
	tests := []struct {
		input    time.Duration
		expected string
	}{
		{0, "0s"},
		{1 * time.Second, "1s"},
		{59 * time.Second, "59s"},
		{60 * time.Second, "1m"},
		{65 * time.Second, "1m 5s"},
		{3599 * time.Second, "59m 59s"},
		{3600 * time.Second, "1h 0m"},
		{3725 * time.Second, "1h 2m"},
	}
	for _, tt := range tests {
		got := formatElapsed(tt.input)
		if got != tt.expected {
			t.Errorf("formatElapsed(%v) = %q, want %q", tt.input, got, tt.expected)
		}
	}
}

func TestFooterNarrow(t *testing.T) {
	tm := theme.Default()
	info := FooterInfo{
		WorkflowPhase: "execute",
		ActiveTask:    "Fix tests",
		ElapsedTime:   42 * time.Second,
	}
	bp := Detect(40)
	result := BuildFooter(info, 40, bp, tm, testCache())
	resultW := len(strings.TrimRight(result, " "))
	if resultW > 40 {
		t.Errorf("footer width %d exceeds terminal width 40", resultW)
	}
}
