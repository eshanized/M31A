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

func TestRenderStatusBar_Timestamp(t *testing.T) {
	now := time.Now()
	result := RenderStatusBar(theme.Dark(), "Working", now, 80, nil)
	if !strings.Contains(result, "Last:") {
		t.Errorf("Should contain timestamp, got %q", result)
	}
}

func TestRenderStatusBar_EmptyTime(t *testing.T) {
	result := RenderStatusBar(theme.Dark(), "Working", time.Time{}, 80, nil)
	if strings.Contains(result, "Last:") {
		t.Errorf("Zero time should not show timestamp, got %q", result)
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
	result := RenderStatusBar(theme.Dark(), longOp, time.Now(), 60, nil)
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

func TestRenderStatusBar_OperationAndTime(t *testing.T) {
	now := time.Now()
	result := RenderStatusBar(theme.Dark(), "Working", now, 120, nil)
	if !strings.Contains(result, "Working") || !strings.Contains(result, "Last:") {
		t.Errorf("Should contain both operation and timestamp, got %q", result)
	}
}

func TestRenderStatusBar_ReadyNoTime(t *testing.T) {
	result := RenderStatusBar(theme.Dark(), "", time.Time{}, 80, nil)
	if !strings.Contains(result, "Ready") {
		t.Errorf("Should show 'Ready' with no timestamp, got %q", result)
	}
}

func TestRenderStatusBar_TrimmedWidth(t *testing.T) {
	now := time.Now()
	result := RenderStatusBar(theme.Dark(), "Hello World", now, 20, nil)
	if result == "" {
		t.Error("Should return non-empty string for width=20")
	}
}

func TestRenderStatusBar_WithUsage(t *testing.T) {
	info := &StatusBarInfo{
		PromptTokens:  1000,
		TotalTokens:   1500,
		Cost:          0.0123,
		ShowCost:      true,
	}
	result := RenderStatusBar(theme.Dark(), "Ready", time.Time{}, 80, info)
	if !strings.Contains(result, "1.5K ctx") {
		t.Errorf("Should contain token count, got %q", result)
	}
	if !strings.Contains(result, "$0.0123") {
		t.Errorf("Should contain cost, got %q", result)
	}
}
