package components

import (
	"strings"
	"testing"
	"time"

	"github.com/eshanized/M31A/internal/ui/tui/theme"
	"github.com/eshanized/M31A/internal/core/types"
)

func TestNewMessageRenderer_DarkTheme(t *testing.T) {
	r, err := NewMessageRenderer(theme.Dark(), 80)
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
	r, err := NewMessageRenderer(theme.Light(), 80)
	if err != nil {
		t.Fatalf("unexpected error creating light renderer: %v", err)
	}
	if r == nil {
		t.Fatal("expected non-nil renderer")
	}
}

func TestRenderMessage_UserMessage(t *testing.T) {
	r, err := NewMessageRenderer(theme.Dark(), 80)
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
	r, err := NewMessageRenderer(theme.Dark(), 80)
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
	r, err := NewMessageRenderer(theme.Dark(), 80)
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
	r, err := NewMessageRenderer(theme.Dark(), 80)
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
	r, err := NewMessageRenderer(theme.Dark(), 80)
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
	r, err := NewMessageRenderer(theme.Dark(), 80)
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
	r, err := NewMessageRenderer(theme.Dark(), 80)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}

	result := r.renderContentSegment("**bold text**", 80)
	if result == "" {
		t.Error("expected non-empty rendered markdown")
	}
}

// TestRenderAssistantMessage_ErrorSegmentNoRawANSI guards against the regression
// where an error banner was pre-styled with lipgloss, stored as a "content"
// segment, and re-rendered through glamour — which mangled the ANSI escape
// sequence so the viewport displayed literal `[1;38;2;242;139;130m× Bad
// request…[0m` instead of styled red text.
func TestRenderAssistantMessage_ErrorSegmentNoRawANSI(t *testing.T) {
	r, err := NewMessageRenderer(theme.Dark(), 100)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}

	msg := types.Message{
		Role:    "assistant",
		Content: "✗ Bad request — check your input parameters",
		Segments: []types.MessageSegment{{
			Type:    "error",
			Content: "✗ Bad request — check your input parameters",
			Visible: true,
		}},
	}
	out := r.RenderMessage(msg, 100)

	// Error text must be present
	if !strings.Contains(out, "Bad request") {
		t.Errorf("rendered output missing error text: %q", out)
	}
	if !strings.Contains(out, "✗") {
		t.Errorf("rendered output missing ✗ glyph: %q", out)
	}

	// Raw ANSI fragments (missing leading ESC \x1b) must NOT appear as visible text.
	// Well-formed ANSI sequences always start with ESC (\x1b) + '['; if the
	// viewport ever shows `[1;38;…m` without the ESC, glamour has mangled it.
	for _, frag := range []string{"[1;38;", "[0m", "[1;31m"} {
		idx := strings.Index(out, frag)
		for idx >= 0 {
			if idx == 0 || out[idx-1] != '\x1b' {
				t.Errorf("raw ANSI fragment %q found without leading ESC at offset %d in %q", frag, idx, out)
				break
			}
			idx = strings.Index(out[idx+1:], frag)
			if idx >= 0 {
				// adjust to absolute index for next iteration
				idx = strings.Index(out, frag)
			}
		}
	}
}
