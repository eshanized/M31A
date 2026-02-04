package components

import (
	"strings"

	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/tui/theme"
)

// CodeBlock renders a syntax-highlighted code block with language label.
type CodeBlock struct {
	Language string
	Code     string
	Theme    theme.Theme
	Width    int
}

// View renders the code block.
func (cb CodeBlock) View() string {
	t := cb.Theme
	w := cb.Width
	if w < 20 {
		w = 80
	}

	var lines []string

	if cb.Language != "" {
		label := lipgloss.NewStyle().
			Foreground(t.TextMuted).
			Italic(true).
			PaddingLeft(2).
			Render(cb.Language)
		lines = append(lines, label)
	}

	codeLines := strings.Split(cb.Code, "\n")
	for _, line := range codeLines {
		styled := lipgloss.NewStyle().
			Foreground(t.Text).
			Background(t.CodeBG).
			Width(w).
			PaddingLeft(2).
			Render(line)
		lines = append(lines, styled)
	}

	return strings.Join(lines, "\n")
}
