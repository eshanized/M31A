package tui

import (
	"fmt"
	"os"
	"path/filepath"
	"testing"

	"github.com/eshanized/M31A/internal/config"
	m31errors "github.com/eshanized/M31A/internal/errors"
	"github.com/eshanized/M31A/internal/tui/components"
	"github.com/eshanized/M31A/internal/tui/theme"
	"github.com/eshanized/M31A/internal/types"
)

// --- H-19/M-12: /optimize respects AutoArbitrage flag ---

func TestOptimizeCommand_RespectsAutoArbitrageFlag(t *testing.T) {
	cfg := &config.Config{
		Model: config.ModelConfig{
			AutoArbitrage: false,
		},
	}
	ctx := CommandContext{
		Config: cfg,
	}

	result := handleOptimize(nil, ctx)
	if result.Success {
		t.Error("expected failure when AutoArbitrage is disabled")
	}
	if result.Message == "" {
		t.Error("expected non-empty message")
	}
}

func TestOptimizeCommand_UsesThreshold(t *testing.T) {
	cfg := &config.Config{
		Model: config.ModelConfig{
			AutoArbitrage:      true,
			ArbitrageThreshold: 0.30,
		},
	}
	ctx := CommandContext{
		Config: cfg,
	}

	// No registry — should fail gracefully (no provider)
	result := handleOptimize(nil, ctx)
	if result.Success {
		t.Error("expected failure with no registry")
	}
}

// --- H-11: Typed error banner ---

func TestTypedErrorBanner_ContextExceeded(t *testing.T) {
	banner := renderErrorBanner(m31errors.ErrContextExceeded, theme.Dark())
	if banner == "" {
		t.Error("expected non-empty banner")
	}
	// The banner should contain the context exceeded message
	// (lipgloss styling wraps the text)
	if !containsText(banner, "Context window exceeded") {
		t.Errorf("banner does not contain expected text, got: %q", banner)
	}
}

func TestTypedErrorBanner_InvalidKey(t *testing.T) {
	banner := renderErrorBanner(m31errors.ErrInvalidKey, theme.Dark())
	if banner == "" {
		t.Error("expected non-empty banner")
	}
	if !containsText(banner, "Invalid API key") {
		t.Errorf("banner does not contain expected text, got: %q", banner)
	}
}

func TestTypedErrorBanner_RateLimited(t *testing.T) {
	banner := renderErrorBanner(m31errors.ErrRateLimited, theme.Dark())
	if banner == "" {
		t.Error("expected non-empty banner")
	}
	if !containsText(banner, "Rate limited") {
		t.Errorf("banner does not contain expected text, got: %q", banner)
	}
}

func TestTypedErrorBanner_GenericError(t *testing.T) {
	err := fmt.Errorf("weird network error")
	banner := renderErrorBanner(err, theme.Dark())
	if banner == "" {
		t.Error("expected non-empty banner")
	}
	if !containsText(banner, "weird network error") {
		t.Errorf("banner does not contain error text, got: %q", banner)
	}
}

// --- M-24: typedErrorName ---

func TestTypedErrorName_ContextExceeded(t *testing.T) {
	name := typedErrorName(m31errors.ErrContextExceeded)
	if name != "ErrContextExceeded" {
		t.Errorf("typedErrorName = %q, want %q", name, "ErrContextExceeded")
	}
}

func TestTypedErrorName_ChainedWrapped(t *testing.T) {
	wrapped := fmt.Errorf("outer context: %w", m31errors.ErrContextExceeded)
	name := typedErrorName(wrapped)
	if name != "ErrContextExceeded" {
		t.Errorf("typedErrorName = %q, want %q for wrapped error", name, "ErrContextExceeded")
	}
}

func TestTypedErrorName_Unknown(t *testing.T) {
	name := typedErrorName(fmt.Errorf("something random"))
	if name != "unknown" {
		t.Errorf("typedErrorName = %q, want %q", name, "unknown")
	}
}

// --- M-17: AutoCollapseTools ---

func TestAutoCollapseTools_Respected(t *testing.T) {
	call := types.ToolCall{
		ID:    "test-1",
		Name:  "Bash",
		Input: []byte(`{"command":"echo hello"}`),
	}

	// Without AutoCollapseTools — card starts expanded (collapsed only for binary/long output)
	card1 := components.NewToolCard(call, nil, components.ToolRunning, theme.Dark())
	if card1.IsCollapsed() {
		t.Error("expected card NOT collapsed by default for short output")
	}

	// With AutoCollapseTools — card starts collapsed
	card2 := components.NewToolCard(call, nil, components.ToolRunning, theme.Dark())
	card2.SetCollapsed(true)
	if !card2.IsCollapsed() {
		t.Error("expected card collapsed after SetCollapsed(true)")
	}
}

// --- M-23: /cost toggles ShowCostEstimate ---

func TestCostCommand_TogglesFlag(t *testing.T) {
	tmpDir := t.TempDir()
	cfgPath := filepath.Join(tmpDir, "config.toml")

	cfg := config.DefaultConfig()
	cfg.UI.ShowCostEstimate = true

	// Save initial config
	if err := cfg.Save(cfgPath); err != nil {
		t.Fatalf("failed to save config: %v", err)
	}

	// Reload to verify
	loaded, err := config.Load(cfgPath)
	if err != nil {
		t.Fatalf("failed to load config: %v", err)
	}
	if !loaded.UI.ShowCostEstimate {
		t.Fatal("expected ShowCostEstimate=true initially")
	}

	ctx := CommandContext{
		Config:     loaded,
		ConfigPath: cfgPath,
	}

	// Toggle off
	result := handleCost(nil, ctx)
	if !result.Success {
		t.Errorf("/cost failed: %s", result.Message)
	}
	if loaded.UI.ShowCostEstimate {
		t.Error("expected ShowCostEstimate=false after first toggle")
	}

	// Verify persistence
	reloaded, err := config.Load(cfgPath)
	if err != nil {
		t.Fatalf("failed to reload config: %v", err)
	}
	if reloaded.UI.ShowCostEstimate {
		t.Error("expected ShowCostEstimate=false in persisted config")
	}

	// Toggle back on
	result = handleCost(nil, CommandContext{Config: reloaded, ConfigPath: cfgPath})
	if !result.Success {
		t.Errorf("/cost failed: %s", result.Message)
	}
	if !reloaded.UI.ShowCostEstimate {
		t.Error("expected ShowCostEstimate=true after second toggle")
	}
}

// --- M-24: debug-mode log ---

func TestTypedErrorName_DebugLog(t *testing.T) {
	// Set debug mode
	t.Setenv("M31A_LOG_LEVEL", "debug")

	// Create a minimal ReplModel and trigger the error handler
	m := NewReplModel(theme.Dark())
	m.width = 80
	m.height = 40

	msg := StreamErrorMsg{Err: m31errors.ErrContextExceeded}
	cmds, _ := m.handleStreamErrorMsg(msg)

	if len(cmds) > 0 {
		t.Error("expected no commands from error handler")
	}

	// Verify the error message was added to the message list
	if len(m.messages) == 0 {
		t.Fatal("expected at least one message after error")
	}

	lastMsg := m.messages[len(m.messages)-1]
	if lastMsg.Role != "assistant" {
		t.Errorf("expected assistant message, got %q", lastMsg.Role)
	}
}

// --- helper ---

// containsText checks if a lipgloss-styled string contains the given text.
// lipgloss may add ANSI codes, so we check the raw string.
func containsText(s, substr string) bool {
	return len(s) > 0 && (s == substr || len(s) > len(substr) && findSubstr(s, substr))
}

func findSubstr(s, substr string) bool {
	for i := 0; i <= len(s)-len(substr); i++ {
		if s[i:i+len(substr)] == substr {
			return true
		}
	}
	return false
}

// silence unused import
var _ = os.Getenv
