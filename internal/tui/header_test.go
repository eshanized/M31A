package tui

import (
	"strings"
	"testing"

	"github.com/eshanized/M31A/internal/tui/theme"
	"github.com/eshanized/M31A/internal/types"
)

func TestRenderHeader_ContainsBrand(t *testing.T) {
	result := RenderHeader(theme.Dark(), "openrouter", nil, types.HealthStatus{}, 0, 0, 120)
	if !strings.Contains(result, "M31A") {
		t.Errorf("Header should contain 'M31A', got %q", result)
	}
}

func TestRenderHeader_BlockAnchor(t *testing.T) {
	result := RenderHeader(theme.Dark(), "openrouter", nil, types.HealthStatus{}, 0, 0, 120)
	if !strings.Contains(result, "▓") {
		t.Errorf("Header should contain block anchor '▓', got %q", result)
	}
}

func TestRenderHeader_ProviderBadge(t *testing.T) {
	result := RenderHeader(theme.Dark(), "openrouter", nil, types.HealthStatus{}, 0, 0, 120)
	if !strings.Contains(result, "[OR]") {
		t.Errorf("Header should contain '[OR]', got %q", result)
	}
}

func TestRenderHeader_ZenBadge(t *testing.T) {
	result := RenderHeader(theme.Dark(), "zen", nil, types.HealthStatus{}, 0, 0, 120)
	if !strings.Contains(result, "[ZEN]") {
		t.Errorf("Header should contain '[ZEN]', got %q", result)
	}
}

func TestRenderHeader_ContextBar(t *testing.T) {
	model := &types.ModelInfo{Name: "test-model"}
	result := RenderHeader(theme.Dark(), "openrouter", model, types.HealthStatus{}, 1000, 100000, 120)
	if !strings.Contains(result, "1.0K") {
		t.Errorf("Header should contain '1.0K', got %q", result)
	}
}

func TestRenderHeader_HealthLive(t *testing.T) {
	result := RenderHeader(theme.Dark(), "openrouter", nil, types.HealthStatus{Status: "live"}, 0, 0, 120)
	if !strings.Contains(result, "[LIVE]") {
		t.Errorf("Header should contain '[LIVE]', got %q", result)
	}
}

func TestRenderHeader_HealthOffline(t *testing.T) {
	result := RenderHeader(theme.Dark(), "openrouter", nil, types.HealthStatus{Status: "offline"}, 0, 0, 120)
	if !strings.Contains(result, "[OFF]") {
		t.Errorf("Header should contain '[OFF]', got %q", result)
	}
}

func TestRenderHeader_Truncation(t *testing.T) {
	model := &types.ModelInfo{Name: "very-long-model-name-that-should-be-truncated"}
	result := RenderHeader(theme.Dark(), "openrouter", model, types.HealthStatus{Status: "live"}, 0, 0, 50)
	if result == "" {
		t.Error("Truncated header should not be empty")
	}
}

func TestRenderHeader_EmptyProvider(t *testing.T) {
	result := RenderHeader(theme.Dark(), "", nil, types.HealthStatus{}, 0, 0, 120)
	if !strings.Contains(result, "M31A") {
		t.Errorf("Empty provider should still render brand, got %q", result)
	}
}

func TestRenderHeader_NilModel(t *testing.T) {
	result := RenderHeader(theme.Dark(), "openrouter", nil, types.HealthStatus{}, 0, 0, 120)
	if !strings.Contains(result, "[OR]") {
		t.Errorf("Nil model should still show badge, got %q", result)
	}
}

func TestRenderHeader_HealthSlow(t *testing.T) {
	result := RenderHeader(theme.Dark(), "openrouter", nil, types.HealthStatus{Status: "slow"}, 0, 0, 120)
	if !strings.Contains(result, "[SLOW]") {
		t.Errorf("Header should contain '[SLOW]', got %q", result)
	}
}

func TestRenderHeader_UnknownHealth(t *testing.T) {
	result := RenderHeader(theme.Dark(), "openrouter", nil, types.HealthStatus{Status: ""}, 0, 0, 120)
	if !strings.Contains(result, "[??]") {
		t.Errorf("Empty health should show '[??]', got %q", result)
	}
}

func TestRenderHeader_ZeroContext(t *testing.T) {
	result := RenderHeader(theme.Dark(), "openrouter", nil, types.HealthStatus{}, 0, 0, 120)
	if !strings.Contains(result, "--/--") {
		t.Errorf("Zero context should show '--/--', got %q", result)
	}
}

func TestRenderHeader_ModelNameTruncated(t *testing.T) {
	model := &types.ModelInfo{Name: "aaaaaaaaaaaaaaaaaaaaabbbbb"}
	result := RenderHeader(theme.Dark(), "openrouter", model, types.HealthStatus{}, 0, 0, 120)
	if strings.Contains(result, "bbbbb") {
		t.Errorf("Model name should be truncated to 20 chars, got %q", result)
	}
}

func TestRenderHeader_WidthVeryNarrow(t *testing.T) {
	result := RenderHeader(theme.Dark(), "openrouter", nil, types.HealthStatus{}, 0, 0, 10)
	if result != "..." {
		t.Errorf("Width < 20 should return '...', got %q", result)
	}
}

func TestRenderHeader_ContextColorWarning(t *testing.T) {
	result := RenderHeader(theme.Dark(), "openrouter", nil, types.HealthStatus{}, 80000, 100000, 120)
	if !strings.Contains(result, "ctx") {
		t.Errorf("80%% context should show context bar, got %q", result)
	}
}

func TestRenderHeader_ContextColorError(t *testing.T) {
	result := RenderHeader(theme.Dark(), "openrouter", nil, types.HealthStatus{}, 96000, 100000, 120)
	if !strings.Contains(result, "ctx") {
		t.Errorf("96%% context should show context bar, got %q", result)
	}
}

func TestRenderHeader_Width80(t *testing.T) {
	result := RenderHeader(theme.Dark(), "openrouter", nil, types.HealthStatus{Status: "live"}, 0, 0, 80)
	if result == "" {
		t.Error("Header at 80 cols should not be empty")
	}
}

func TestRenderHeader_Width160(t *testing.T) {
	result := RenderHeader(theme.Dark(), "openrouter", nil, types.HealthStatus{Status: "live"}, 0, 0, 160)
	if result == "" {
		t.Error("Header at 160 cols should not be empty")
	}
}

func TestRenderHeader_Width200(t *testing.T) {
	result := RenderHeader(theme.Dark(), "openrouter", nil, types.HealthStatus{Status: "live"}, 0, 0, 200)
	if result == "" {
		t.Error("Header at 200 cols should not be empty")
	}
}

func TestRenderPhaseBreadcrumb_Current(t *testing.T) {
	result := RenderPhaseBreadcrumb(theme.Dark(), types.PhaseExecute, 120)
	if !strings.Contains(result, "EXECUTE") {
		t.Errorf("Phase breadcrumb should contain 'EXECUTE', got %q", result)
	}
	if !strings.Contains(result, "↑") {
		t.Errorf("Current phase should have ↑ pointer, got %q", result)
	}
}

func TestRenderPhaseBreadcrumb_Past(t *testing.T) {
	result := RenderPhaseBreadcrumb(theme.Dark(), types.PhaseExecute, 120)
	if !strings.Contains(result, "DISCUSS") {
		t.Errorf("Past phases should appear, got %q", result)
	}
	if !strings.Contains(result, "PLAN") {
		t.Errorf("Past phases should appear, got %q", result)
	}
}

func TestRenderPhaseBreadcrumb_Future(t *testing.T) {
	result := RenderPhaseBreadcrumb(theme.Dark(), types.PhaseExecute, 120)
	if !strings.Contains(result, "VERIFY") {
		t.Errorf("Future phases should appear, got %q", result)
	}
	if !strings.Contains(result, "SHIP") {
		t.Errorf("Future phases should appear, got %q", result)
	}
}

func TestRenderPhaseBreadcrumb_Separators(t *testing.T) {
	result := RenderPhaseBreadcrumb(theme.Dark(), types.PhasePlan, 120)
	if !strings.Contains(result, "───") {
		t.Errorf("Phase breadcrumb should have separators, got %q", result)
	}
}
