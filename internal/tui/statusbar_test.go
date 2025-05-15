package tui

import (
	"strings"
	"testing"
	"time"

	"github.com/eshanized/M31A/internal/tui/theme"
)

func TestRenderStatusBar_Ready(t *testing.T) {
	result := RenderStatusBar(theme.Dark(), "", time.Time{}, 80, nil)
	if !strings.Contains(result, "Ready") {
		t.Errorf("Empty operation should show 'Ready', got %q", result)
	}
}

func TestRenderStatusBar_Operation(t *testing.T) {
	result := RenderStatusBar(theme.Dark(), "Processing...", time.Time{}, 80, nil)
	if !strings.Contains(result, "Processing...") {
		t.Errorf("Should contain operation text, got %q", result)
	}
}

func TestRenderStatusBar_Streaming(t *testing.T) {
	info := &StatusBarInfo{IsStreaming: true}
	result := RenderStatusBar(theme.Dark(), "", time.Time{}, 80, info)
	if !strings.Contains(result, "building") {
		t.Errorf("Should show streaming indicator, got %q", result)
	}
}

func TestRenderStatusBar_Thinking(t *testing.T) {
	info := &StatusBarInfo{IsStreaming: true, IsThinking: true}
	result := RenderStatusBar(theme.Dark(), "", time.Time{}, 80, info)
	if !strings.Contains(result, "thinking") {
		t.Errorf("Should show thinking indicator, got %q", result)
	}
}

func TestRenderStatusBar_LeaderActive(t *testing.T) {
	info := &StatusBarInfo{LeaderActive: true}
	result := RenderStatusBar(theme.Dark(), "", time.Time{}, 80, info)
	if !strings.Contains(result, "ctrl+x") {
		t.Errorf("Should show leader prompt, got %q", result)
	}
}

func TestRenderStatusBar_KeyboardHints(t *testing.T) {
	info := &StatusBarInfo{KeyboardHints: []string{"ctrl+p commands"}}
	result := RenderStatusBar(theme.Dark(), "", time.Time{}, 80, info)
	if !strings.Contains(result, "ctrl+p commands") {
		t.Errorf("Should show keyboard hints, got %q", result)
	}
}

func TestRenderStatusBar_Truncation(t *testing.T) {
	result := RenderStatusBar(theme.Dark(), "test", time.Time{}, 5, nil)
	if result != "" {
		t.Errorf("Width < 10 should return empty, got %q", result)
	}
}

func TestRenderStatusBar_LongOperation(t *testing.T) {
	longOp := "this is a very long operation description that should be truncated to fit"
	result := RenderStatusBar(theme.Dark(), longOp, time.Time{}, 60, nil)
	if strings.Contains(result, longOp) {
		t.Errorf("Long operation should be truncated, got %q", result)
	}
}

func TestRenderStatusBar_Format(t *testing.T) {
	result := RenderStatusBar(theme.Dark(), "test", time.Time{}, 80, nil)
	if result == "" {
		t.Error("Status bar should not be empty")
	}
}

func TestRenderStatusBar_WithUsage(t *testing.T) {
	info := &StatusBarInfo{
		PromptTokens: 1000,
		TotalTokens:  1500,
		Cost:         0.0123,
		ShowCost:     true,
	}
	result := RenderStatusBar(theme.Dark(), "Ready", time.Time{}, 80, info)
	if !strings.Contains(result, "1.5K ctx") {
		t.Errorf("Should contain token count, got %q", result)
	}
	if !strings.Contains(result, "$0.0123") {
		t.Errorf("Should contain cost, got %q", result)
	}
}

func TestRenderPromptMetadata(t *testing.T) {
	result := RenderPromptMetadata("Build", "claude-sonnet-4", "openrouter", theme.Dark(), 80)
	if !strings.Contains(result, "Build") {
		t.Errorf("Should contain agent name, got %q", result)
	}
	if !strings.Contains(result, "OPE") {
		t.Errorf("Should contain provider short name, got %q", result)
	}
}

func TestRenderPromptBottomBorder(t *testing.T) {
	result := RenderPromptBottomBorder(theme.Dark().Border, 80)
	if len(result) == 0 {
		t.Error("Bottom border should not be empty")
	}
	if !strings.Contains(result, "\u2579") {
		t.Errorf("Should contain corner character, got %q", result)
	}
	if !strings.Contains(result, "\u2580") {
		t.Errorf("Should contain half-block character, got %q", result)
	}
}
