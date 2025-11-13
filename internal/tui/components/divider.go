package components

import (
	"strings"

	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/tui/theme"
)

// SectionDivider renders a horizontal divider line with an optional title.
// Without title: ────────────────────────────
// With title:    ── title ───────────────────
type SectionDivider struct {
	Title string
	Width int
	Theme theme.Theme
}

// Render returns the divider as a styled string.
func (d SectionDivider) Render() string {
	if d.Width < 4 {
		return ""
	}
	style := lipgloss.NewStyle().
		Foreground(d.Theme.TextMuted).
		Faint(true)
	if d.Title == "" {
		return style.Render(strings.Repeat(d.Theme.DividerChar, d.Width))
	}
	// ── title ────────────────────────────
	label := " " + d.Title + " "
	remaining := d.Width - lipgloss.Width(label)
	if remaining <= 0 {
		return label
	}
	left := strings.Repeat(d.Theme.DividerChar, max(2, remaining/2-1))
	right := strings.Repeat(d.Theme.DividerChar, remaining-len(left)-1)
	return style.Render(
		d.Theme.DividerChar + left + label + right + d.Theme.DividerChar,
	)
}

func max(a, b int) int {
	if a > b {
		return a
	}
	return b
}
