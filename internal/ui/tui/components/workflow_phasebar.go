package components

import (
	"strings"

	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/ui/tui/theme"
)

// WorkflowPhaseBar renders a horizontal pipeline visualization.
type WorkflowPhaseBar struct {
	Phases    []string
	Current   string
	Completed map[string]bool
	Theme     theme.Theme
	Width     int
}

// View renders the phase pipeline: ○ Initialize → ● Discuss → ✓ Plan → ...
func (wpb WorkflowPhaseBar) View() string {
	t := wpb.Theme
	var parts []string

	for i, phase := range wpb.Phases {
		var icon string
		var style lipgloss.Style

		if wpb.Completed[phase] {
			icon = "✓"
			style = lipgloss.NewStyle().Foreground(t.Success).Bold(true)
		} else if phase == wpb.Current {
			icon = "●"
			style = lipgloss.NewStyle().Foreground(t.Brand).Bold(true)
		} else {
			icon = "○"
			style = lipgloss.NewStyle().Foreground(t.TextMuted)
		}

		label := style.Render(icon + " " + phase)
		parts = append(parts, label)

		if i < len(wpb.Phases)-1 {
			parts = append(parts, lipgloss.NewStyle().Foreground(t.TextMuted).Render("→"))
		}
	}

	return strings.Join(parts, " ")
}
