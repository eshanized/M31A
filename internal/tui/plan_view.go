package tui

import (
	"fmt"

	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/tui/theme"
)

// plan_view.go — view helpers for the Plan screen.
// The main View() method is in plan_model.go.

// renderPlanHeader builds the one-line plan header with task count,
// cost estimate, and model/provider badge.
func renderPlanHeader(t theme.Theme, taskCount int, cost float64, costEstimate, timeEstimate, modelName, provider string) string {
	header := lipgloss.NewStyle().Foreground(t.Brand).Bold(true).Render("📋 Plan") +
		lipgloss.NewStyle().Foreground(t.TextSecondary).Render(fmt.Sprintf(" · %d tasks", taskCount))
	if cost > 0 {
		header += lipgloss.NewStyle().Foreground(t.TextMuted).Render(fmt.Sprintf(" · ~$%.4f", cost))
	} else if costEstimate != "" {
		header += lipgloss.NewStyle().Foreground(t.TextMuted).Render(" · " + costEstimate)
	}
	if timeEstimate != "" {
		header += lipgloss.NewStyle().Foreground(t.TextMuted).Render(" · ~" + timeEstimate)
	}
	if modelName != "" {
		prov := ProviderShortName(provider)
		header += lipgloss.NewStyle().Foreground(t.TextMuted).Render(fmt.Sprintf(" · %s [%s]", modelName, prov))
	}
	return header
}

// renderPlanFooter builds the footer with contextual keybinding hints.
// The hints change based on whether the user is in normal, confirm, or refine mode.
func renderPlanFooter(t theme.Theme, confirmMode, refineMode bool, version int) string {
	sep := lipgloss.NewStyle().Foreground(t.BorderSubtle).Render("─")

	var hints string
	switch {
	case refineMode:
		hints = lipgloss.NewStyle().Foreground(t.TextSecondary).Render(
			"  [Ctrl+Enter] Submit  [Esc] Cancel")
	case confirmMode:
		hints = lipgloss.NewStyle().Foreground(t.Success).Bold(true).Render(
			"  [y] Accept & Execute") +
			lipgloss.NewStyle().Foreground(t.TextSecondary).Render(
				"  [r] Refine  [Esc] Back")
	default:
		hints = lipgloss.NewStyle().Foreground(t.TextSecondary).Render(
			"  [Enter] Review  [r] Refine  [Esc] Cancel")
	}

	versionStr := ""
	if version > 0 {
		versionStr = lipgloss.NewStyle().Foreground(t.TextMuted).Render(fmt.Sprintf("  v%d", version))
	}

	return sep + "\n" + hints + versionStr
}
