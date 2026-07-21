package components

import (
	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/ui/tui/theme"
)

// ScreenTitle renders a consistent screen title with brand styling.
// Used across all standalone view screens for uniform header appearance.
type ScreenTitle struct {
	Text        string
	Theme       theme.Theme
	PaddingLeft int
}

// Render returns the styled screen title.
func (s ScreenTitle) Render() string {
	t := s.Theme
	if t.Brand == "" {
		t = theme.Default()
	}

	padLeft := s.PaddingLeft
	if padLeft == 0 {
		padLeft = 2
	}

	return lipgloss.NewStyle().
		Foreground(t.Brand).
		Bold(true).
		PaddingLeft(padLeft).
		Render(s.Text)
}
