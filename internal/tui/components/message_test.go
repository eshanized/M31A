package components

import (
	"testing"
	"time"

	"github.com/eshanized/M31A/internal/tui/theme"
	"github.com/eshanized/M31A/internal/types"
)

func TestNewMessageRenderer_DarkTheme(t *testing.T) {
	r, err := NewMessageRenderer(theme.Dark())
	if err != nil {
		t.Fatalf("unexpected error creating dark renderer: %v", err)
	}
	if r == nil {
		t.Fatal("expected non-nil renderer")
	}
	if r.renderer == nil {
		t.Error("expected non-nil glamour renderer")
	}
}

func TestNewMessageRenderer_LightTheme(t *testing.T) {
	r, err := NewMessageRenderer(theme.Light())
	if err != nil {
		t.Fatalf("unexpected error creating light renderer: %v", err)
	}
	if r == nil {
		t.Fatal("expected non-nil renderer")
	}
}

func TestRenderMessage_UserMessage(t *testing.T) {
	r, err := NewMessageRenderer(theme.Dark())
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}

	msg := types.Message{
		Role:      "user",
		Content:   "hello world",
		CreatedAt: time.Time{},
	}
	result := r.RenderMessage(msg, 80)
	if result == "" {
		t.Error("expected non-empty render")
	}
}

func TestRenderMessage_UserMessageRightAligned(t *testing.T) {
	r, err := NewMessageRenderer(theme.Dark())
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}

	msg := types.Message{
		Role:      "user",
		Content:   "short",
		CreatedAt: time.Time{},
	}
	result := r.RenderMessage(msg, 100)
	if result == "" {
		t.Error("expected non-empty render")
	}
}

func TestRenderMessage_AssistantMessage(t *testing.T) {
	r, err := NewMessageRenderer(theme.Dark())
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}

	msg := types.Message{
		Role:    "assistant",
		Content: "Hello, I can help you with that.",
		Segments: []types.MessageSegment{
			{Type: "content", Content: "Hello, I can help you with that.", Visible: true},
		},
		CreatedAt: time.Time{},
	}
	result := r.RenderMessage(msg, 80)
	if result == "" {
		t.Error("expected non-empty render")
	}
}

func TestRenderMessage_AssistantWithThinking(t *testing.T) {
	r, err := NewMessageRenderer(theme.Dark())
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}

	msg := types.Message{
		Role:    "assistant",
		Content: "Here is the solution.",
		Segments: []types.MessageSegment{
			{Type: "thinking", Content: "Let me think about this...", Visible: true},
			{Type: "content", Content: "Here is the solution.", Visible: true},
		},
		CreatedAt: time.Time{},
	}
	result := r.RenderMessage(msg, 80)
	if result == "" {
		t.Error("expected non-empty render")
	}
}

func TestRenderMessage_MultipleSegments(t *testing.T) {
	r, err := NewMessageRenderer(theme.Dark())
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}

	msg := types.Message{
		Role:    "assistant",
		Content: "Final answer",
		Segments: []types.MessageSegment{
			{Type: "thinking", Content: "First thought", Visible: true},
			{Type: "content", Content: "Intermediate", Visible: true},
			{Type: "thinking", Content: "Second thought", Visible: true},
			{Type: "content", Content: "Final answer", Visible: true},
		},
		CreatedAt: time.Time{},
	}
	result := r.RenderMessage(msg, 80)
	if result == "" {
		t.Error("expected non-empty render for multiple segments")
	}
}

func TestRenderMessage_TruncatesWidth(t *testing.T) {
	r, err := NewMessageRenderer(theme.Dark())
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}

	msg := types.Message{
		Role:    "user",
		Content: "hi",
	}
	result := r.RenderMessage(msg, 40)
	if result == "" {
		t.Error("expected non-empty render at reduced width")
	}
}

func TestRenderContentSegment_GlamourMarkdown(t *testing.T) {
	r, err := NewMessageRenderer(theme.Dark())
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}

	result := r.renderContentSegment("**bold text**", 80)
	if result == "" {
		t.Error("expected non-empty rendered markdown")
	}
}
