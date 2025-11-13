package tui

import (
	"strings"

	"github.com/charmbracelet/lipgloss"
)

// renderDiffView renders the full diff viewer screen.
func renderDiffView(dm *DiffModel) string {
	t := dm.theme
	w := dm.width
	if w < 20 {
		w = 80
	}

	titleText := "Git Diff"
	if dm.title != "" {
		titleText = dm.title
	}
	title := lipgloss.NewStyle().Foreground(t.Brand).Bold(true).PaddingLeft(1).
		Render("  " + titleText)
	divider := lipgloss.NewStyle().Foreground(t.TextMuted).Render(strings.Repeat("─", w))

	body := ""
	if dm.diff == "" {
		body = lipgloss.NewStyle().Foreground(t.TextMuted).PaddingLeft(2).
			Render("No diff to display.")
	} else {
		body = dm.viewport.View()
	}

	// Legend with styled previews
	added := lipgloss.NewStyle().
		Background(t.DiffAddedBg).
		Foreground(t.DiffAdded).
		Render("+ added")
	removed := lipgloss.NewStyle().
		Background(t.DiffRemovedBg).
		Foreground(t.DiffRemoved).
		Render("- removed")
	legend := "  " + added + "  " + removed

	footer := lipgloss.NewStyle().Foreground(t.TextMuted).PaddingLeft(2).
		Render("j/k or ↑↓ scroll  esc close")

	return lipgloss.JoinVertical(lipgloss.Left,
		title, divider, body, "", legend, divider, footer)
}
