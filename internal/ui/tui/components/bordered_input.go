package components

import (
	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/ui/tui/theme"
)

// BorderedInput renders a text input with a consistent bordered style.
// Used across views for uniform input box appearance.
type BorderedInput struct {
	Content     string // The input content to display
	Width       int
	Focused     bool
	Theme       theme.Theme
	PaddingLeft int
	Placeholder string // Shown when content is empty
}

// Render returns the styled bordered input.
func (bi BorderedInput) Render() string {
	t := bi.Theme
	if t.Brand == "" {
		t = theme.Default()
	}

	padLeft := bi.PaddingLeft
	w := bi.Width
	if w < 10 {
		w = 30
	}

	// Determine border color based on focus state
	borderColor := t.Border
	if bi.Focused {
		borderColor = t.Brand
	}

	// Get input content
	content := bi.Content
	if content == "" && bi.Placeholder != "" {
		content = lipgloss.NewStyle().Foreground(t.TextMuted).Italic(true).Render(bi.Placeholder)
	}

	return lipgloss.NewStyle().
		Border(lipgloss.RoundedBorder()).
		BorderForeground(borderColor).
		Padding(0, 1).
		Width(w).
		MarginLeft(padLeft).
		Render(content)
}

// BorderedInputWithOptions renders a bordered input with additional options.
type BorderedInputWithOptions struct {
	Content     string
	Width       int
	Focused     bool
	Theme       theme.Theme
	PaddingLeft int
	Placeholder string
	Label       string // Optional label above the input
	Error       string // Optional error message below the input
}

// Render returns the styled bordered input with label and error.
func (bio BorderedInputWithOptions) Render() string {
	t := bio.Theme
	if t.Brand == "" {
		t = theme.Default()
	}

	var parts []string

	// Label
	if bio.Label != "" {
		label := lipgloss.NewStyle().
			Foreground(t.TextPrimary).
			Bold(true).
			Render(bio.Label)
		parts = append(parts, label)
	}

	// Input
	input := BorderedInput{
		Content:     bio.Content,
		Width:       bio.Width,
		Focused:     bio.Focused,
		Theme:       bio.Theme,
		PaddingLeft: bio.PaddingLeft,
		Placeholder: bio.Placeholder,
	}
	parts = append(parts, input.Render())

	// Error
	if bio.Error != "" {
		errorMsg := lipgloss.NewStyle().
			Foreground(t.Error).
			PaddingLeft(bio.PaddingLeft + 2).
			Render(bio.Error)
		parts = append(parts, errorMsg)
	}

	result := ""
	for i, part := range parts {
		if i > 0 {
			result += "\n"
		}
		result += part
	}

	return result
}
