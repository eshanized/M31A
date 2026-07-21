package components

import (
	"github.com/eshanized/M31A/internal/ui/tui/theme"
)

// ErrorBanner renders a consistent error message with red styling.
// Used across views for uniform error display.
type ErrorBanner struct {
	Message     string
	Theme       theme.Theme
	Prefix      string // Custom prefix (default: "! ")
	PaddingLeft int
	Warning     bool // If true, use warning color instead of error
}

// Render returns the styled error banner.
func (e ErrorBanner) Render() string {
	t := e.Theme
	if t.Brand == "" {
		t = theme.Default()
	}
	s := theme.BuildSemanticStyles(t)

	prefix := e.Prefix
	if prefix == "" {
		if e.Warning {
			prefix = "⚠ "
		} else {
			prefix = "! "
		}
	}

	padLeft := e.PaddingLeft
	if padLeft == 0 {
		padLeft = 2
	}

	style := s.ErrorBanner
	if e.Warning {
		style = s.WarningBanner
	}

	return style.PaddingLeft(padLeft).Render(prefix + e.Message)
}

// InlineEmptyState renders a simple inline empty state message.
// Used when a full EmptyState component is too heavy.
type InlineEmptyState struct {
	Message     string
	Theme       theme.Theme
	PaddingLeft int
}

// Render returns the styled inline empty state.
func (i InlineEmptyState) Render() string {
	t := i.Theme
	if t.Brand == "" {
		t = theme.Default()
	}
	s := theme.BuildSemanticStyles(t)

	padLeft := i.PaddingLeft
	if padLeft == 0 {
		padLeft = 2
	}

	return s.Muted.PaddingLeft(padLeft).Render(i.Message)
}
