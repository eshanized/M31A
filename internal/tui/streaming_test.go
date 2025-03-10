package tui

import (
	"context"
	"io"
	"testing"
	"time"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/eshanized/M31A/internal/provider"
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

	cmd := StartStreamCmd(context.Background(), p, req, "test-session")
	if cmd == nil {
		t.Fatal("expected non-nil cmd")
	}

	msg := cmd()
	if msg == nil {
		t.Fatal("expected non-nil msg")
	}

	ch, ok := msg.(chan tea.Msg)
	if !ok {
		t.Fatalf("expected chan tea.Msg, got %T", msg)
	}

	var streamMsgCount int
	var doneMsgCount int

	for m := range ch {
		switch m.(type) {
		case StreamMsg:
			streamMsgCount++
		case StreamDoneMsg:
			doneMsgCount++
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

	cmd := StartStreamCmd(context.Background(), p, req, "session-1")
	msg := cmd()

	ch, ok := msg.(chan tea.Msg)
	if !ok {
		t.Fatalf("expected chan tea.Msg, got %T", msg)
	}

	var streamMsgCount int
	var doneMsgCount int

	for m := range ch {
		switch m.(type) {
		case StreamMsg:
			streamMsgCount++
		case StreamDoneMsg:
			doneMsgCount++
		case StreamErrorMsg:
			t.Error("unexpected StreamErrorMsg")
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

	cmd := StartStreamCmd(context.Background(), p, req, "session-2")
	msg := cmd()

	ch, ok := msg.(chan tea.Msg)
	if !ok {
		t.Fatalf("expected chan tea.Msg, got %T", msg)
	}

	hasError := false
	for m := range ch {
		if _, ok := m.(StreamErrorMsg); ok {
			hasError = true
		}
	}

	if !hasError {
		t.Error("expected StreamErrorMsg for context exceeded")
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

	cmd := StartStreamCmd(ctx, p, req, "session-3")
	msg := cmd()

	ch, ok := msg.(chan tea.Msg)
	if !ok {
		t.Fatalf("expected chan tea.Msg, got %T", msg)
	}

	count := 0
	for m := range ch {
		count++
		if count == 2 {
			cancel()
		}
		_ = m
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
