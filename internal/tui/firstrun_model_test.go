package tui

import (
	"testing"

	"github.com/eshanized/M31A/internal/config"
	"github.com/eshanized/M31A/internal/tui/theme"
)

func TestFirstRun_WelcomeStep_Renders(t *testing.T) {
	tm := theme.NewManager(theme.ModeDark)
	cfg := &config.Config{}
	m := NewFirstRunModel(tm.Current(), nil, cfg, "test", nil)
	m.SetDimensions(80, 24)
	view := m.View()
	if view == "" {
		t.Error("Welcome step should render non-empty view")
	}
}

func TestFirstRun_SetDimensions_HandlesZeroDims(t *testing.T) {
	tm := theme.NewManager(theme.ModeDark)
	cfg := &config.Config{}
	m := NewFirstRunModel(tm.Current(), nil, cfg, "test", nil)
	m.SetDimensions(0, 0)
	view := m.View()
	_ = view
}

func TestFirstRun_SetDimensions_HandlesSmallDims(t *testing.T) {
	tm := theme.NewManager(theme.ModeDark)
	cfg := &config.Config{}
	m := NewFirstRunModel(tm.Current(), nil, cfg, "test", nil)
	m.SetDimensions(20, 5)
	view := m.View()
	if view == "" {
		t.Error("Small dimensions should still render non-empty view")
	}
}
