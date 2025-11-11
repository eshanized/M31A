package tui

import (
	"math"
	"strings"

	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/tui/theme"
)

// execute_view.go — view helpers for the Execute screen.
// The main View() method is in execute_model.go.

// renderProgressBar renders a [████░░░░] progress bar using theme block chars.
func renderProgressBar(t theme.Theme, done, total int, width int) string {
	if total == 0 || width < 3 {
		return ""
	}
	pct := float64(done) / float64(total)
	barWidth := width
	filled := int(math.Round(pct * float64(barWidth)))
	if filled < 0 {
		filled = 0
	}
	if filled > barWidth {
		filled = barWidth
	}
	bar := lipgloss.NewStyle().Foreground(t.Brand).Render(
		strings.Repeat(t.BlockFull, filled))
	empty := lipgloss.NewStyle().Foreground(t.Surface).Render(
		strings.Repeat(t.BlockLow, barWidth-filled))
	return "[" + bar + empty + "]"
}
