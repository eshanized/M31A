package compaction

import (
	"context"
	"testing"

	"github.com/eshanized/M31A/internal/tokens"
	"github.com/eshanized/M31A/pkg/types"
)

func TestDefaultConfig(t *testing.T) {
	cfg := DefaultConfig()
	if !cfg.Auto {
		t.Error("Auto = false, want true")
	}
	if cfg.Buffer != 20000 {
		t.Errorf("Buffer = %d, want 20000", cfg.Buffer)
	}
	if cfg.KeepTokens != 8000 {
		t.Errorf("KeepTokens = %d, want 8000", cfg.KeepTokens)
	}
}

func TestNew(t *testing.T) {
	cfg := DefaultConfig()
	c := New(cfg, nil)
	if c == nil {
		t.Fatal("New() returned nil")
	}
	if c.cfg != cfg {
		t.Error("config not set correctly")
	}
}

func TestCompactor_ShouldCompact_Disabled(t *testing.T) {
	cfg := Config{Auto: false}
	c := New(cfg, nil)

	messages := []types.Message{
		{Role: "user", Content: "hello"},
	}

	if c.ShouldCompact(messages, 100000) {
		t.Error("ShouldCompact returned true when Auto is false")
	}
}

func TestCompactor_ShouldCompact_NilEstimator(t *testing.T) {
	cfg := Config{Auto: true}
	c := New(cfg, nil)

	messages := []types.Message{
		{Role: "user", Content: "hello"},
	}

	if c.ShouldCompact(messages, 100000) {
		t.Error("ShouldCompact returned true with nil estimator")
	}
}

func TestCompactor_ShouldCompact(t *testing.T) {
	cfg := Config{
		Auto:       true,
		Buffer:     100,
		KeepTokens: 10,
	}

	estimator := tokens.NewEstimator("gpt-4")
	c := New(cfg, estimator)

	// Small message - should not compact
	smallMessages := []types.Message{
		{Role: "user", Content: "hello"},
	}
	if c.ShouldCompact(smallMessages, 1000) {
		t.Error("ShouldCompact returned true for small message")
	}

	// Large message - should compact (need enough tokens to exceed threshold)
	largeContent := make([]byte, 10000)
	for i := range largeContent {
		largeContent[i] = 'a'
	}
	largeMessages := []types.Message{
		{Role: "user", Content: string(largeContent)},
	}
	if !c.ShouldCompact(largeMessages, 1000) {
		t.Error("ShouldCompact returned false for large message")
	}
}

func TestCompactor_ShouldCompact_ZeroContextLength(t *testing.T) {
	cfg := Config{
		Auto:       true,
		Buffer:     100,
		KeepTokens: 10,
	}

	estimator := tokens.NewEstimator("gpt-4")
	c := New(cfg, estimator)

	messages := []types.Message{
		{Role: "user", Content: "hello"},
	}

	// Zero context length should use default
	if c.ShouldCompact(messages, 0) {
		t.Error("ShouldCompact returned true for zero context length")
	}
}

func TestCompactor_ShouldCompact_NegativeThreshold(t *testing.T) {
	cfg := Config{
		Auto:       true,
		Buffer:     200000,
		KeepTokens: 10,
	}

	estimator := tokens.NewEstimator("gpt-4")
	c := New(cfg, estimator)

	messages := []types.Message{
		{Role: "user", Content: "hello"},
	}

	// Buffer > context length means threshold <= 0
	if c.ShouldCompact(messages, 100000) {
		t.Error("ShouldCompact returned true when threshold is negative")
	}
}

func TestCompactor_Compact_NilEstimator(t *testing.T) {
	cfg := DefaultConfig()
	c := New(cfg, nil)

	messages := []types.Message{
		{Role: "user", Content: "hello"},
	}

	result, err := c.Compact(context.TODO(), messages, nil, "")
	if err == nil {
		t.Error("expected error with nil estimator")
	}
	if result.Compacted {
		t.Error("expected Compacted = false")
	}
}

func TestCompactor_LastResult(t *testing.T) {
	cfg := DefaultConfig()
	c := New(cfg, nil)

	if c.LastResult() != nil {
		t.Error("LastResult() should be nil initially")
	}
}

func TestSerializeMessages(t *testing.T) {
	tests := []struct {
		name     string
		messages []types.Message
		contains []string
	}{
		{
			"user message",
			[]types.Message{{Role: "user", Content: "hello"}},
			[]string{"[User", "hello"},
		},
		{
			"assistant message",
			[]types.Message{{Role: "assistant", Content: "response"}},
			[]string{"[Assistant", "response"},
		},
		{
			"system message",
			[]types.Message{{Role: "system", Content: "system prompt"}},
			[]string{"[System", "system prompt"},
		},
		{
			"tool message",
			[]types.Message{{Role: "tool", Content: "tool output"}},
			[]string{"[Tool", "tool output"},
		},
		{
			"skip for llm",
			[]types.Message{{Role: "user", Content: "hello", SkipForLLM: true}},
			[]string{},
		},
		{
			"empty",
			[]types.Message{},
			[]string{},
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			result := SerializeMessages(tt.messages)
			for _, s := range tt.contains {
				if !containsString(result, s) {
					t.Errorf("SerializeMessages() = %q, does not contain %q", result, s)
				}
			}
		})
	}
}

func TestSerializeMessages_ToolCalls(t *testing.T) {
	messages := []types.Message{
		{
			Role:    "assistant",
			Content: "calling tool",
			ToolCalls: []types.ToolCall{
				{Name: "bash", Input: []byte(`{"command":"ls"}`)},
			},
		},
	}

	result := SerializeMessages(messages)
	if !containsString(result, "ToolCall: bash") {
		t.Errorf("SerializeMessages() = %q, missing tool call", result)
	}
}

func TestSerializeMessages_ToolCallTruncation(t *testing.T) {
	longInput := make([]byte, 600)
	for i := range longInput {
		longInput[i] = 'a'
	}

	messages := []types.Message{
		{
			Role:    "assistant",
			Content: "calling tool",
			ToolCalls: []types.ToolCall{
				{Name: "bash", Input: longInput},
			},
		},
	}

	result := SerializeMessages(messages)
	if !containsString(result, "...") {
		t.Error("expected truncation indicator")
	}
}

func TestSerializeMessages_ToolOutputTruncation(t *testing.T) {
	longOutput := make([]byte, 3000)
	for i := range longOutput {
		longOutput[i] = 'a'
	}

	messages := []types.Message{
		{Role: "tool", Content: string(longOutput)},
	}

	result := SerializeMessages(messages)
	if !containsString(result, "...[truncated]") {
		t.Error("expected truncation indicator for tool output")
	}
}

func TestSerializeMessages_SystemTruncation(t *testing.T) {
	longContent := make([]byte, 3000)
	for i := range longContent {
		longContent[i] = 'a'
	}

	messages := []types.Message{
		{Role: "system", Content: string(longContent)},
	}

	result := SerializeMessages(messages)
	if !containsString(result, "...") {
		t.Error("expected truncation indicator for system message")
	}
}

func TestSplitMessages(t *testing.T) {
	estimateFn := func(s string) int {
		return len(s)
	}

	tests := []struct {
		name          string
		messages      []types.Message
		keepTokens    int
		wantHeadLen   int
		wantRecentLen int
	}{
		{
			"empty",
			nil,
			100,
			0,
			0,
		},
		{
			"all recent",
			[]types.Message{{Content: "hi"}},
			100,
			0,
			1,
		},
		{
			"large keepTokens sends all to recent",
			[]types.Message{
				{Content: "hello"},
				{Content: "world"},
			},
			1000,
			0,
			2,
		},
		{
			"split",
			[]types.Message{
				{Content: "hello"},
				{Content: "world"},
				{Content: "foo"},
				{Content: "bar"},
			},
			10,
			2,
			2,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			head, recent := SplitMessages(tt.messages, tt.keepTokens, estimateFn)
			if len(head) != tt.wantHeadLen {
				t.Errorf("head length = %d, want %d", len(head), tt.wantHeadLen)
			}
			if len(recent) != tt.wantRecentLen {
				t.Errorf("recent length = %d, want %d", len(recent), tt.wantRecentLen)
			}
		})
	}
}

func TestSplitMessages_WithToolCalls(t *testing.T) {
	estimateFn := func(s string) int {
		return len(s)
	}

	messages := []types.Message{
		{Content: "hello", ToolCalls: []types.ToolCall{{Input: []byte("tool input")}}},
		{Content: "world"},
	}

	head, recent := SplitMessages(messages, 5, estimateFn)
	if len(head) != 1 {
		t.Errorf("head length = %d, want 1", len(head))
	}
	if len(recent) != 1 {
		t.Errorf("recent length = %d, want 1", len(recent))
	}
}

func TestTemplate(t *testing.T) {
	tmpl := Template()
	if tmpl == "" {
		t.Error("Template() returned empty string")
	}
	if !containsString(tmpl, "Goal") {
		t.Error("Template() missing Goal section")
	}
	if !containsString(tmpl, "Progress") {
		t.Error("Template() missing Progress section")
	}
}

func TestTruncate(t *testing.T) {
	tests := []struct {
		name   string
		input  string
		maxLen int
		want   string
	}{
		{"short", "hello", 10, "hello"},
		{"exact", "hello", 5, "hello"},
		{"long", "hello world", 5, "hello..."},
		{"empty", "", 5, ""},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got := truncate(tt.input, tt.maxLen)
			if got != tt.want {
				t.Errorf("truncate(%q, %d) = %q, want %q", tt.input, tt.maxLen, got, tt.want)
			}
		})
	}
}

func containsString(s, substr string) bool {
	return len(s) >= len(substr) && (s == substr || len(substr) == 0 || findSubstring(s, substr))
}

func findSubstring(s, substr string) bool {
	for i := 0; i <= len(s)-len(substr); i++ {
		if s[i:i+len(substr)] == substr {
			return true
		}
	}
	return false
}
