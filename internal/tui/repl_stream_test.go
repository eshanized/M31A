package tui

import (
	"errors"
	"testing"
	"time"

	"github.com/eshanized/M31A/internal/config"
	m31errors "github.com/eshanized/M31A/internal/errors"
	"github.com/eshanized/M31A/internal/tui/components"
	"github.com/eshanized/M31A/internal/tui/theme"
	"github.com/eshanized/M31A/internal/types"
)

// newTestReplModel creates a ReplModel suitable for unit testing stream handlers.
func newTestReplModel(t *testing.T) *ReplModel {
	t.Helper()
	th := theme.NewManager(theme.ModeDark).Current()
	rm := NewReplModel(th)
	rm.cfg = &config.Config{}
	rm.thinkingBlocks = make(map[int]*components.ThinkingBlock)
	rm.toolCards = make(map[int]*components.ToolCard)
	return &rm
}

// TestReplModel_AppendStreamChunk_Basic verifies basic chunk appending.
func TestReplModel_AppendStreamChunk_Basic(t *testing.T) {
	t.Parallel()
	rm := newTestReplModel(t)

	rm.AppendStreamChunk(&types.StreamChunk{Type: "content", Delta: "hello "})
	rm.AppendStreamChunk(&types.StreamChunk{Type: "content", Delta: "world"})

	if rm.streamContent.String() != "hello world" {
		t.Errorf("expected 'hello world', got %q", rm.streamContent.String())
	}
	if !rm.streaming {
		t.Error("expected streaming to be true")
	}
}

// TestReplModel_AppendStreamChunk_NilChunk verifies nil chunk is a no-op.
func TestReplModel_AppendStreamChunk_NilChunk(t *testing.T) {
	t.Parallel()
	rm := newTestReplModel(t)

	rm.AppendStreamChunk(nil)
	if rm.streamContent.Len() != 0 {
		t.Errorf("expected empty stream content, got %q", rm.streamContent.String())
	}
	if rm.streaming {
		t.Error("expected streaming to remain false")
	}
}

// TestReplModel_AppendStreamChunk_EmptyDelta verifies empty delta is a no-op.
func TestReplModel_AppendStreamChunk_EmptyDelta(t *testing.T) {
	t.Parallel()
	rm := newTestReplModel(t)

	rm.AppendStreamChunk(&types.StreamChunk{Type: "content", Delta: ""})
	if rm.streaming {
		t.Error("expected streaming to remain false for empty delta")
	}
}

// TestReplModel_CloseActiveSegment_Thinking verifies thinking segment is finalized with duration.
func TestReplModel_CloseActiveSegment_Thinking(t *testing.T) {
	t.Parallel()
	rm := newTestReplModel(t)
	rm.activeSegmentType = "thinking"
	rm.thinkingStartAt = time.Now().Add(-100 * time.Millisecond)
	rm.streamContent.WriteString("let me think...")

	rm.closeActiveSegment()

	if len(rm.streamSegments) != 1 {
		t.Fatalf("expected 1 segment, got %d", len(rm.streamSegments))
	}
	seg := rm.streamSegments[0]
	if seg.Type != "thinking" {
		t.Errorf("expected type 'thinking', got %q", seg.Type)
	}
	if seg.Content != "let me think..." {
		t.Errorf("expected content 'let me think...', got %q", seg.Content)
	}
	if seg.DurationMs < 50 {
		t.Errorf("expected DurationMs >= 50, got %d", seg.DurationMs)
	}
	if rm.streamContent.Len() != 0 {
		t.Error("expected streamContent to be reset")
	}
}

// TestReplModel_CloseActiveSegment_Content verifies content segment is finalized.
func TestReplModel_CloseActiveSegment_Content(t *testing.T) {
	t.Parallel()
	rm := newTestReplModel(t)
	rm.activeSegmentType = "content"
	rm.streamContent.WriteString("some response")

	rm.closeActiveSegment()

	if len(rm.streamSegments) != 1 {
		t.Fatalf("expected 1 segment, got %d", len(rm.streamSegments))
	}
	seg := rm.streamSegments[0]
	if seg.Type != "content" {
		t.Errorf("expected type 'content', got %q", seg.Type)
	}
	if seg.Content != "some response" {
		t.Errorf("expected content 'some response', got %q", seg.Content)
	}
	if !seg.Visible {
		t.Error("expected segment to be visible")
	}
}

// TestReplModel_CloseActiveSegment_EmptyContent verifies empty content doesn't create segment.
func TestReplModel_CloseActiveSegment_EmptyContent(t *testing.T) {
	t.Parallel()
	rm := newTestReplModel(t)
	rm.activeSegmentType = "content"
	// streamContent is empty

	rm.closeActiveSegment()

	if len(rm.streamSegments) != 0 {
		t.Errorf("expected 0 segments for empty content, got %d", len(rm.streamSegments))
	}
}

// TestReplModel_CloseActiveSegment_MultipleSegments verifies segments accumulate.
func TestReplModel_CloseActiveSegment_MultipleSegments(t *testing.T) {
	t.Parallel()
	rm := newTestReplModel(t)

	// First: thinking segment
	rm.activeSegmentType = "thinking"
	rm.thinkingStartAt = time.Now()
	rm.streamContent.WriteString("thinking part")
	rm.closeActiveSegment()

	// Second: content segment
	rm.activeSegmentType = "content"
	rm.streamContent.WriteString("content part")
	rm.closeActiveSegment()

	if len(rm.streamSegments) != 2 {
		t.Fatalf("expected 2 segments, got %d", len(rm.streamSegments))
	}
	if rm.streamSegments[0].Type != "thinking" {
		t.Errorf("expected first segment type 'thinking', got %q", rm.streamSegments[0].Type)
	}
	if rm.streamSegments[1].Type != "content" {
		t.Errorf("expected second segment type 'content', got %q", rm.streamSegments[1].Type)
	}
}

// TestReplModel_HandleStreamDoneMsg_Basic verifies stream finalization.
func TestReplModel_HandleStreamDoneMsg_Basic(t *testing.T) {
	t.Parallel()
	rm := newTestReplModel(t)

	// Build some segments first
	rm.activeSegmentType = "thinking"
	rm.thinkingStartAt = time.Now().Add(-50 * time.Millisecond)
	rm.streamContent.WriteString("thinking...")
	rm.closeActiveSegment()

	rm.activeSegmentType = "content"
	rm.streamContent.WriteString("response")
	rm.closeActiveSegment()

	// Create a StreamDoneMsg
	doneMsg := StreamDoneMsg{
		Message: types.Message{
			Role:    "assistant",
			Content: "thinking...response",
		},
	}

	cmds, updated := rm.handleStreamDoneMsg(doneMsg)

	if !updated {
		t.Error("expected updated=true")
	}
	if len(cmds) != 0 {
		t.Errorf("expected 0 cmds, got %d", len(cmds))
	}
	if rm.streaming {
		t.Error("expected streaming to be false after done")
	}
	if rm.thinking {
		t.Error("expected thinking to be false after done")
	}
	if rm.activeSegmentType != "" {
		t.Errorf("expected activeSegmentType empty, got %q", rm.activeSegmentType)
	}
	if len(rm.streamSegments) != 0 {
		t.Error("expected streamSegments to be nil after done")
	}
	// Messages should contain the finalized message
	if len(rm.messages) != 1 {
		t.Fatalf("expected 1 message, got %d", len(rm.messages))
	}
	if rm.messages[0].Role != "assistant" {
		t.Errorf("expected role 'assistant', got %q", rm.messages[0].Role)
	}
}

// TestReplModel_HandleStreamDoneMsg_WithUsage verifies usage tracking.
func TestReplModel_HandleStreamDoneMsg_WithUsage(t *testing.T) {
	t.Parallel()
	rm := newTestReplModel(t)

	doneMsg := StreamDoneMsg{
		Message: types.Message{Role: "assistant", Content: "done"},
		Usage: &types.Usage{
			PromptTokens:     100,
			CompletionTokens: 50,
			TotalTokens:      150,
		},
	}

	_, _ = rm.handleStreamDoneMsg(doneMsg)

	if rm.lastUsage == nil {
		t.Fatal("expected lastUsage to be set")
	}
	if rm.lastUsage.TotalTokens != 150 {
		t.Errorf("expected 150 total tokens, got %d", rm.lastUsage.TotalTokens)
	}
}

// TestReplModel_HandleStreamDoneMsg_ThinkingBlocks verifies thinking blocks are populated.
func TestReplModel_HandleStreamDoneMsg_ThinkingBlocks(t *testing.T) {
	t.Parallel()
	rm := newTestReplModel(t)

	// Build a thinking segment
	rm.activeSegmentType = "thinking"
	rm.thinkingStartAt = time.Now().Add(-100 * time.Millisecond)
	rm.streamContent.WriteString("thinking block content")
	rm.closeActiveSegment()

	doneMsg := StreamDoneMsg{
		Message: types.Message{Role: "assistant", Content: "thinking block content"},
	}

	_, _ = rm.handleStreamDoneMsg(doneMsg)

	if len(rm.thinkingBlocks) != 1 {
		t.Fatalf("expected 1 thinking block, got %d", len(rm.thinkingBlocks))
	}
}

// TestReplModel_HandleStreamErrorMsg_Basic verifies error message handling.
func TestReplModel_HandleStreamErrorMsg_Basic(t *testing.T) {
	t.Parallel()
	rm := newTestReplModel(t)

	errMsg := StreamErrorMsg{Err: errors.New("connection failed")}

	cmds, updated := rm.handleStreamErrorMsg(errMsg)

	if !updated {
		t.Error("expected updated=true")
	}
	if len(cmds) != 0 {
		t.Errorf("expected 0 cmds, got %d", len(cmds))
	}
	if rm.streaming {
		t.Error("expected streaming to be false after error")
	}
	if rm.thinking {
		t.Error("expected thinking to be false after error")
	}
	// Error message should be added to messages
	if len(rm.messages) != 1 {
		t.Fatalf("expected 1 error message, got %d", len(rm.messages))
	}
	if rm.messages[0].Role != "assistant" {
		t.Errorf("expected role 'assistant', got %q", rm.messages[0].Role)
	}
}

// TestReplModel_HandleStreamErrorMsg_ResetsState verifies all streaming state is reset on error.
func TestReplModel_HandleStreamErrorMsg_ResetsState(t *testing.T) {
	t.Parallel()
	rm := newTestReplModel(t)

	// Set up some streaming state
	rm.streaming = true
	rm.thinking = true
	rm.activeSegmentType = "thinking"
	rm.streamContent.WriteString("partial content")
	rm.streamSegments = append(rm.streamSegments, types.MessageSegment{
		Type:    "thinking",
		Content: "partial",
	})

	_, _ = rm.handleStreamErrorMsg(StreamErrorMsg{Err: errors.New("fail")})

	if rm.streaming {
		t.Error("expected streaming=false")
	}
	if rm.thinking {
		t.Error("expected thinking=false")
	}
	if rm.activeSegmentType != "" {
		t.Errorf("expected activeSegmentType empty, got %q", rm.activeSegmentType)
	}
	if rm.streamContent.Len() != 0 {
		t.Error("expected streamContent reset")
	}
	if len(rm.streamSegments) != 0 {
		t.Error("expected streamSegments nil")
	}
}

// TestReplModel_HandleStreamMsg_ContentChunk verifies content chunk handling.
func TestReplModel_HandleStreamMsg_ContentChunk(t *testing.T) {
	t.Parallel()
	rm := newTestReplModel(t)

	msg := StreamMsg{Chunk: &types.StreamChunk{Type: "content", Delta: "hello"}}
	cmds, _ := rm.handleStreamMsg(msg)

	if len(cmds) != 1 {
		t.Fatalf("expected 1 continuation cmd, got %d", len(cmds))
	}
	if rm.streamContent.String() != "hello" {
		t.Errorf("expected 'hello', got %q", rm.streamContent.String())
	}
	if rm.activeSegmentType != "content" {
		t.Errorf("expected activeSegmentType 'content', got %q", rm.activeSegmentType)
	}
	if rm.thinking {
		t.Error("expected thinking=false for content chunk")
	}
}

// TestReplModel_HandleStreamMsg_ThinkingChunk verifies thinking chunk handling.
func TestReplModel_HandleStreamMsg_ThinkingChunk(t *testing.T) {
	t.Parallel()
	rm := newTestReplModel(t)

	msg := StreamMsg{Chunk: &types.StreamChunk{Type: "thinking", Delta: "reasoning..."}}
	cmds, _ := rm.handleStreamMsg(msg)

	if len(cmds) != 1 {
		t.Fatalf("expected 1 continuation cmd, got %d", len(cmds))
	}
	if rm.streamContent.String() != "reasoning..." {
		t.Errorf("expected 'reasoning...', got %q", rm.streamContent.String())
	}
	if rm.activeSegmentType != "thinking" {
		t.Errorf("expected activeSegmentType 'thinking', got %q", rm.activeSegmentType)
	}
	if !rm.thinking {
		t.Error("expected thinking=true for thinking chunk")
	}
	if rm.thinkingStartAt.IsZero() {
		t.Error("expected thinkingStartAt to be set")
	}
}

// TestReplModel_HandleStreamMsg_SegmentTransition_ThinkingToContent verifies segment closing on type switch.
func TestReplModel_HandleStreamMsg_SegmentTransition_ThinkingToContent(t *testing.T) {
	t.Parallel()
	rm := newTestReplModel(t)

	// Start with thinking
	thinkingMsg := StreamMsg{Chunk: &types.StreamChunk{Type: "thinking", Delta: "thinking..."}}
	_, _ = rm.handleStreamMsg(thinkingMsg)

	// Switch to content
	contentMsg := StreamMsg{Chunk: &types.StreamChunk{Type: "content", Delta: "content..."}}
	_, _ = rm.handleStreamMsg(contentMsg)

	// The thinking segment should have been closed and added to streamSegments
	if len(rm.streamSegments) != 1 {
		t.Fatalf("expected 1 closed segment, got %d", len(rm.streamSegments))
	}
	if rm.streamSegments[0].Type != "thinking" {
		t.Errorf("expected closed segment type 'thinking', got %q", rm.streamSegments[0].Type)
	}
	if rm.streamContent.String() != "content..." {
		t.Errorf("expected new content 'content...', got %q", rm.streamContent.String())
	}
}

// TestReplModel_HandleStreamMsg_SegmentTransition_ContentToThinking verifies segment closing on type switch.
func TestReplModel_HandleStreamMsg_SegmentTransition_ContentToThinking(t *testing.T) {
	t.Parallel()
	rm := newTestReplModel(t)

	// Start with content
	contentMsg := StreamMsg{Chunk: &types.StreamChunk{Type: "content", Delta: "content..."}}
	_, _ = rm.handleStreamMsg(contentMsg)

	// Switch to thinking
	thinkingMsg := StreamMsg{Chunk: &types.StreamChunk{Type: "thinking", Delta: "thinking..."}}
	_,_ = rm.handleStreamMsg(thinkingMsg)

	// The content segment should have been closed
	if len(rm.streamSegments) != 1 {
		t.Fatalf("expected 1 closed segment, got %d", len(rm.streamSegments))
	}
	if rm.streamSegments[0].Type != "content" {
		t.Errorf("expected closed segment type 'content', got %q", rm.streamSegments[0].Type)
	}
}

// TestReplModel_HandleStreamMsg_NilChunk verifies nil chunk is handled gracefully.
func TestReplModel_HandleStreamMsg_NilChunk(t *testing.T) {
	t.Parallel()
	rm := newTestReplModel(t)

	msg := StreamMsg{Chunk: nil}
	cmds, updated := rm.handleStreamMsg(msg)

	if updated {
		t.Error("expected updated=false for nil chunk")
	}
	if len(cmds) != 0 {
		t.Errorf("expected 0 cmds, got %d", len(cmds))
	}
}

// TestReplModel_HandleStreamMsg_DoneChunk verifies done chunk type.
func TestReplModel_HandleStreamMsg_DoneChunk(t *testing.T) {
	t.Parallel()
	rm := newTestReplModel(t)

	msg := StreamMsg{Chunk: &types.StreamChunk{Type: "done", Delta: ""}}
	cmds, _ := rm.handleStreamMsg(msg)

	if len(cmds) != 1 {
		t.Fatalf("expected 1 continuation cmd, got %d", len(cmds))
	}
}

// TestRenderErrorBanner verifies error banner rendering for different sentinel errors.
func TestRenderErrorBanner(t *testing.T) {
	t.Parallel()
	th := theme.NewManager(theme.ModeDark).Current()

	tests := []struct {
		name     string
		err      error
		contains string
	}{
		{"context exceeded", m31errors.ErrContextExceeded, "Context window exceeded"},
		{"invalid key", m31errors.ErrInvalidKey, "Invalid API key"},
		{"rate limited", m31errors.ErrRateLimited, "Rate limited"},
		{"generic error", errors.New("something"), "something"},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			t.Parallel()
			banner := renderErrorBanner(tt.err, th)
			if banner == "" {
				t.Error("expected non-empty banner")
			}
		})
	}
}

// TestTypedErrorName verifies sentinel error name extraction.
func TestTypedErrorName(t *testing.T) {
	t.Parallel()

	tests := []struct {
		name string
		err  error
		want string
	}{
		{"context exceeded", m31errors.ErrContextExceeded, "ErrContextExceeded"},
		{"invalid key", m31errors.ErrInvalidKey, "ErrInvalidKey"},
		{"rate limited", m31errors.ErrRateLimited, "ErrRateLimited"},
		{"provider unreachable", m31errors.ErrProviderUnreachable, "ErrProviderUnreachable"},
		{"model not found", m31errors.ErrModelNotFound, "ErrModelNotFound"},
		{"tool execution", m31errors.ErrToolExecution, "ErrToolExecution"},
		{"permission denied", m31errors.ErrPermissionDenied, "ErrPermissionDenied"},
		{"unknown", errors.New("something"), "unknown"},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			t.Parallel()
			got := typedErrorName(tt.err)
			if got != tt.want {
				t.Errorf("expected %q, got %q", tt.want, got)
			}
		})
	}
}

// TestReplModel_GetToolCallsFromSegments verifies tool call extraction from segments.
func TestReplModel_GetToolCallsFromSegments(t *testing.T) {
	t.Parallel()
	rm := newTestReplModel(t)

	rm.streamSegments = []types.MessageSegment{
		{Type: "content", Content: "some text"},
		{Type: "tool_use", Content: `{"id":"tc-1","name":"Bash","input":{"command":"echo hi"}}`},
		{Type: "thinking", Content: "hmm"},
		{Type: "tool_use", Content: `{"id":"tc-2","name":"FileRead","input":{"path":"foo.go"}}`},
	}

	calls := rm.getToolCallsFromSegments()

	if len(calls) != 2 {
		t.Fatalf("expected 2 tool calls, got %d", len(calls))
	}
	if calls[0].Name != "Bash" {
		t.Errorf("expected first call 'Bash', got %q", calls[0].Name)
	}
	if calls[1].Name != "FileRead" {
		t.Errorf("expected second call 'FileRead', got %q", calls[1].Name)
	}
}

// TestReplModel_GetToolCallsFromSegments_Empty verifies no tool calls returns empty slice.
func TestReplModel_GetToolCallsFromSegments_Empty(t *testing.T) {
	t.Parallel()
	rm := newTestReplModel(t)

	rm.streamSegments = []types.MessageSegment{
		{Type: "content", Content: "just text"},
	}

	calls := rm.getToolCallsFromSegments()
	if len(calls) != 0 {
		t.Errorf("expected 0 tool calls, got %d", len(calls))
	}
}

// TestReplModel_GetToolCallsFromSegments_InvalidJSON verifies invalid JSON tool calls are skipped.
func TestReplModel_GetToolCallsFromSegments_InvalidJSON(t *testing.T) {
	t.Parallel()
	rm := newTestReplModel(t)

	rm.streamSegments = []types.MessageSegment{
		{Type: "tool_use", Content: "not valid json{"},
	}

	calls := rm.getToolCallsFromSegments()
	if len(calls) != 0 {
		t.Errorf("expected 0 tool calls for invalid JSON, got %d", len(calls))
	}
}

// TestReplModel_StreamTickCmds verifies stream tick returns continuation cmd.
func TestReplModel_StreamTickCmds(t *testing.T) {
	t.Parallel()
	rm := newTestReplModel(t)

	cmds, updated := rm.streamTickCmds()
	if updated {
		t.Error("expected updated=false for tick")
	}
	if len(cmds) != 1 {
		t.Fatalf("expected 1 cmd, got %d", len(cmds))
	}
}
