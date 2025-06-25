package tui

import (
	"github.com/charmbracelet/lipgloss"
)

// renderQuickActions shows clickable action tiles.
func (m *ReplModel) renderQuickActions() string {
	t := m.theme

	type action struct {
		key         string
		label       string
		description string
		symbol      string
	}

	actions := []action{
		{key: "/settings", label: "Settings", description: "Configure", symbol: ">"},
		{key: "/models", label: "Models", description: "Browse", symbol: "*"},
		{key: "/resume", label: "Sessions", description: "Resume", symbol: "~"},
		{key: "/help", label: "Help", description: "Commands", symbol: "?"},
	}

	cards := make([]string, len(actions))
	for i, a := range actions {
		cardStyle := lipgloss.NewStyle().
			Background(t.Surface).
			Border(lipgloss.RoundedBorder()).
			BorderForeground(t.Border).
			Padding(0, 1).
			Width(18)

		symbolStyle := lipgloss.NewStyle().Foreground(t.Brand).Bold(true)
		keyStyle := lipgloss.NewStyle().Foreground(t.Brand)
		labelStyle := lipgloss.NewStyle().Foreground(t.TextPrimary)
		descStyle := lipgloss.NewStyle().Foreground(t.TextSecondary)

		header := symbolStyle.Render(a.symbol) + " " + keyStyle.Render(a.key)
		label := labelStyle.Render(a.label)
		desc := descStyle.Render(a.description)

		content := lipgloss.JoinVertical(lipgloss.Left, header, label, desc)
		cards[i] = cardStyle.Render(content)
	}

	return lipgloss.JoinHorizontal(lipgloss.Top, cards...)
}
