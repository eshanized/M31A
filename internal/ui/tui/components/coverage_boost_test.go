package components

import (
	"testing"

	"github.com/eshanized/M31A/internal/ui/tui/theme"
)

// --- Context ---

func TestContext_Fields(t *testing.T) {
	t.Parallel()
	th := theme.Dark()
	ctx := Context{
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
	ctx := Context{
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
	ctx := Context{
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
	ctx := NewContext(80, 24, th)
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
	types := []BadgeType{BadgeBrand, BadgeSuccess, BadgeError, BadgeWarning, BadgeInfo, BadgeNeutral}
	for _, bt := range types {
		badge := SimpleBadge{Text: "test", Type: bt, Theme: th}
		got := badge.Render()
		if got == "" {
			t.Errorf("SimpleBadge type %d should render non-empty", bt)
		}
	}
}

func TestSimpleBadge_Render_Compact(t *testing.T) {
	t.Parallel()
	th := theme.Dark()
	badge := SimpleBadge{Text: "test", Type: BadgeBrand, Compact: true, Theme: th}
	got := badge.Render()
	if got == "" {
		t.Error("Compact badge should render non-empty")
	}
}

func TestSimpleBadge_Render_NonCompact(t *testing.T) {
	t.Parallel()
	th := theme.Dark()
	badge := SimpleBadge{Text: "test", Type: BadgeBrand, Compact: false, Theme: th}
	got := badge.Render()
	if got == "" {
		t.Error("Non-compact badge should render non-empty")
	}
}

// --- Badge ---

func TestBadge_Render_Empty(t *testing.T) {
	t.Parallel()
	badge := Badge{}
	got := badge.Render()
	if got != "" {
		t.Error("Empty badge should render empty string")
	}
}

func TestBadge_Render_NonEmpty(t *testing.T) {
	t.Parallel()
	th := theme.Dark()
	badge := NewBadge("test", BadgeSuccessPreset, th)
	got := badge.Render()
	if got == "" {
		t.Error("Non-empty badge should render non-empty string")
	}
}

// --- NewBadge ---

func TestNewBadge_AllPresets(t *testing.T) {
	t.Parallel()
	th := theme.Dark()
	presets := []BadgePreset{
		BadgeSuccessPreset, BadgeWarningPreset, BadgeErrorPreset,
		BadgeInfoPreset, BadgeBrandPreset, BadgeMutedPreset,
	}
	for _, p := range presets {
		badge := NewBadge("test", p, th)
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
	badges := []Badge{
		NewBadge("fast", BadgeSuccessPreset, th),
		NewBadge("cheap", BadgeInfoPreset, th),
		NewBadge("safe", BadgeWarningPreset, th),
	}
	got := RenderBadges(badges)
	if got == "" {
		t.Error("RenderBadges should return non-empty")
	}
}

// --- CapabilityBadge ---

func TestCapabilityBadge_Various(t *testing.T) {
	t.Parallel()
	th := theme.Dark()
	caps := []string{"reasoning", "vision", "code", "fast"}
	for _, cap := range caps {
		badge := CapabilityBadge(cap, th)
		if badge.Label != cap {
			t.Errorf("CapabilityBadge label = %q, want %q", badge.Label, cap)
		}
	}
}

// --- StatusBadge ---

func TestStatusBadge_AllStatuses(t *testing.T) {
	t.Parallel()
	th := theme.Dark()
	statuses := []string{"done", "pass", "complete", "running", "thinking", "pending", "failed", "error", "warning", "skipped", "unknown"}
	for _, status := range statuses {
		badge := StatusBadge(status, th)
		if badge.Label != status {
			t.Errorf("StatusBadge(%q) label = %q", status, badge.Label)
		}
	}
}

// --- BadgeType constants ---

func TestBadgeType_Constants(t *testing.T) {
	t.Parallel()
	if BadgeBrand != 0 {
		t.Errorf("BadgeBrand = %d, want 0", BadgeBrand)
	}
	if BadgeSuccess != 1 {
		t.Errorf("BadgeSuccess = %d, want 1", BadgeSuccess)
	}
	if BadgeError != 2 {
		t.Errorf("BadgeError = %d, want 2", BadgeError)
	}
	if BadgeWarning != 3 {
		t.Errorf("BadgeWarning = %d, want 3", BadgeWarning)
	}
	if BadgeInfo != 4 {
		t.Errorf("BadgeInfo = %d, want 4", BadgeInfo)
	}
	if BadgeNeutral != 5 {
		t.Errorf("BadgeNeutral = %d, want 5", BadgeNeutral)
	}
}

// --- BadgePreset constants ---

func TestBadgePreset_Constants(t *testing.T) {
	t.Parallel()
	if BadgeSuccessPreset != 0 {
		t.Errorf("BadgeSuccessPreset = %d, want 0", BadgeSuccessPreset)
	}
	if BadgeWarningPreset != 1 {
		t.Errorf("BadgeWarningPreset = %d, want 1", BadgeWarningPreset)
	}
	if BadgeErrorPreset != 2 {
		t.Errorf("BadgeErrorPreset = %d, want 2", BadgeErrorPreset)
	}
	if BadgeInfoPreset != 3 {
		t.Errorf("BadgeInfoPreset = %d, want 3", BadgeInfoPreset)
	}
	if BadgeBrandPreset != 4 {
		t.Errorf("BadgeBrandPreset = %d, want 4", BadgeBrandPreset)
	}
	if BadgeMutedPreset != 5 {
		t.Errorf("BadgeMutedPreset = %d, want 5", BadgeMutedPreset)
	}
}
