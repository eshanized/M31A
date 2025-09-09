package components

import (
	"testing"

	"github.com/eshanized/M31A/internal/tui/theme"
)

func TestRenderStarfield_Deterministic(t *testing.T) {
	t1 := theme.Dark()
	r1 := RenderStarfield(40, 10, 42, t1)
	r2 := RenderStarfield(40, 10, 42, t1)
	if r1 != r2 {
		t.Error("same seed should produce identical output")
	}
}

func TestRenderStarfield_DifferentSeeds(t *testing.T) {
	t1 := theme.Dark()
	r1 := RenderStarfield(40, 10, 42, t1)
	r2 := RenderStarfield(40, 10, 99, t1)
	if r1 == r2 {
		t.Error("different seeds should produce different output")
	}
}

func TestRenderStarfield_ZeroWidth(t *testing.T) {
	result := RenderStarfield(0, 10, 42, theme.Dark())
	if result != "" {
		t.Error("zero width should return empty string")
	}
}

func TestRenderStarfield_ZeroHeight(t *testing.T) {
	result := RenderStarfield(40, 0, 42, theme.Dark())
	if result != "" {
		t.Error("zero height should return empty string")
	}
}

func TestRenderStarfield_Dimensions(t *testing.T) {
	result := RenderStarfield(40, 10, 42, theme.Dark())
	if result == "" {
		t.Error("starfield should not be empty")
	}
	lines := 0
	for range result {
		if result == "" {
			break
		}
	}
	// Count newlines
	for i := 0; i < len(result); i++ {
		if result[i] == '\n' {
			lines++
		}
	}
	// Should have height-1 newlines (last line has no trailing newline)
	if lines != 9 {
		t.Errorf("expected 9 newlines for height=10, got %d", lines)
	}
}
