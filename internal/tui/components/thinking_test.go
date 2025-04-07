package components

import (
	"strings"
	"testing"
	"time"

	"github.com/eshanized/M31A/internal/tui/theme"
	"github.com/eshanized/M31A/internal/types"
)

func TestNewThinkingBlock_DefaultState(t *testing.T) {
	seg := types.MessageSegment{
		Type:    "thinking",
		Content: "thinking content",
	}
	tb := NewThinkingBlock(seg, theme.Dark(), false)
	if tb == nil {
		t.Fatal("expected non-nil ThinkingBlock")
	}
	if tb.IsExpanded() {
		t.Error("expected collapsed by default")
	}
}

func TestThinkingBlock_RenderCollapsed(t *testing.T) {
	seg := types.MessageSegment{
		Type:    "thinking",
		Content: "deep reasoning content",
	}
	tb := NewThinkingBlock(seg, theme.Dark(), false)
	result := tb.Render(80)
	if result == "" {
		t.Error("expected non-empty collapsed render")
	}
}

func TestThinkingBlock_RenderExpanded(t *testing.T) {
	seg := types.MessageSegment{
		Type:    "thinking",
		Content: "expanded thinking content here",
	}
	tb := NewThinkingBlock(seg, theme.Dark(), true)
	result := tb.Render(80)
	if result == "" {
		t.Error("expected non-empty expanded render")
	}
}

func TestThinkingBlock_Toggle(t *testing.T) {
	seg := types.MessageSegment{
		Type:    "thinking",
		Content: "content",
	}
	tb := NewThinkingBlock(seg, theme.Dark(), false)

	tb.Toggle()
	if !tb.IsExpanded() {
		t.Error("expected expanded after toggle")
	}

	tb.Toggle()
	if tb.IsExpanded() {
		t.Error("expected collapsed after second toggle")
	}
}

func TestThinkingBlock_Duration_Under10s(t *testing.T) {
	seg := types.MessageSegment{
		Type:       "thinking",
		Content:    "test",
		DurationMs: 1200,
	}
	tb := NewThinkingBlock(seg, theme.Dark(), false)
	dur := tb.Duration()
	if dur != "1.2s" {
		t.Errorf("expected 1.2s, got %q", dur)
	}
}

func TestThinkingBlock_Duration_Over10s(t *testing.T) {
	seg := types.MessageSegment{
		Type:       "thinking",
		Content:    "test",
		DurationMs: 12300,
	}
	tb := NewThinkingBlock(seg, theme.Dark(), false)
	dur := tb.Duration()
	if dur != "12.3s" {
		t.Errorf("expected 12.3s, got %q", dur)
	}
}

func TestThinkingBlock_Duration_Over60s(t *testing.T) {
	seg := types.MessageSegment{
		Type:       "thinking",
		Content:    "test",
		DurationMs: 83000,
	}
	tb := NewThinkingBlock(seg, theme.Dark(), false)
	dur := tb.Duration()
	if dur != "1m 23s" {
		t.Errorf("expected 1m 23s, got %q", dur)
	}
}

func TestThinkingBlock_Header_TruncatesWide(t *testing.T) {
	seg := types.MessageSegment{
		Type:    "thinking",
		Content: "very long content that should be truncated in header",
	}
	tb := NewThinkingBlock(seg, theme.Dark(), true)
	header := tb.Header(20)
	if header == "" {
		t.Error("expected non-empty header")
	}
}

func TestThinkingBlock_FinalizedDuration(t *testing.T) {
	seg := types.MessageSegment{
		Type:       "thinking",
		Content:    "done thinking",
		DurationMs: 5500,
	}
	tb := NewThinkingBlock(seg, theme.Dark(), false)
	dur := tb.Duration()
	if dur != "5.5s" {
		t.Errorf("expected 5.5s, got %q", dur)
	}
}

func TestThinkingBlock_LiveDuration(t *testing.T) {
	seg := types.MessageSegment{
		Type:    "thinking",
		Content: "live thinking",
	}
	tb := NewThinkingBlock(seg, theme.Dark(), false)
	time.Sleep(60 * time.Millisecond)
	dur := tb.Duration()
	if dur == "0.0s" {
		t.Errorf("expected non-zero live duration, got %q", dur)
	}
}

func TestThinkingBlock_Header_Collapsed(t *testing.T) {
	seg := types.MessageSegment{
		Type:    "thinking",
		Content: "content",
	}
	tb := NewThinkingBlock(seg, theme.Dark(), false)
	header := tb.Header(80)
	if !strings.Contains(header, "▼") {
		t.Error("expected '▼' in collapsed header")
	}
}

func TestThinkingBlock_Header_Expanded(t *testing.T) {
	seg := types.MessageSegment{
		Type:    "thinking",
		Content: "content",
	}
	tb := NewThinkingBlock(seg, theme.Dark(), true)
	header := tb.Header(80)
	if !strings.Contains(header, "▲") {
		t.Error("expected '▲' in expanded header")
	}
}

func TestThinkingBlock_AfterToggle(t *testing.T) {
	seg := types.MessageSegment{
		Type:    "thinking",
		Content: "content",
	}
	tb := NewThinkingBlock(seg, theme.Dark(), false)

	// Initially collapsed
	if tb.IsExpanded() {
		t.Error("expected initially collapsed")
	}
	headerBefore := tb.Header(80)
	if !strings.Contains(headerBefore, "▼") {
		t.Error("expected '▼' before toggle")
	}

	// Toggle to expanded
	tb.Toggle()
	if !tb.IsExpanded() {
		t.Error("expected expanded after toggle")
	}
	headerAfter := tb.Header(80)
	if !strings.Contains(headerAfter, "▲") {
		t.Error("expected '▲' after toggle")
	}
}
