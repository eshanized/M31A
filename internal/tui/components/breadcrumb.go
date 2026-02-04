package components

import (
	"strings"

	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/tui/theme"
)

// Breadcrumb renders a navigation breadcrumb trail.
type Breadcrumb struct {
	Parts []string
	Theme theme.Theme
	Width int
}

// View renders the breadcrumb.
func (bc Breadcrumb) View() string {
	t := bc.Theme
	if len(bc.Parts) == 0 {
		return ""
	}

	var parts []string
	for i, part := range bc.Parts {
		if i == len(bc.Parts)-1 {
			parts = append(parts, lipgloss.NewStyle().
				Foreground(t.Brand).Bold(true).Render(part))
		} else {
			parts = append(parts, lipgloss.NewStyle().
				Foreground(t.TextMuted).Render(part))
		}
	}

	sep := lipgloss.NewStyle().Foreground(t.TextMuted).Render(" › ")
	return strings.Join(parts, sep)
}
