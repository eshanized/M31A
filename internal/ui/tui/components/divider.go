package components

import (
	"strings"

	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/ui/tui/theme"
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
	leftLen := remaining / 2
	if leftLen < 2 {
		leftLen = 2
	}
	rightLen := remaining - leftLen
	if rightLen < 0 {
		rightLen = 0
	}
	left := strings.Repeat(d.Theme.DividerChar, leftLen)
	right := strings.Repeat(d.Theme.DividerChar, rightLen)
	return style.Render(left + label + right)
}
