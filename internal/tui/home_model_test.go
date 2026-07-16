package tui

import (
	"strings"
	"testing"

	"github.com/eshanized/M31A/internal/tui/theme"
)

func TestHomeView_RendersLogo(t *testing.T) {
	tm := theme.NewManager(theme.ModeDark)
	m := NewHomeModel(tm.Current(), 80, 24, "test")
	m.SetDimensions(80, 24)
	view := m.View()
	if view == "" {
		t.Error("Home view should render non-empty view")
	}
}

func TestHomeView_RendersPromptInput(t *testing.T) {
	tm := theme.NewManager(theme.ModeDark)
	m := NewHomeModel(tm.Current(), 80, 24, "test")
	m.SetDimensions(80, 24)
	view := m.View()
	if !strings.Contains(view, "Type") && !strings.Contains(view, "prompt") {
		t.Error("Home view should contain prompt input hint")
	}
}

func TestHomeView_SetDimensions(t *testing.T) {
	tm := theme.NewManager(theme.ModeDark)
	m := NewHomeModel(tm.Current(), 80, 24, "test")
	m.SetDimensions(80, 24)
	view := m.View()
	if view == "" {
		t.Error("Home view should render non-empty after SetDimensions")
	}
}

func TestHomeView_SetDimensions_SmallTerminal(t *testing.T) {
	tm := theme.NewManager(theme.ModeDark)
	m := NewHomeModel(tm.Current(), 40, 10, "test")
	m.SetDimensions(40, 10)
	view := m.View()
	if view == "" {
		t.Error("Home view should render non-empty on small terminal")
	}
}
