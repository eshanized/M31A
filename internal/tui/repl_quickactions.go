package tui

import (
	"strings"

	"github.com/charmbracelet/lipgloss"
)

// repl_quickactions.go — quick-actions dropdown overlay (ctrl+q).
//
// The quick-actions chip ("quick actions ctrl+q") is embedded directly in
// the input separator shelf (see repl_view.go) so it consumes no extra row.
// This file only handles the expanded dropdown that ctrl+q toggles.

// renderQuickActionsOverlay renders the expanded quick-actions list as a
// dropdown-style overlay. Anchored to the bottom of the viewport by
// compositeOverlays in View()/ViewContent.
func (m *ReplModel) renderQuickActionsOverlay(width int) string {
	t := m.theme

	items := []struct {
		key  string
		desc string
	}{
		{"/workflow", "start workflow"},
		{"/optimize", "suggest model"},
		{"/compress", "compress context"},
		{"/history", "conversation log"},
	}

	var lines []string
	for _, item := range items {
		slash := lipgloss.NewStyle().Foreground(t.Brand).Render(item.key)
		desc := lipgloss.NewStyle().Foreground(t.TextMuted).Render("  " + item.desc)
		line := "  " + slash + desc
		if lipgloss.Width(line) > width {
			line = TruncateWithEllipsis(line, width)
		}
		lines = append(lines, line)
	}

	box := lipgloss.NewStyle().
		Border(lipgloss.RoundedBorder()).
		BorderForeground(t.Brand).
		Width(width - 2).
		Render(strings.Join(lines, "\n"))

	return box
}
