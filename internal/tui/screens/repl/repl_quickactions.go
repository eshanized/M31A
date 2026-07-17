package repl

import (
	"strings"

	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/tui/tuitypes"
)

// repl_quickactions.go — quick-actions dropdown overlay (ctrl+q).
//
// The quick-actions chip ("quick actions ctrl+q") is embedded directly in
// the input separator shelf (see repl_view.go) so it consumes no extra row.
// This file only handles the expanded dropdown that ctrl+q toggles.

// quickAction represents a single quick action item.
type quickAction struct {
	key  string
	desc string
}

// getQuickActions returns context-aware quick actions based on current state.
func (m *ReplModel) getQuickActions() []quickAction {
	// Base actions always shown
	actions := []quickAction{
		{"/chat", "new chat session"},
		{"/help", "list all commands"},
	}

	// Add context-specific actions
	if m.changedFiles > 0 {
		actions = append(actions, quickAction{"/diff", "review changes"})
	}

	if len(m.messages) > 0 {
		actions = append(actions, quickAction{"/compress", "compress context"})
		actions = append(actions, quickAction{"/history", "conversation log"})
	}

	// Add workflow actions if in a workflow
	actions = append(actions, quickAction{"/workflow", "start workflow"})
	actions = append(actions, quickAction{"/optimize", "suggest model"})

	return actions
}

// renderQuickActionsOverlay renders the expanded quick-actions list as a
// dropdown-style overlay. Anchored to the bottom of the viewport by
// compositeOverlays in View()/ViewContent.
func (m *ReplModel) renderQuickActionsOverlay(width int) string {
	t := m.theme

	items := m.getQuickActions()

	var lines []string
	for _, item := range items {
		slash := lipgloss.NewStyle().Foreground(t.Brand).Render(item.key)
		desc := lipgloss.NewStyle().Foreground(t.TextMuted).Render("  " + item.desc)
		line := "  " + slash + desc
		if lipgloss.Width(line) > width {
			line = tuitypes.TruncateWithEllipsis(line, width)
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
