package components

import (
	"testing"
	"time"

	"github.com/eshanized/M31A/pkg/types"
)

func TestThinkingLabel_PlanPhase(t *testing.T) {
	tests := []struct {
		elapsed time.Duration
		want    string
	}{
		{500 * time.Millisecond, "Analyzing code"},
		{1 * time.Second, "Analyzing code"},
		{2 * time.Second, "Planning implementation"},
		{5 * time.Second, "Planning implementation"},
		{30 * time.Second, "Planning implementation"},
	}

	for _, tt := range tests {
		t.Run(tt.elapsed.String(), func(t *testing.T) {
			got := thinkingLabel("plan", "", tt.elapsed)
			if got != tt.want {
				t.Errorf("thinkingLabel(\"plan\", \"\", %v) = %q, want %q", tt.elapsed, got, tt.want)
			}
		})
	}
}

func TestThinkingLabel_ExecutePhase(t *testing.T) {
	tests := []struct {
		taskAction string
		elapsed    time.Duration
		want       string
	}{
		{"Write auth handler", 100 * time.Millisecond, "Implementing changes"},
		{"Write auth handler", 5 * time.Second, "Implementing changes"},
		{"", 1 * time.Second, "Analyzing code"},
		{"", 3 * time.Second, "Implementing changes"},
		{"", 10 * time.Second, "Implementing changes"},
	}

	for _, tt := range tests {
		t.Run(tt.taskAction, func(t *testing.T) {
			got := thinkingLabel("execute", tt.taskAction, tt.elapsed)
			if got != tt.want {
				t.Errorf("thinkingLabel(\"execute\", %q, %v) = %q, want %q", tt.taskAction, tt.elapsed, got, tt.want)
			}
		})
	}
}

func TestThinkingLabel_VerifyPhase(t *testing.T) {
	got := thinkingLabel("verify", "", 2*time.Second)
	want := "Verifying results"
	if got != want {
		t.Errorf("thinkingLabel(\"verify\", \"\", 2s) = %q, want %q", got, want)
	}
}

func TestThinkingLabel_DiscussPhase(t *testing.T) {
	got := thinkingLabel("discuss", "", 3*time.Second)
	want := "Planning implementation"
	if got != want {
		t.Errorf("thinkingLabel(\"discuss\", \"\", 3s) = %q, want %q", got, want)
	}
}

func TestThinkingLabel_ShipPhase(t *testing.T) {
	got := thinkingLabel("ship", "", 2*time.Second)
	want := "Synthesizing"
	if got != want {
		t.Errorf("thinkingLabel(\"ship\", \"\", 2s) = %q, want %q", got, want)
	}
}

func TestThinkingLabel_RuntimePhase(t *testing.T) {
	got := thinkingLabel("runtime", "", 1*time.Second)
	want := "Analyzing code"
	if got != want {
		t.Errorf("thinkingLabel(\"runtime\", \"\", 1s) = %q, want %q", got, want)
	}
}

func TestThinkingLabel_UnknownPhase(t *testing.T) {
	tests := []struct {
		elapsed time.Duration
		want    string
	}{
		{100 * time.Millisecond, "Analyzing"},
		{1 * time.Second, "Refining approach"},
		{5 * time.Second, "Synthesizing"},
		{10 * time.Second, "Synthesizing"},
	}

	for _, tt := range tests {
		t.Run(tt.elapsed.String(), func(t *testing.T) {
			got := thinkingLabel("", "", tt.elapsed)
			if got != tt.want {
				t.Errorf("thinkingLabel(\"\", \"\", %v) = %q, want %q", tt.elapsed, got, tt.want)
			}
		})
	}
}

func TestThinkingLabel_CaseInsensitive(t *testing.T) {
	got := thinkingLabel("PLAN", "", 5*time.Second)
	want := "Planning implementation"
	if got != want {
		t.Errorf("thinkingLabel(\"PLAN\", \"\", 5s) = %q, want %q", got, want)
	}
}

func TestThinkingLabel_Duration(t *testing.T) {
	// Test that the label is consistent regardless of exact duration
	label1 := thinkingLabel("plan", "", 2*time.Second)
	label2 := thinkingLabel("plan", "", 2*time.Second)
	if label1 != label2 {
		t.Errorf("thinkingLabel should be deterministic, got %q and %q", label1, label2)
	}
}

func TestThinkingBlock_SetContext(t *testing.T) {
	block := &ThinkingBlock{}
	block.SetContext("execute", "Write auth handler")
	if block.phase != "execute" {
		t.Errorf("phase = %q, want %q", block.phase, "execute")
	}
	if block.taskAction != "Write auth handler" {
		t.Errorf("taskAction = %q, want %q", block.taskAction, "Write auth handler")
	}
}

func TestThinkingBlock_Elapsed(t *testing.T) {
	block := &ThinkingBlock{
		startedAt: time.Now().Add(-5 * time.Second),
	}
	elapsed := block.Elapsed()
	if elapsed < 4*time.Second || elapsed > 6*time.Second {
		t.Errorf("Elapsed() = %v, want ~5s", elapsed)
	}
}

func TestThinkingBlock_Elapsed_WithDurationMs(t *testing.T) {
	block := &ThinkingBlock{
		segment: types.MessageSegment{
			DurationMs: 3000,
		},
		startedAt: time.Now().Add(-10 * time.Second),
	}
	elapsed := block.Elapsed()
	if elapsed != 3*time.Second {
		t.Errorf("Elapsed() = %v, want 3s", elapsed)
	}
}

func TestClassifyThinkingIntent(t *testing.T) {
	tests := []struct {
		name       string
		phase      string
		taskAction string
		elapsed    time.Duration
		want       ThinkingIntent
	}{
		{"plan short", "plan", "", 500 * time.Millisecond, IntentAnalyzing},
		{"plan long", "plan", "", 3 * time.Second, IntentPlanning},
		{"execute with action", "execute", "Write code", 100 * time.Millisecond, IntentImplementing},
		{"execute no action short", "execute", "", 1 * time.Second, IntentAnalyzing},
		{"execute no action long", "execute", "", 5 * time.Second, IntentImplementing},
		{"verify", "verify", "", 2 * time.Second, IntentVerifying},
		{"discuss", "discuss", "", 3 * time.Second, IntentPlanning},
		{"ship", "ship", "", 2 * time.Second, IntentSynthesizing},
		{"runtime", "runtime", "", 1 * time.Second, IntentAnalyzing},
		{"unknown short", "", "", 500 * time.Millisecond, IntentAnalyzing},
		{"unknown medium", "", "", 2 * time.Second, IntentRefining},
		{"unknown long", "", "", 10 * time.Second, IntentSynthesizing},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got := classifyThinkingIntent(tt.phase, tt.taskAction, tt.elapsed)
			if got != tt.want {
				t.Errorf("classifyThinkingIntent(%q, %q, %v) = %d, want %d", tt.phase, tt.taskAction, tt.elapsed, got, tt.want)
			}
		})
	}
}

func TestIntentLabel(t *testing.T) {
	tests := []struct {
		intent  ThinkingIntent
		elapsed time.Duration
		want    string
	}{
		{IntentPlanning, 2 * time.Second, "Planning implementation"},
		{IntentImplementing, 2 * time.Second, "Implementing changes"},
		{IntentVerifying, 2 * time.Second, "Verifying results"},
		{IntentAnalyzing, 100 * time.Millisecond, "Analyzing"},
		{IntentAnalyzing, 1 * time.Second, "Analyzing code"},
		{IntentRefining, 2 * time.Second, "Refining approach"},
		{IntentResearching, 2 * time.Second, "Researching"},
		{IntentSynthesizing, 2 * time.Second, "Synthesizing"},
		{IntentUnknown, 500 * time.Millisecond, "Thinking"},
		{IntentUnknown, 2 * time.Second, "Thinking…"},
	}

	for _, tt := range tests {
		t.Run(tt.want, func(t *testing.T) {
			got := intentLabel(tt.intent, tt.elapsed)
			if got != tt.want {
				t.Errorf("intentLabel(%d, %v) = %q, want %q", tt.intent, tt.elapsed, got, tt.want)
			}
		})
	}
}

func TestThinkingLabelExported(t *testing.T) {
	// Test that the exported function works the same as the internal one
	got := ThinkingLabel("plan", "", 5*time.Second)
	want := thinkingLabel("plan", "", 5*time.Second)
	if got != want {
		t.Errorf("ThinkingLabel() = %q, want %q", got, want)
	}
}
