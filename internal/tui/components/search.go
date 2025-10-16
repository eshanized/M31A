package components

import (
	"github.com/charmbracelet/lipgloss"
)

// RenderSearchBar renders a search input with a label.
func RenderSearchBar(label, query string, width int, brandColor, textColor lipgloss.Color) string {
	labelStyle := lipgloss.NewStyle().
		Foreground(brandColor).
		Bold(true)
	prompt := labelStyle.Render(label)

	input := lipgloss.NewStyle().
		Foreground(textColor).
		Render(query)

	cursor := lipgloss.NewStyle().
		Foreground(brandColor).
		Render("│")

	searchRow := lipgloss.JoinHorizontal(lipgloss.Center, prompt, input, cursor)

	style := lipgloss.NewStyle().
		Foreground(textColor).
		Padding(0, 1).
		Width(width - 2)

	return style.Render(searchRow)
}
