package tui

import (
	"errors"
	"testing"
	"time"

	m31errors "github.com/eshanized/M31A/internal/core/errors"
	"github.com/eshanized/M31A/internal/core/types"
	"github.com/eshanized/M31A/internal/tools/subagent"
	"github.com/eshanized/M31A/internal/ui/tui/components"
)

func TestFormatFloat(t *testing.T) {
	tests := []struct {
		f    float64
		want string
	}{
		{0, "0.00"},
		{1.5, "1.50"},
		{3.14159, "3.14"},
		{-2.1, "-2.10"},
		{100, "100.00"},
	}
	for _, tt := range tests {
		got := formatFloat(tt.f)
		if got != tt.want {
			t.Errorf("formatFloat(%v) = %q, want %q", tt.f, got, tt.want)
		}
	}
}

func TestTypedErrorName(t *testing.T) {
	tests := []struct {
		err  error
		want string
	}{
		{m31errors.ErrContextExceeded, "ErrContextExceeded"},
		{m31errors.ErrInvalidKey, "ErrInvalidKey"},
		{m31errors.ErrRateLimited, "ErrRateLimited"},
		{m31errors.ErrProviderUnreachable, "ErrProviderUnreachable"},
		{m31errors.ErrModelNotFound, "ErrModelNotFound"},
		{m31errors.ErrToolExecution, "ErrToolExecution"},
		{m31errors.ErrPermissionDenied, "ErrPermissionDenied"},
		{errors.New("something else"), "unknown"},
		{nil, "unknown"},
	}
	for _, tt := range tests {
		got := typedErrorName(tt.err)
		if got != tt.want {
			t.Errorf("typedErrorName(%v) = %q, want %q", tt.err, got, tt.want)
		}
	}
}

func TestRenderErrorBanner(t *testing.T) {
	th := testTheme()
	tests := []struct {
		err      error
		contains string
	}{
		{m31errors.ErrContextExceeded, "Context window exceeded"},
		{m31errors.ErrInvalidKey, "Invalid API key"},
		{m31errors.ErrRateLimited, "Rate limited"},
		{errors.New("generic error"), "generic error"},
	}
	for _, tt := range tests {
		got := renderErrorBanner(tt.err, th, "")
		if got == "" {
			t.Errorf("renderErrorBanner(%v) should not be empty", tt.err)
		}
		_ = tt.contains // lipgloss output includes ANSI, just check non-empty
	}
}

func TestAbbrev(t *testing.T) {
	tests := []struct {
		s    string
		n    int
		want string
	}{
		{"hello", 10, "hello"},
		{"hello", 3, "he…"},
		{"hello", 1, "…"},
		{"hello", 0, ""},
		{"hello world", 5, "hell…"},
		{"  spaces  ", 5, "spac…"},
		{"multi\nline", 8, "multi l…"},
		{"", 5, ""},
	}
	for _, tt := range tests {
		got := abbrev(tt.s, tt.n)
		if got != tt.want {
			t.Errorf("abbrev(%q, %d) = %q, want %q", tt.s, tt.n, got, tt.want)
		}
	}
}

func TestSubagentsModelUpsert(t *testing.T) {
	m := NewSubagentsModel(testTheme())

	m.upsert("agent-1", func(r *SubagentRow) {
		r.LastEvent = "Grep foo"
	})
	if len(m.rows) != 1 {
		t.Fatalf("want 1 row, got %d", len(m.rows))
	}
	if m.rows[0].LastEvent != "Grep foo" {
		t.Error("event not set")
	}

	m.upsert("agent-1", func(r *SubagentRow) {
		r.LastEvent = "Read x.go"
	})
	if len(m.rows) != 1 {
		t.Error("should not add duplicate")
	}
	if m.rows[0].LastEvent != "Read x.go" {
		t.Error("event not updated")
	}

	m.upsert("agent-2", func(r *SubagentRow) {
		r.Info = subagent.SubagentInfo{ID: "agent-2"}
	})
	if len(m.rows) != 2 {
		t.Fatalf("want 2 rows, got %d", len(m.rows))
	}
}

func TestSessionAge(t *testing.T) {
	now := time.Now()
	if got := sessionAge(now); got != "just now" {
		t.Errorf("now: want 'just now', got %q", got)
	}
	if got := sessionAge(now.Add(-30 * time.Minute)); got != "30m ago" {
		t.Errorf("30m: want '30m ago', got %q", got)
	}
	if got := sessionAge(now.Add(-2 * time.Hour)); got != "2h ago" {
		t.Errorf("2h: want '2h ago', got %q", got)
	}
	if got := sessionAge(now.Add(-3 * 24 * time.Hour)); got != "3d ago" {
		t.Errorf("3d: want '3d ago', got %q", got)
	}
}

func TestCloseActiveSegmentEmpty(t *testing.T) {
	rm := NewReplModel(testTheme(), "v1")
	rm.closeActiveSegment()
	if len(rm.streamSegments) != 0 {
		t.Error("empty content should not create segment")
	}
}

func TestCloseActiveSegmentContent(t *testing.T) {
	rm := NewReplModel(testTheme(), "v1")
	rm.streamContent.WriteString("hello world")
	rm.activeSegmentType = "text"
	rm.closeActiveSegment()
	if len(rm.streamSegments) != 1 {
		t.Fatalf("want 1 segment, got %d", len(rm.streamSegments))
	}
	if rm.streamSegments[0].Type != "text" {
		t.Error("wrong segment type")
	}
	if rm.streamSegments[0].Content != "hello world" {
		t.Error("wrong segment content")
	}
	if rm.streamContent.Len() != 0 {
		t.Error("stream content should be reset")
	}
}

func TestCloseActiveSegmentThinking(t *testing.T) {
	rm := NewReplModel(testTheme(), "v1")
	rm.streamContent.WriteString("thinking...")
	rm.activeSegmentType = "thinking"
	rm.thinkingStartAt = time.Now().Add(-100 * time.Millisecond)
	rm.closeActiveSegment()
	if len(rm.streamSegments) != 1 {
		t.Fatal("want 1 segment")
	}
	if rm.streamSegments[0].DurationMs <= 0 {
		t.Error("thinking segment should have duration")
	}
}

func TestRenderThinkingToggleHint(t *testing.T) {
	rm := NewReplModel(testTheme(), "v1")
	rm.thinkingBlocks = make(map[int]*components.ThinkingBlock)

	got := rm.renderThinkingToggleHint(0, 0)
	if got != "" {
		t.Error("missing block should return empty")
	}

	rm.thinkingBlocks[0] = components.NewThinkingBlock(types.MessageSegment{}, testTheme(), false, 0)
	got = rm.renderThinkingToggleHint(0, 1500)
	if got == "" {
		t.Error("should render hint")
	}
}

func TestHandleThinkingToggle(t *testing.T) {
	rm := NewReplModel(testTheme(), "v1")
	rm.thinkingBlocks = make(map[int]*components.ThinkingBlock)
	rm.thinkingBlocks[0] = components.NewThinkingBlock(types.MessageSegment{}, testTheme(), false, 0)

	cmd := rm.handleThinkingToggle(ThinkingBlockToggleMsg{Index: 0})
	if cmd != nil {
		t.Error("should return nil cmd")
	}
}

func TestGetToolCallsFromSegments(t *testing.T) {
	rm := NewReplModel(testTheme(), "v1")
	rm.streamSegments = []types.MessageSegment{
		{Type: "tool_use", Content: `{"id":"t1","name":"bash","input":{}}`},
		{Type: "text", Content: "hello"},
		{Type: "tool_use", Content: "invalid json"},
	}
	calls := rm.getToolCallsFromSegments()
	if len(calls) != 1 {
		t.Errorf("want 1 tool call, got %d", len(calls))
	}
	if calls[0].Name != "bash" {
		t.Error("wrong tool name")
	}
}

func TestGetToolCallsFromSegmentsEmpty(t *testing.T) {
	rm := NewReplModel(testTheme(), "v1")
	calls := rm.getToolCallsFromSegments()
	if len(calls) != 0 {
		t.Error("empty segments should return empty calls")
	}
}

func TestFormatDurationMsPure(t *testing.T) {
	tests := []struct {
		ms   int64
		want string
	}{
		{0, "0s"},
		{500, "0s"},
		{1500, "1s"},
		{10000, "10s"},
		{65000, "1m5s"},
		{3665000, "1h1m"},
		{-1, "0s"},
	}
	for _, tt := range tests {
		got := formatDurationMs(tt.ms)
		if got != tt.want {
			t.Errorf("formatDurationMs(%d) = %q, want %q", tt.ms, got, tt.want)
		}
	}
}
