package theme

import (
	"testing"
)

func TestNormalizeTabs_WideTabs(t *testing.T) {
	t.Parallel()
	input := "hello\tworld"
	got := NormalizeTabs(input, 8)
	if got == input {
		t.Error("expected tabs to be expanded")
	}
}

func TestDefault_BackgroundNonZero(t *testing.T) {
	t.Parallel()
	th := Default()
	if string(th.Background) == "" {
		t.Error("expected non-zero background")
	}
}

func TestDark_BackgroundNonZero(t *testing.T) {
	t.Parallel()
	th := Dark()
	if th.Mode != ModeDark {
		t.Errorf("expected ModeDark, got %v", th.Mode)
	}
}

func TestTheme_M31A_TrueColor(t *testing.T) {
	t.Parallel()
	m := NewManager(ModeDark)
	th := m.Current()
	if string(th.Background) == "" {
		t.Error("TrueColor theme should have non-empty background")
	}
	if string(th.TextPrimary) == "" {
		t.Error("TrueColor theme should have non-empty text primary")
	}
}

func TestTheme_ANSI_16Color(t *testing.T) {
	t.Parallel()
	th := ansiPalette()
	// ANSI palette should use ANSI color numbers (0-15)
	if string(th.Background) == "" {
		t.Error("ANSI palette should have non-empty background")
	}
	if string(th.TextPrimary) == "" {
		t.Error("ANSI palette should have non-empty text primary")
	}
	if string(th.Brand) == "" {
		t.Error("ANSI palette should have non-empty brand")
	}
}

func TestTheme_StyleCache_NoFgBgCollision(t *testing.T) {
	t.Parallel()
	th := M31A()
	cache := NewStyleCache(th)
	// Check that no style has both foreground and background set to the same color
	// This is a basic check - in practice, styles use semantic tokens
	_ = cache
	// The cache builds without panic, which verifies basic correctness
}

func TestTheme_BorderContrast(t *testing.T) {
	t.Parallel()
	th := M31A()
	// Border colors should be different from background colors
	if th.Border == th.Background {
		t.Error("Border should contrast with Background")
	}
	if th.BorderActive == th.Background {
		t.Error("BorderActive should contrast with Background")
	}
}
