package tui

import (
	"strings"

	"github.com/charmbracelet/lipgloss"
)

// repl_quickactions.go — quick action panel shown below messages when no active workflow.

// renderQuickActionsPanel renders a quick actions panel suggesting next steps.
// Only shown when there are messages but no workflow is active.
func (m *ReplModel) renderQuickActionsPanel(width int) string {
	if m.streaming {
		return ""
	}
	if len(m.messages) == 0 {
		return ""
	}

	t := m.theme

	// Collapsed state: single-line hint
	if m.quickActionsCollapsed {
		return lipgloss.NewStyle().Foreground(t.TextMuted).Italic(true).
			Render("  Quick actions (ctrl+q to expand)")
	}

	items := []struct {
		key  string
		desc string
	}{
		{"/workflow", "start workflow"},
		{"/optimize", "suggest model"},
		{"/compress", "compress context"},
		{"/history", "conversation log"},
	}

	var parts []string
	for _, item := range items {
		keyS := lipgloss.NewStyle().Foreground(t.Brand).Render(item.key)
		descS := lipgloss.NewStyle().Foreground(t.TextMuted).Render(" " + item.desc)
		parts = append(parts, "  "+keyS+descS)
	}

	joined := strings.Join(parts, "  ")
	if lipgloss.Width(joined) > width {
		joined = strings.Join(parts[:3], "  ")
	}

	return lipgloss.NewStyle().Foreground(t.TextMuted).Render(joined)
}
