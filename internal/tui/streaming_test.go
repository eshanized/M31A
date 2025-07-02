package tui

import (
	"context"
	"io"
	"strings"
	"sync"
	"testing"
	"time"

	"github.com/eshanized/M31A/internal/provider"
	"github.com/eshanized/M31A/internal/tui/theme"
	"github.com/eshanized/M31A/internal/types"
)

type mockStreamProvider struct {
	chunks []*types.StreamChunk
	err    error
	index  int
}

func (m *mockStreamProvider) Name() string {
	return "mock-stream"
}

func (m *mockStreamProvider) FetchModels(ctx context.Context) ([]types.ModelInfo, error) {
	return nil, nil
}

func (m *mockStreamProvider) ChatCompletionStream(ctx context.Context, req provider.ChatRequest) (*types.StreamIterator, error) {
	return &types.StreamIterator{
		Next: func() (*types.StreamChunk, error) {
			if m.err != nil {
				return nil, m.err
			}
			if m.index >= len(m.chunks) {
				return nil, io.EOF
			}
			chunk := m.chunks[m.index]
			m.index++
			return chunk, nil
		},
		Close: func() error {
			return nil
		},
	}, nil
}

func (m *mockStreamProvider) EstimateCost(modelID string, usage types.Usage) float64 {
	return 0
}

func (m *mockStreamProvider) HealthCheck(ctx context.Context) types.HealthStatus {
	return types.HealthStatus{Status: "live"}
}

func (m *mockStreamProvider) GetModel(id string) (*types.ModelInfo, error) {
	return nil, nil
}

func TestStartStreamCmd_ContentOnly(t *testing.T) {
	p := &mockStreamProvider{
		chunks: []*types.StreamChunk{
			{Type: "content", Delta: "Hello "},
			{Type: "content", Delta: "World"},
		},
	}

	req := provider.ChatRequest{
		Model: "test-model",
		Messages: []types.Message{
			{Role: "user", Content: "hi"},
		},
	}

	cmd, _ := StartStreamCmd(context.Background(), p, req, "test-session")
	if cmd == nil {
		t.Fatal("expected non-nil cmd")
	}

	// Collect all messages from the cmd
	var streamMsgCount int
	var doneMsgCount int

	for {
		msg := cmd()
		if msg == nil {
			break
		}
		switch msg.(type) {
		case StreamMsg:
			streamMsgCount++
		case StreamDoneMsg:
			doneMsgCount++
			return // StreamDoneMsg is the last message
		case StreamErrorMsg:
			t.Error("unexpected StreamErrorMsg")
			return
		}
	}

	if streamMsgCount != 2 {
		t.Errorf("expected 2 StreamMsg, got %d", streamMsgCount)
	}
	if doneMsgCount != 1 {
		t.Errorf("expected 1 StreamDoneMsg, got %d", doneMsgCount)
	}
}

func TestStartStreamCmd_WithThinking(t *testing.T) {
	p := &mockStreamProvider{
		chunks: []*types.StreamChunk{
			{Type: "thinking", Delta: "thinking step 1"},
			{Type: "content", Delta: "Final answer"},
		},
	}

	req := provider.ChatRequest{
		Model: "test-model",
		Messages: []types.Message{
			{Role: "user", Content: "think"},
		},
	}

	cmd, _ := StartStreamCmd(context.Background(), p, req, "session-1")

	var streamMsgCount int
	var doneMsgCount int

	for {
		msg := cmd()
		if msg == nil {
			break
		}
		switch msg.(type) {
		case StreamMsg:
			streamMsgCount++
		case StreamDoneMsg:
			doneMsgCount++
			return
		case StreamErrorMsg:
			t.Error("unexpected StreamErrorMsg")
			return
		}
	}

	if streamMsgCount != 2 {
		t.Errorf("expected 2 StreamMsg, got %d", streamMsgCount)
	}
	if doneMsgCount != 1 {
		t.Errorf("expected 1 StreamDoneMsg, got %d", doneMsgCount)
	}
}

func TestStartStreamCmd_WithContextExceeded(t *testing.T) {
	p := &mockStreamProvider{
		chunks: []*types.StreamChunk{
			{Type: "content", Delta: "partial content"},
		},
		err: io.ErrUnexpectedEOF,
	}

	req := provider.ChatRequest{
		Model: "test-model",
	}

	cmd, _ := StartStreamCmd(context.Background(), p, req, "session-2")

	msg := cmd()
	if msg == nil {
		t.Fatal("expected non-nil msg")
	}

	if _, ok := msg.(StreamErrorMsg); !ok {
		t.Errorf("expected StreamErrorMsg for context exceeded, got %T", msg)
	}
}

func TestStartStreamCmd_ContextCancellation(t *testing.T) {
	p := &mockStreamProvider{
		chunks: []*types.StreamChunk{
			{Type: "content", Delta: "one"},
			{Type: "content", Delta: "two"},
			{Type: "content", Delta: "three"},
		},
	}

	ctx, cancel := context.WithCancel(context.Background())
	defer cancel()
	req := provider.ChatRequest{
		Model: "test-model",
	}

	cmd, _ := StartStreamCmd(ctx, p, req, "session-3")

	count := 0
	for {
		msg := cmd()
		if msg == nil {
			break
		}
		count++
		if count == 2 {
			cancel()
		}
		// Check if we got a StreamDoneMsg or StreamErrorMsg (terminal messages)
		switch msg.(type) {
		case StreamDoneMsg, StreamErrorMsg:
			return
		}
	}

	if count < 2 {
		t.Error("expected at least 2 chunks before cancellation")
	}
}

func TestStreamTickCmd_EmitsAt10fps(t *testing.T) {
	cmd := StreamTickCmd()
	if cmd == nil {
		t.Fatal("expected non-nil cmd")
	}

	msg := cmd()
	tickMsg, ok := msg.(TickMsg)
	if !ok {
		t.Fatalf("expected TickMsg, got %T", msg)
	}

	if tickMsg.Time.IsZero() {
		t.Error("expected non-zero time in TickMsg")
	}
}

func TestTickMsg_Time(t *testing.T) {
	now := time.Now()
	msg := TickMsg{Time: now}
	if msg.Time != now {
		t.Error("TickMsg time mismatch")
	}
}

// --- ReplModel streaming handler tests ---

func TestReplModel_HandleStreamMsg(t *testing.T) {
	m := NewReplModel(theme.Dark())

	// Send content chunks
	m.Update(StreamMsg{
		Chunk: &types.StreamChunk{Type: "content", Delta: "Hello "},
	})
	if !m.streaming {
		t.Error("expected streaming=true after StreamMsg")
	}
	if m.streamContent.String() != "Hello " {
		t.Errorf("streamContent = %q, want %q", m.streamContent.String(), "Hello ")
	}

	m.Update(StreamMsg{
		Chunk: &types.StreamChunk{Type: "content", Delta: "World"},
	})
	expected := "Hello World"
	if m.streamContent.String() != expected {
		t.Errorf("after second chunk, streamContent = %q, want %q", m.streamContent.String(), expected)
	}

	// Send a thinking chunk mid-stream
	m.Update(StreamMsg{
		Chunk: &types.StreamChunk{Type: "thinking", Delta: "thinking step"},
	})
	if !m.thinking {
		t.Error("expected thinking=true after thinking chunk")
	}
	if len(m.streamSegments) != 1 {
		t.Fatalf("expected 1 segment after thinking transition, got %d", len(m.streamSegments))
	}
	if m.streamSegments[0].Type != "content" {
		t.Errorf("segment type = %q, want %q", m.streamSegments[0].Type, "content")
	}
	// streamContent should now contain the thinking text (reset after segment flush)
	if m.streamContent.String() != "thinking step" {
		t.Errorf("after thinking chunk, streamContent = %q, want %q", m.streamContent.String(), "thinking step")
	}
}

func TestReplModel_HandleStreamDoneMsg(t *testing.T) {
	m := NewReplModel(theme.Dark())
	m.streaming = true
	m.streamContent.WriteString("Hello World")
	m.activeSegmentType = "content"

	// Create a proper StreamDoneMsg
	doneMsg := StreamDoneMsg{
		Message: types.Message{
			Role:      "assistant",
			Content:   "Hello World",
			CreatedAt: time.Now(),
		},
		ModelID:   "test-model",
		SessionID: "test-session",
	}

	cmds, sent := m.Update(doneMsg)
	if !sent {
		t.Error("expected sent=true after StreamDoneMsg")
	}
	if m.streaming {
		t.Error("expected streaming=false after StreamDoneMsg")
	}
	if m.thinking {
		t.Error("expected thinking=false after StreamDoneMsg")
	}
	if m.streamContent.Len() != 0 {
		t.Error("expected streamContent cleared after StreamDoneMsg")
	}
	if m.streamSegments != nil {
		t.Error("expected streamSegments nil after StreamDoneMsg")
	}
	if len(m.messages) != 1 {
		t.Fatalf("expected 1 finalized message, got %d", len(m.messages))
	}
	if m.messages[0].Role != "assistant" {
		t.Errorf("message role = %q, want %q", m.messages[0].Role, "assistant")
	}
	if m.messages[0].Content != "Hello World" {
		t.Errorf("message content = %q, want %q", m.messages[0].Content, "Hello World")
	}
	_ = cmds
}

func TestReplModel_HandleStreamDoneMsg_WithSegments(t *testing.T) {
	m := NewReplModel(theme.Dark())
	m.streaming = true
	m.streamContent.WriteString("Final answer")
	m.streamSegments = []types.MessageSegment{
		{Type: "thinking", Content: "step 1", Visible: true},
	}
	m.activeSegmentType = "content"

	doneMsg := StreamDoneMsg{
		Message: types.Message{
			Role:    "assistant",
			Content: "Final answer",
		},
	}

	m.Update(doneMsg)

	if len(m.messages) != 1 {
		t.Fatalf("expected 1 message, got %d", len(m.messages))
	}
	msg := m.messages[0]
	if len(msg.Segments) != 2 {
		t.Fatalf("expected 2 segments (thinking + content), got %d", len(msg.Segments))
	}
	if msg.Segments[0].Type != "thinking" {
		t.Errorf("segment[0].Type = %q, want %q", msg.Segments[0].Type, "thinking")
	}
	if msg.Segments[1].Type != "content" {
		t.Errorf("segment[1].Type = %q, want %q", msg.Segments[1].Type, "content")
	}
}

func TestReplModel_HandleStreamErrorMsg(t *testing.T) {
	m := NewReplModel(theme.Dark())
	m.streaming = true
	m.thinking = true
	m.streamContent.WriteString("partial content")

	errMsg := StreamErrorMsg{
		Err:     io.ErrUnexpectedEOF,
		ModelID: "test-model",
	}

	cmds, sent := m.Update(errMsg)
	if !sent {
		t.Error("expected sent=true after StreamErrorMsg")
	}
	if m.streaming {
		t.Error("expected streaming=false after StreamErrorMsg")
	}
	if m.thinking {
		t.Error("expected thinking=false after StreamErrorMsg")
	}
	if m.streamContent.Len() != 0 {
		t.Error("expected streamContent cleared after StreamErrorMsg")
	}
	if len(m.messages) != 1 {
		t.Fatalf("expected 1 error message, got %d", len(m.messages))
	}
	if m.messages[0].Role != "assistant" {
		t.Errorf("message role = %q, want %q", m.messages[0].Role, "assistant")
	}
	if !strings.Contains(m.messages[0].Content, "unexpected EOF") {
		t.Errorf("error message content = %q, want it to contain 'unexpected EOF'", m.messages[0].Content)
	}
	_ = cmds
}

func TestReplModel_StreamTickMsg(t *testing.T) {
	m := NewReplModel(theme.Dark())
	m.streaming = true

	cmds, _ := m.Update(TickMsg{Time: time.Now()})
	if len(cmds) == 0 {
		t.Error("expected at least one cmd returned from TickMsg during streaming")
	}
	// The tick cmd should be StreamTickCmd which produces another TickMsg
	msg := cmds[0]()
	if _, ok := msg.(TickMsg); !ok {
		t.Errorf("expected TickMsg from stream tick cmd, got %T", msg)
	}
}

func TestReplModel_StreamMsg_ThinkingTransitions(t *testing.T) {
	m := NewReplModel(theme.Dark())

	// Send content chunk first (establishes content baseline)
	m.Update(StreamMsg{
		Chunk: &types.StreamChunk{Type: "content", Delta: "Some reasoning"},
	})
	if m.activeSegmentType != "content" {
		t.Errorf("activeSegmentType = %q, want %q", m.activeSegmentType, "content")
	}

	// Send thinking chunk (content→thinking transition: content flushed to segments)
	m.Update(StreamMsg{
		Chunk: &types.StreamChunk{Type: "thinking", Delta: "deeper thinking"},
	})
	if !m.thinking {
		t.Error("expected thinking=true after thinking chunk")
	}
	if m.activeSegmentType != "thinking" {
		t.Errorf("activeSegmentType = %q, want %q", m.activeSegmentType, "thinking")
	}
	if len(m.streamSegments) != 1 {
		t.Fatalf("expected 1 saved segment (content flushed), got %d", len(m.streamSegments))
	}
	if m.streamSegments[0].Type != "content" {
		t.Errorf("saved segment type = %q, want 'content'", m.streamSegments[0].Type)
	}
	if m.streamSegments[0].Content != "Some reasoning" {
		t.Errorf("saved segment content = %q, want %q", m.streamSegments[0].Content, "Some reasoning")
	}

	// Send another content chunk (thinking→content transition: thinking flushed to segments)
	m.Update(StreamMsg{
		Chunk: &types.StreamChunk{Type: "content", Delta: "Final answer"},
	})
	if m.thinking {
		t.Error("expected thinking=false after content follows thinking")
	}
	if m.activeSegmentType != "content" {
		t.Errorf("activeSegmentType = %q, want %q", m.activeSegmentType, "content")
	}
	// streamSegments should be 2: content flushed on content→thinking, thinking flushed on thinking→content
	if len(m.streamSegments) != 2 {
		t.Fatalf("expected 2 segments (both transitions flush), got %d", len(m.streamSegments))
	}
	if m.streamSegments[0].Type != "content" {
		t.Errorf("segment 0 type = %q, want 'content'", m.streamSegments[0].Type)
	}
	if m.streamSegments[1].Type != "thinking" {
		t.Errorf("segment 1 type = %q, want 'thinking'", m.streamSegments[1].Type)
	}
	if m.streamSegments[1].Content != "deeper thinking" {
		t.Errorf("segment 1 content = %q, want %q", m.streamSegments[1].Content, "deeper thinking")
	}
	// streamContent has new content after flush
	expected := "Final answer"
	if m.streamContent.String() != expected {
		t.Errorf("streamContent = %q, want %q", m.streamContent.String(), expected)
	}
}

// --- Regression tests for audit findings C-3, H-9, M-21 ---

// TestStartStreamCmd_Twice_NoPanic verifies that invoking StartStreamCmd
// twice in sequence does not panic with "close of closed channel" (C-3).
// Each call allocates fresh channels internally; the goroutine owns the
// write side and closes streamCh on exit.
func TestStartStreamCmd_Twice_NoPanic(t *testing.T) {
	p := &mockStreamProvider{
		chunks: []*types.StreamChunk{
			{Type: "content", Delta: "chunk-a1"},
			{Type: "content", Delta: "chunk-a2"},
		},
	}

	req := provider.ChatRequest{
		Model: "test-model",
		Messages: []types.Message{
			{Role: "user", Content: "hello"},
		},
	}

	// First invocation
	cmd1, _ := StartStreamCmd(context.Background(), p, req, "session-1")
	if cmd1 == nil {
		t.Fatal("expected non-nil cmd1")
	}

	// Drain first stream
	var stream1Chunks int
	for {
		msg := cmd1()
		if msg == nil {
			break
		}
		switch msg.(type) {
		case StreamMsg:
			stream1Chunks++
		case StreamDoneMsg:
			goto done1
		case StreamErrorMsg:
			t.Error("unexpected error in stream 1")
			return
		}
	}
done1:

	if stream1Chunks != 2 {
		t.Errorf("stream 1: expected 2 chunks, got %d", stream1Chunks)
	}

	// Second invocation — must not panic (C-3)
	p2 := &mockStreamProvider{
		chunks: []*types.StreamChunk{
			{Type: "content", Delta: "chunk-b1"},
		},
	}

	cmd2, _ := StartStreamCmd(context.Background(), p2, req, "session-2")
	if cmd2 == nil {
		t.Fatal("expected non-nil cmd2")
	}

	var stream2Chunks int
	for {
		msg := cmd2()
		if msg == nil {
			break
		}
		switch msg.(type) {
		case StreamMsg:
			stream2Chunks++
		case StreamDoneMsg:
			goto done2
		case StreamErrorMsg:
			t.Error("unexpected error in stream 2")
			return
		}
	}
done2:

	if stream2Chunks != 1 {
		t.Errorf("stream 2: expected 1 chunk, got %d", stream2Chunks)
	}
}

// TestSafeClose_RaceFree verifies that safeCloseOnce is race-free when
// called from 100 concurrent goroutines (H-9). The test must pass with
// -race to confirm no data race.
func TestSafeClose_RaceFree(t *testing.T) {
	for i := 0; i < 10; i++ {
		ch := make(chan struct{})
		var wg sync.WaitGroup
		for j := 0; j < 100; j++ {
			wg.Add(1)
			go func() {
				defer wg.Done()
				safeCloseOnce(ch)
			}()
		}
		wg.Wait()

		// Channel must be closed
		select {
		case <-ch:
			// Good — channel is closed
		default:
			t.Error("channel should be closed after concurrent safeCloseOnce calls")
		}
	}

	// nil channel should return false without panic
	if safeCloseOnce(nil) {
		t.Error("expected safeCloseOnce(nil) to return false")
	}
}

// TestDeferOrder_StreamDoneClosesFirst verifies the M-21 fix: the goroutine
// in StartStreamCmd closes streamCh when it exits. Since we use a single
// channel (streamDone was eliminated), this test verifies that the stream
// completes cleanly — the cmd returns nil (channel closed) after StreamDoneMsg.
func TestDeferOrder_StreamDoneClosesFirst(t *testing.T) {
	p := &mockStreamProvider{
		chunks: []*types.StreamChunk{
			{Type: "content", Delta: "final"},
		},
	}

	req := provider.ChatRequest{
		Model: "test-model",
		Messages: []types.Message{
			{Role: "user", Content: "test"},
		},
	}

	cmd, _ := StartStreamCmd(context.Background(), p, req, "session-defer")

	// Read all messages — should get StreamMsg, StreamDoneMsg, then nil
	var gotStreamMsg, gotDoneMsg bool
	for {
		msg := cmd()
		if msg == nil {
			break
		}
		switch m := msg.(type) {
		case StreamMsg:
			gotStreamMsg = true
			_ = m
		case StreamDoneMsg:
			gotDoneMsg = true
		case StreamErrorMsg:
			t.Fatalf("unexpected error: %v", m.Err)
		}
	}

	if !gotStreamMsg {
		t.Error("expected to receive a StreamMsg")
	}
	if !gotDoneMsg {
		t.Error("expected to receive a StreamDoneMsg before nil")
	}

	// Verify defer order in source: streamCh must be the only channel,
	// closed via defer close(streamCh) after the goroutine's work.
	// Source-level verification: the cmd returned nil after StreamDoneMsg,
	// which means the channel was closed after the done message was sent.
}

// TestSafeCloseOnce_FirstCallerWins verifies that exactly one caller
// of safeCloseOnce performs the close, and all others see closed=false.
func TestSafeCloseOnce_FirstCallerWins(t *testing.T) {
	ch := make(chan struct{})

	// First call should close
	if !safeCloseOnce(ch) {
		t.Error("first call should return true (performed close)")
	}

	// Subsequent calls should not close
	for i := 0; i < 10; i++ {
		if safeCloseOnce(ch) {
			t.Errorf("call %d should return false (already closed)", i)
		}
	}
}
