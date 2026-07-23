package components_test

import (
	"github.com/eshanized/M31A/internal/ui/tui/components"
	"testing"

	"github.com/eshanized/M31A/internal/ui/tui/theme"
)

// --- Context ---

func TestContext_Fields(t *testing.T) {
	t.Parallel()
	th := theme.Dark()
	ctx := components.Context{
		Width:   80,
		Height:  24,
		Theme:   th,
		Cache:   theme.NewStyleCache(th),
		Focused: true,
		Hovered: false,
	}
	if ctx.Width != 80 {
		t.Errorf("Width = %d, want 80", ctx.Width)
	}
	if ctx.Height != 24 {
		t.Errorf("Height = %d, want 24", ctx.Height)
	}
	if !ctx.Focused {
		t.Error("Focused should be true")
	}
	if ctx.Hovered {
		t.Error("Hovered should be false")
	}
}

func TestContext_S_WithCache(t *testing.T) {
	t.Parallel()
	th := theme.Dark()
	ctx := components.Context{
		Width:  80,
		Height: 24,
		Theme:  th,
		Cache:  theme.NewStyleCache(th),
	}
	s := ctx.S()
	if s.PageTitle.Render("test") == "" {
		t.Error("S().PageTitle should render non-empty")
	}
}

func TestContext_S_WithoutCache(t *testing.T) {
	t.Parallel()
	th := theme.Dark()
	ctx := components.Context{
		Width:  80,
		Height: 24,
		Theme:  th,
		Cache:  nil,
	}
	s := ctx.S()
	if s.PageTitle.Render("test") == "" {
		t.Error("S().PageTitle should render non-empty without cache")
	}
}

func TestNewContext(t *testing.T) {
	t.Parallel()
	th := theme.Dark()
	ctx := components.NewContext(80, 24, th)
	if ctx.Width != 80 {
		t.Errorf("Width = %d, want 80", ctx.Width)
	}
	if ctx.Height != 24 {
		t.Errorf("Height = %d, want 24", ctx.Height)
	}
	if ctx.Cache == nil {
		t.Error("Cache should not be nil")
	}
}

// --- SimpleBadge ---

func TestSimpleBadge_Render_AllTypes(t *testing.T) {
	t.Parallel()
	th := theme.Dark()
	types := []components.BadgeType{components.BadgeBrand, components.BadgeSuccess, components.BadgeError, components.BadgeWarning, components.BadgeInfo, components.BadgeNeutral}
	for _, bt := range types {
		badge := components.SimpleBadge{Text: "test", Type: bt, Theme: th}
		got := badge.Render()
		if got == "" {
			t.Errorf("components.SimpleBadge type %d should render non-empty", bt)
		}
	}
}

func TestSimpleBadge_Render_Compact(t *testing.T) {
	t.Parallel()
	th := theme.Dark()
	badge := components.SimpleBadge{Text: "test", Type: components.BadgeBrand, Compact: true, Theme: th}
	got := badge.Render()
	if got == "" {
		t.Error("Compact badge should render non-empty")
	}
}

func TestSimpleBadge_Render_NonCompact(t *testing.T) {
	t.Parallel()
	th := theme.Dark()
	badge := components.SimpleBadge{Text: "test", Type: components.BadgeBrand, Compact: false, Theme: th}
	got := badge.Render()
	if got == "" {
		t.Error("Non-compact badge should render non-empty")
	}
}

// --- components.Badge ---

func TestBadge_Render_Empty(t *testing.T) {
	t.Parallel()
	badge := components.Badge{}
	got := badge.Render()
	if got != "" {
		t.Error("Empty badge should render empty string")
	}
}

func TestBadge_Render_NonEmpty(t *testing.T) {
	t.Parallel()
	th := theme.Dark()
	badge := components.NewBadge("test", components.BadgeSuccessPreset, th)
	got := badge.Render()
	if got == "" {
		t.Error("Non-empty badge should render non-empty string")
	}
}

// --- NewBadge ---

func TestNewBadge_AllPresets(t *testing.T) {
	t.Parallel()
	th := theme.Dark()
	presets := []components.BadgePreset{
		components.BadgeSuccessPreset, components.BadgeWarningPreset, components.BadgeErrorPreset,
		components.BadgeInfoPreset, components.BadgeBrandPreset, components.BadgeMutedPreset,
	}
	for _, p := range presets {
		badge := components.NewBadge("test", p, th)
		if badge.Label != "test" {
			t.Errorf("preset %d: label = %q, want test", p, badge.Label)
		}
		got := badge.Render()
		if got == "" {
			t.Errorf("preset %d should render non-empty", p)
		}
	}
}

// --- RenderBadges ---

func TestRenderBadges_Multiple(t *testing.T) {
	t.Parallel()
	th := theme.Dark()
	badges := []components.Badge{
		components.NewBadge("fast", components.BadgeSuccessPreset, th),
		components.NewBadge("cheap", components.BadgeInfoPreset, th),
		components.NewBadge("safe", components.BadgeWarningPreset, th),
	}
	got := components.RenderBadges(badges)
	if got == "" {
		t.Error("RenderBadges should return non-empty")
	}
}

// --- components.CapabilityBadge ---

func TestCapabilityBadge_Various(t *testing.T) {
	t.Parallel()
	th := theme.Dark()
	caps := []string{"reasoning", "vision", "code", "fast"}
	for _, cap := range caps {
		badge := components.CapabilityBadge(cap, th)
		if badge.Label != cap {
			t.Errorf("components.CapabilityBadge label = %q, want %q", badge.Label, cap)
		}
	}
}

// --- StatusBadge ---

func TestStatusBadge_AllStatuses(t *testing.T) {
	t.Parallel()
	th := theme.Dark()
	statuses := []string{"done", "pass", "complete", "running", "thinking", "pending", "failed", "error", "warning", "skipped", "unknown"}
	for _, status := range statuses {
		badge := components.StatusBadge(status, th)
		if badge.Label != status {
			t.Errorf("components.StatusBadge(%q) label = %q", status, badge.Label)
		}
	}
}

// --- components.BadgeType constants ---

func TestBadgeType_Constants(t *testing.T) {
	t.Parallel()
	if components.BadgeBrand != 0 {
		t.Errorf("components.BadgeBrand = %d, want 0", components.BadgeBrand)
	}
	if components.BadgeSuccess != 1 {
		t.Errorf("components.BadgeSuccess = %d, want 1", components.BadgeSuccess)
	}
	if components.BadgeError != 2 {
		t.Errorf("components.BadgeError = %d, want 2", components.BadgeError)
	}
	if components.BadgeWarning != 3 {
		t.Errorf("components.BadgeWarning = %d, want 3", components.BadgeWarning)
	}
	if components.BadgeInfo != 4 {
		t.Errorf("components.BadgeInfo = %d, want 4", components.BadgeInfo)
	}
	if components.BadgeNeutral != 5 {
		t.Errorf("components.BadgeNeutral = %d, want 5", components.BadgeNeutral)
	}
}

// --- BadgePreset constants ---

func TestBadgePreset_Constants(t *testing.T) {
	t.Parallel()
	if components.BadgeSuccessPreset != 0 {
		t.Errorf("components.BadgeSuccessPreset = %d, want 0", components.BadgeSuccessPreset)
	}
	if components.BadgeWarningPreset != 1 {
		t.Errorf("components.BadgeWarningPreset = %d, want 1", components.BadgeWarningPreset)
	}
	if components.BadgeErrorPreset != 2 {
		t.Errorf("components.BadgeErrorPreset = %d, want 2", components.BadgeErrorPreset)
	}
	if components.BadgeInfoPreset != 3 {
		t.Errorf("components.BadgeInfoPreset = %d, want 3", components.BadgeInfoPreset)
	}
	if components.BadgeBrandPreset != 4 {
		t.Errorf("components.BadgeBrandPreset = %d, want 4", components.BadgeBrandPreset)
	}
	if components.BadgeMutedPreset != 5 {
		t.Errorf("components.BadgeMutedPreset = %d, want 5", components.BadgeMutedPreset)
	}
}
