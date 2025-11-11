package tui

import (
	"strings"

	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/tui/theme"
)

// ship_view.go — view helpers for the Ship screen.
// The main View() method is in ship_model.go.

// renderShipStatsGrid renders a 2-column stats layout for the ship summary.
func renderShipStatsGrid(t theme.Theme, rows [][2]string, colWidth int) string {
	var grid []string
	for _, row := range rows {
		left := lipgloss.NewStyle().Width(colWidth).Render(row[0])
		right := lipgloss.NewStyle().Width(colWidth).Render(row[1])
		grid = append(grid, lipgloss.JoinHorizontal(lipgloss.Left, left, right))
	}
	return strings.Join(grid, "\n")
}
