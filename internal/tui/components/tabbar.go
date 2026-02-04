package components

import (
	"strings"

	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/tui/theme"
)

// TabBar renders a horizontal tab bar.
type TabBar struct {
	Tabs     []string
	Active   int
	Theme    theme.Theme
	Width    int
}

// View renders the tab bar.
func (tb TabBar) View() string {
	t := tb.Theme
	var parts []string

	for i, tab := range tb.Tabs {
		if i == tb.Active {
			styled := lipgloss.NewStyle().
				Foreground(t.Brand).
				Bold(true).
				Underline(true).
				Padding(0, 2).
				Render(tab)
			parts = append(parts, styled)
		} else {
			styled := lipgloss.NewStyle().
				Foreground(t.TextMuted).
				Padding(0, 2).
				Render(tab)
			parts = append(parts, styled)
		}
	}

	underline := lipgloss.NewStyle().
		Foreground(t.Border).
		Render(strings.Repeat("─", tb.Width))

	return lipgloss.JoinVertical(lipgloss.Left,
		strings.Join(parts, ""),
		underline,
	)
}

// MoveLeft moves to the previous tab.
func (tb *TabBar) MoveLeft() {
	if tb.Active > 0 {
		tb.Active--
	}
}

// MoveRight moves to the next tab.
func (tb *TabBar) MoveRight() {
	if tb.Active < len(tb.Tabs)-1 {
		tb.Active++
	}
}
