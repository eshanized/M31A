package tui

import (
	"fmt"

	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/ui/tui/theme"
)

// plan_view.go — view helpers for the Plan screen.
// The main View() method is in plan_model.go.

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
