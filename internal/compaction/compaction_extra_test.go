package compaction

import (
	"context"
	"strings"
	"testing"

	"github.com/eshanized/M31A/internal/tokens"
	"github.com/eshanized/M31A/internal/types"
)

// --- SplitMessages edge cases ---

func TestSplitMessages_Empty(t *testing.T) {
	head, recent := SplitMessages(nil, 100, func(s string) int { return len(s) })
	if head != nil || recent != nil {
		t.Error("SplitMessages(nil) should return nil, nil")
	}
}

func TestSplitMessages_KeepTokensZero(t *testing.T) {
	msgs := []types.Message{
		{Role: "user", Content: "hello"},
		{Role: "assistant", Content: "hi"},
	}
	head, recent := SplitMessages(msgs, 0, func(s string) int { return len(s) })
	if len(head) != 1 {
		t.Errorf("head len = %d, want 1", len(head))
	}
	if len(recent) != 1 {
		t.Errorf("recent len = %d, want 1", len(recent))
	}
}

func TestSplitMessages_AllFitInKeepTokens(t *testing.T) {
	msgs := []types.Message{
		{Role: "user", Content: "a"},
		{Role: "assistant", Content: "b"},
	}
	head, recent := SplitMessages(msgs, 1000, func(s string) int { return len(s) })
	if head != nil {
		t.Errorf("head should be nil when all fit, got %d", len(head))
	}
	if len(recent) != 2 {
		t.Errorf("recent len = %d, want 2", len(recent))
	}
}

func TestSplitMessages_SingleLargeMessage(t *testing.T) {
	msgs := []types.Message{
		{Role: "user", Content: strings.Repeat("x", 2000)},
	}
	estimateFn := func(s string) int { return len(s) }
	head, recent := SplitMessages(msgs, 100, estimateFn)
	if head != nil {
		t.Error("head should be nil for single message")
	}
	if len(recent) != 1 {
		t.Errorf("recent len = %d, want 1", len(recent))
	}
}

func TestSplitMessages_ExactBoundary(t *testing.T) {
	msgs := []types.Message{
		{Role: "user", Content: "aaa"},
		{Role: "assistant", Content: "bbb"},
		{Role: "user", Content: "ccc"},
	}
	estimateFn := func(s string) int { return len(s) }
	head, recent := SplitMessages(msgs, 6, estimateFn)
	if len(head) != 1 {
		t.Errorf("head len = %d, want 1", len(head))
	}
	if len(recent) != 2 {
		t.Errorf("recent len = %d, want 2", len(recent))
	}
}

func TestSplitMessages_WithToolCalls_Extra(t *testing.T) {
	msgs := []types.Message{
		{Role: "user", Content: "hello"},
		{Role: "assistant", Content: "tool call", ToolCalls: []types.ToolCall{
			{Name: "Bash", Input: []byte(`{"command":"ls"}`)},
		}},
		{Role: "tool", Content: "file1.txt"},
		{Role: "assistant", Content: "done"},
	}
	estimateFn := func(s string) int { return len(s) }
	head, recent := SplitMessages(msgs, 20, estimateFn)
	if len(head)+len(recent) != 4 {
		t.Error("total messages should be 4")
	}
}

// --- SerializeMessages additional tests ---

func TestSerializeMessages_MixedRoles(t *testing.T) {
	msgs := []types.Message{
		{Role: "system", Content: "You are helpful"},
		{Role: "user", Content: "hello"},
		{Role: "assistant", Content: "hi there"},
		{Role: "tool", Content: "result"},
	}
	result := SerializeMessages(msgs)
	if !strings.Contains(result, "[System") {
		t.Error("should contain System")
	}
	if !strings.Contains(result, "[User") {
		t.Error("should contain User")
	}
	if !strings.Contains(result, "[Assistant") {
		t.Error("should contain Assistant")
	}
	if !strings.Contains(result, "[Tool") {
		t.Error("should contain Tool")
	}
}

func TestSerializeMessages_SkipForLLM(t *testing.T) {
	msgs := []types.Message{
		{Role: "user", Content: "keep this"},
		{Role: "assistant", Content: "skip this", SkipForLLM: true},
	}
	result := SerializeMessages(msgs)
	if !strings.Contains(result, "keep this") {
		t.Error("should contain non-skipped message")
	}
	if strings.Contains(result, "skip this") {
		t.Error("should not contain skipped message")
	}
}

func TestSerializeMessages_ToolCallInputTruncation(t *testing.T) {
	longInput := strings.Repeat("x", 600)
	msgs := []types.Message{
		{Role: "assistant", Content: "", ToolCalls: []types.ToolCall{
			{Name: "Bash", Input: []byte(longInput)},
		}},
	}
	result := SerializeMessages(msgs)
	if !strings.Contains(result, "...") {
		t.Error("long tool input should be truncated")
	}
}

// --- Template tests ---

func TestTemplate_Sections(t *testing.T) {
	tpl := Template()
	sections := []string{"Goal", "Progress", "Constraints", "Key Decisions", "Next Steps", "Critical Context", "Relevant Files"}
	for _, section := range sections {
		if !strings.Contains(tpl, section) {
			t.Errorf("template missing section %q", section)
		}
	}
}

// --- Config tests ---

func TestDefaultConfig_Extra(t *testing.T) {
	cfg := DefaultConfig()
	if cfg.Buffer <= 0 {
		t.Error("DefaultConfig buffer should be positive")
	}
	if cfg.KeepTokens <= 0 {
		t.Error("DefaultConfig keep tokens should be positive")
	}
}

func TestNewCompactor(t *testing.T) {
	cfg := DefaultConfig()
	c := New(cfg, nil)
	if c == nil {
		t.Error("New() returned nil")
	}
}

// --- Compactor edge cases ---

func TestCompactor_ShouldCompact_NilEstimator_Extra(t *testing.T) {
	c := New(DefaultConfig(), nil)
	if c.ShouldCompact(nil, 100000) {
		t.Error("nil estimator should not compact")
	}
}

func TestCompactor_Compact_EmptyMessages(t *testing.T) {
	cfg := DefaultConfig()
	est := tokens.NewEstimator("claude-3-5-sonnet-20241022")
	c := New(cfg, est)
	result, err := c.Compact(context.Background(), nil, nil, "")
	if err != nil {
		t.Fatalf("Compact(nil) error: %v", err)
	}
	if result.Compacted {
		t.Error("Compact(nil) should not compact")
	}
}

func TestCompactor_LastResult_AfterFailedCompact(t *testing.T) {
	est := tokens.NewEstimator("claude-3-5-sonnet-20241022")
	c := New(DefaultConfig(), est)
	_, _ = c.Compact(context.Background(), nil, nil, "")
	// LastResult may be nil if Compact returned early (no provider)
	_ = c.LastResult()
}

func TestCompactor_ShouldCompact_DisabledConfig(t *testing.T) {
	c := New(Config{Auto: false}, nil)
	if c.ShouldCompact(nil, 100000) {
		t.Error("disabled config should not compact")
	}
}
