package components

import (
	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/tui/theme"
)

// LoadingIndicator renders a consistent loading state with spinner and label.
// Supports both centered (full screen) and inline (left-aligned) modes.
type LoadingIndicator struct {
	Label       string
	Theme       theme.Theme
	Width       int
	Height      int
	Inline      bool // If true, left-aligned; if false, centered
	PaddingLeft int
}

// Render returns the styled loading indicator.
func (l LoadingIndicator) Render() string {
	t := l.Theme
	if t.Brand == "" {
		t = theme.Default()
	}
	s := theme.BuildSemanticStyles(t)

	padLeft := l.PaddingLeft
	if padLeft == 0 && l.Inline {
		padLeft = 2
	}

	label := l.Label
	if label == "" {
		label = "Loading..."
	}

	// Spinner frame (static representation)
	spinner := s.SpinnerBrand.Render("⠋")
	content := spinner + " " + s.Loading.Render(label)

	if l.Inline {
		return lipgloss.NewStyle().
			PaddingLeft(padLeft).
			Render(content)
	}

	// Centered mode
	w := l.Width
	if w < 20 {
		w = 40
	}
	h := l.Height
	if h < 3 {
		h = 10
	}

	return lipgloss.Place(w, h, lipgloss.Center, lipgloss.Center, content)
}
