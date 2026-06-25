package components

import (
	"strings"

	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/tui/theme"
)

// HintBar renders a consistent keyboard hints footer.
// Used across all standalone view screens for uniform footer appearance.
type HintBar struct {
	Hints       []string // e.g., ["↑↓ Navigate", "enter Select", "esc Back"]
	Theme       theme.Theme
	PaddingLeft int
}

// Render returns the styled hint bar.
func (h HintBar) Render() string {
	t := h.Theme
	if t.Brand == "" {
		t = theme.Default()
	}

	padLeft := h.PaddingLeft
	if padLeft == 0 {
		padLeft = 2
	}

	if len(h.Hints) == 0 {
		return ""
	}

	// Format each hint with brackets
	var formatted []string
	for _, hint := range h.Hints {
		formatted = append(formatted, "["+hint+"]")
	}

	return lipgloss.NewStyle().
		Foreground(t.TextMuted).
		PaddingLeft(padLeft).
		Render(strings.Join(formatted, " "))
}

// HintBarFromMap creates a HintBar from a map of key->action pairs.
// Keys are sorted for consistent display.
func HintBarFromMap(hints map[string]string, t theme.Theme, paddingLeft int) HintBar {
	var keys []string
	for k := range hints {
		keys = append(keys, k)
	}

	var formatted []string
	for _, k := range keys {
		formatted = append(formatted, k+" "+hints[k])
	}

	return HintBar{
		Hints:       formatted,
		Theme:       t,
		PaddingLeft: paddingLeft,
	}
}
