package tui

import (
	tea "github.com/charmbracelet/bubbletea"
	"github.com/charmbracelet/lipgloss"
)

// ThinkingBlock represents an expandable thinking block in the REPL.
// It delegates display to components.ThinkingBlock but manages its
// expand/collapse state here.

// renderThinkingToggleHint renders a "▶ Show thinking" / "▼ Hide thinking" toggle.
func (m *ReplModel) renderThinkingToggleHint(blockIndex int, durationMs int64) string {
	t := m.theme
	block, ok := m.thinkingBlocks[blockIndex]
	if !ok {
		return ""
	}

	label := ""
	if block.IsExpanded() {
		label = "▼ Hide thinking"
		if durationMs > 0 {
			label += " · " + formatDurationMs(durationMs) + "s"
		}
	} else {
		label = "▶ Show thinking"
		if durationMs > 0 {
			label += " · " + formatDurationMs(durationMs) + "s"
		}
	}

	return lipgloss.NewStyle().Foreground(t.Thinking).Render(label)
}

// handleThinkingToggle processes a ThinkingBlockToggleMsg.
func (m *ReplModel) handleThinkingToggle(msg ThinkingBlockToggleMsg) tea.Cmd {
	block, ok := m.thinkingBlocks[msg.Index]
	if ok {
		block.Toggle()
		m.renderMessages()
	}
	return nil
}
