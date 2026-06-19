package components

import (
	"strings"

	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/tui/theme"
)

// RenderFocusRing wraps content with a visual focus indicator.
// When focused: brand-colored rounded border with a ▸ indicator.
// When unfocused: subtle border without indicator.
func RenderFocusRing(content string, focused bool, width int, t theme.Theme) string {
	if focused {
		indicator := lipgloss.NewStyle().Foreground(t.Brand).Bold(true).Render("▸")
		style := lipgloss.NewStyle().
			Border(lipgloss.RoundedBorder()).
			BorderForeground(t.Brand).
			Width(width).
			Padding(0, 1)
		// Prepend focus indicator to first line
		lines := strings.Split(content, "\n")
		if len(lines) > 0 {
			lines[0] = indicator + " " + lines[0]
		}
		return style.Render(strings.Join(lines, "\n"))
	}

	style := lipgloss.NewStyle().
		Border(lipgloss.RoundedBorder()).
		BorderForeground(t.BorderSubtle).
		Width(width).
		Padding(0, 1)
	return style.Render(content)
}

// RenderFocusRingSimple renders a minimal focus indicator — just a border change.
func RenderFocusRingSimple(content string, focused bool, width int, t theme.Theme) string {
	borderColor := t.BorderSubtle
	if focused {
		borderColor = t.Brand
	}
	style := lipgloss.NewStyle().
		Border(lipgloss.RoundedBorder()).
		BorderForeground(borderColor).
		Width(width).
		Padding(0, 1)
	return style.Render(content)
}
