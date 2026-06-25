package components

import (
	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/tui/theme"
)

// CursorIndicator renders a selection cursor marker (▶).
// Used in selectable lists to indicate the currently selected item.
type CursorIndicator struct {
	Selected bool
	Theme    theme.Theme
	Char     string // Custom character (default: "▶")
}

// Render returns the styled cursor indicator.
func (c CursorIndicator) Render() string {
	t := c.Theme
	if t.Brand == "" {
		t = theme.Default()
	}

	char := c.Char
	if char == "" {
		char = "▶"
	}

	if !c.Selected {
		// Return fixed-width space for alignment
		return lipgloss.NewStyle().
			Width(lipgloss.Width(char)).
			Render(" ")
	}

	return lipgloss.NewStyle().
		Foreground(t.Brand).
		Render(char)
}

// CursorWidth returns the width of the cursor character for layout purposes.
func CursorWidth(char string) int {
	if char == "" {
		char = "▶"
	}
	return lipgloss.Width(char)
}
