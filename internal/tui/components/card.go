package components

import (
	"strings"

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

// CardVariant represents the visual variant of a card
type CardVariant int

const (
	CardPlain    CardVariant = iota // current: border only
	CardElevated                    // border + background + shadow
	CardHeader                      // filled header bar + border body
	CardMinimal                     // top gradient line only, no side borders
	CardInline                      // no borders, just background tint
)

// Card renders a reusable bordered panel with optional title.
// Width is required; if 0, the card may render at 0 width.
type Card struct {
	Title   string
	Content string
	Width   int
	Border  lipgloss.Border // theme.ThinBorder, theme.NormalBorder, theme.DoubleBorder
	Style   CardStyle
	Variant CardVariant
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
		footerStyle := lipgloss.NewStyle().
			Foreground(c.Theme.TextMuted).
			Italic(true)
		content = content + "\n" + footerStyle.Render(c.Footer)
	}

	switch c.Variant {
	case CardElevated:
		// Border + background + shadow
		style := lipgloss.NewStyle().
			Border(c.Border).
			BorderForeground(borderColor).
			Width(c.Width).
			Padding(0, 1).
			Background(lipgloss.Color(c.Theme.SurfaceElevated))
		return theme.RenderWithShadow(style.Render(content), c.Theme.ShadowColor, 1, 1)
	case CardHeader:
		// Filled header bar + border body
		if c.Title != "" {
			headerStyle := lipgloss.NewStyle().
				Background(borderColor).
				Foreground(c.Theme.BadgeForeground).
				Padding(0, 1).
				Bold(true)
			header := headerStyle.Render(c.Title)
			bodyStyle := lipgloss.NewStyle().
				Border(c.Border).
				BorderForeground(borderColor).
				Width(c.Width).
				Padding(0, 1)
			return header + "\n" + bodyStyle.Render(c.Content)
		}
	case CardMinimal:
		// Top gradient line only, no side borders
		gradientStyle := lipgloss.NewStyle().
			Foreground(c.Theme.Brand).
			Bold(true)
		gradientW := c.Width - 2
		if gradientW < 1 {
			gradientW = 1
		}
		gradientLine := gradientStyle.Render(strings.Repeat("─", gradientW))
		return gradientLine + "\n" + content
	case CardInline:
		// No borders, just background tint
		style := lipgloss.NewStyle().
			Width(c.Width).
			Padding(0, 1).
			Background(lipgloss.Color(c.Theme.Surface))
		return style.Render(content)
	}

	// Default: Plain card with border
	style := lipgloss.NewStyle().
		Border(c.Border).
		BorderForeground(borderColor).
		Width(c.Width).
		Padding(0, 1)
	return style.Render(content)
}
