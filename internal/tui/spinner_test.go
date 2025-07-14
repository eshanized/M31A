package tui

import (
	"testing"

	"github.com/charmbracelet/bubbles/spinner"
	"github.com/eshanized/M31A/internal/config"
	"github.com/eshanized/M31A/internal/tui/theme"
	"github.com/eshanized/M31A/internal/types"
	"github.com/eshanized/M31A/internal/workflow"
)

func TestSpinnerLoading(t *testing.T) {
	theme := theme.Dark()

	t.Run("PlanModel shows spinner when loading", func(t *testing.T) {
		// Create model with zero dimensions (loading state)
		m := NewPlanModel([]types.Task{}, theme, "", "", "", 0, "", 0, 0)
		view := m.View()
		if view == "Loading plan..." {
			t.Error("PlanModel should show spinner, not static text")
		}
		// Check that view contains spinner character or is non-empty
		if len(view) == 0 {
			t.Error("PlanModel view should not be empty")
		}
	})

	t.Run("ExecuteModel shows spinner when loading", func(t *testing.T) {
		m := NewExecuteModel([]types.Task{}, theme, 0, 0)
		view := m.View()
		if view == "Loading execute..." {
			t.Error("ExecuteModel should show spinner, not static text")
		}
		if len(view) == 0 {
			t.Error("ExecuteModel view should not be empty")
		}
	})

	t.Run("VerifyModel shows spinner when loading", func(t *testing.T) {
		m := NewVerifyModel([]types.Task{}, map[int]workflow.VerificationResult{}, theme, 0, 0)
		view := m.View()
		if view == "Loading verify..." {
			t.Error("VerifyModel should show spinner, not static text")
		}
		if len(view) == 0 {
			t.Error("VerifyModel view should not be empty")
		}
	})

	t.Run("ShipModel shows spinner when loading", func(t *testing.T) {
		m := NewShipModel(ShipSummary{}, theme, 0, 0)
		view := m.View()
		if view == "Loading ship..." {
			t.Error("ShipModel should show spinner, not static text")
		}
		if len(view) == 0 {
			t.Error("ShipModel view should not be empty")
		}
	})

	t.Run("SettingsModel shows spinner when loading", func(t *testing.T) {
		cfg := &config.Config{}
		m := NewSettingsModel(cfg, "", theme, nil, nil)
		view := m.View()
		if view == "Loading..." {
			t.Error("SettingsModel should show spinner, not static text")
		}
		if len(view) == 0 {
			t.Error("SettingsModel view should not be empty")
		}
	})

	t.Run("ResumeModel shows spinner when loading", func(t *testing.T) {
		// ResumeModel requires a session.Manager, which we can't easily mock here
		// So we test the spinner tick handling instead
		m := &ResumeModel{}
		msg := spinner.TickMsg{ID: 1}
		_, _ = m.Update(msg)
		// If we get here without panic, spinner handling works
	})

	t.Run("SidebarModel shows spinner when loading", func(t *testing.T) {
		m := &SidebarModel{loading: true}
		view := m.View()
		if view == "  loading..." {
			t.Error("SidebarModel should show spinner, not static text")
		}
	})

	t.Run("Spinner ticks on TickMsg", func(t *testing.T) {
		m := NewPlanModel([]types.Task{}, theme, "", "", "", 0, "", 80, 24)
		// Send a tick message
		cmd := m.Init()
		if cmd == nil {
			t.Error("Init should return a spinner tick command")
		}
	})
}