package components

import (
	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/tui/theme"
)

// CardStyle determines the border color of a Card.
type CardStyle int

const (
	CardDefault CardStyle = iota
	CardBrand
	CardSuccess
	CardError
	CardWarning
)

// Card renders a reusable bordered panel with optional title.
// Width is required; if 0, the card may render at 0 width.
type Card struct {
	Title   string
	Content string
	Width   int
	Border  lipgloss.Border // theme.ThinBorder, theme.NormalBorder, theme.DoubleBorder
	Style   CardStyle
	Icon    string // optional prefix icon
	Footer  string // optional footer
	Focused bool   // hover/focus state
	Theme   theme.Theme
}

// Render returns the card as a styled string.
func (c Card) Render() string {
	if c.Border == (lipgloss.Border{}) {
		c.Border = lipgloss.RoundedBorder()
	}

	s := theme.BuildSemanticStyles(c.Theme)

	// Determine border color from style
	borderColor := c.Theme.Border
	switch c.Style {
	case CardBrand:
		borderColor = c.Theme.Brand
	case CardSuccess:
		borderColor = c.Theme.Success
	case CardError:
		borderColor = c.Theme.Error
	case CardWarning:
		borderColor = c.Theme.Warning
	}

	// Handle focused state with brand border
	if c.Focused {
		borderColor = c.Theme.Brand
	}

	var content string
	if c.Title != "" {
		header := lipgloss.NewStyle().
			Foreground(borderColor).
			Bold(true).
			Render(c.Title)
		content = header + "\n" + c.Content
	} else {
		content = c.Content
	}

	// Add icon prefix if provided
	if c.Icon != "" {
		content = c.Icon + " " + content
	}

	// Add footer if provided
	if c.Footer != "" {
		content = content + "\n" + s.Muted.Italic(true).Render(c.Footer)
	}

	// Default: Plain card with border
	style := lipgloss.NewStyle().
		Border(c.Border).
		BorderForeground(borderColor).
		Width(c.Width).
		Padding(0, 1)
	return style.Render(content)
}
