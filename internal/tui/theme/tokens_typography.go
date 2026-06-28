package theme

import "github.com/charmbracelet/lipgloss"

// M31A Design System v2 — Typography
//
// Use typography as hierarchy.
// Headings, body, muted text, status, hints.
// Never rely on color alone.

var (
	// ── Typography Styles ────────────────────────────────────────────────────
	// Pre-computed styles for consistent text rendering.

	// StyleHeading is bold text in primary color — for screen titles.
	StyleHeading = lipgloss.NewStyle().
			Bold(true).
			Foreground(lipgloss.Color(TextPrimary))

	// StyleSubheading is bold text in secondary color — for section headers.
	StyleSubheading = lipgloss.NewStyle().
			Bold(true).
			Foreground(lipgloss.Color(TextSecondary))

	// StyleBody is normal-weight text in primary color — for main content.
	StyleBody = lipgloss.NewStyle().
			Foreground(lipgloss.Color(TextPrimary))

	// StyleCaption is muted text — for labels and metadata.
	StyleCaption = lipgloss.NewStyle().
			Foreground(lipgloss.Color(TextMuted))

	// StyleFaint is very dim text — for decorative elements.
	StyleFaint = lipgloss.NewStyle().
			Foreground(lipgloss.Color(TextFaint)).
			Faint(true)

	// StyleCode is accent-colored text — for inline code references.
	StyleCode = lipgloss.NewStyle().
			Foreground(lipgloss.Color(AccentPrimary))

	// StyleBrand is accent-colored text — for brand elements.
	StyleBrand = lipgloss.NewStyle().
			Foreground(lipgloss.Color(AccentPrimary))

	// StyleBrandBold is bold accent-colored text — for emphasis.
	StyleBrandBold = lipgloss.NewStyle().
			Foreground(lipgloss.Color(AccentPrimary)).
			Bold(true)

	// StyleSuccess is success-colored text.
	StyleSuccess = lipgloss.NewStyle().
			Foreground(lipgloss.Color(Success))

	// StyleError is error-colored text.
	StyleError = lipgloss.NewStyle().
			Foreground(lipgloss.Color(Error))

	// StyleWarning is warning-colored text.
	StyleWarning = lipgloss.NewStyle().
			Foreground(lipgloss.Color(Warning))

	// StyleThinking is thinking-colored italic text.
	StyleThinking = lipgloss.NewStyle().
			Foreground(lipgloss.Color(Thinking)).
			Italic(true)
)
