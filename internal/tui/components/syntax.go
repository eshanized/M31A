package components

import (
	"bytes"
	"fmt"
	"strings"

	"github.com/alecthomas/chroma"
	"github.com/alecthomas/chroma/formatters"
	"github.com/alecthomas/chroma/lexers"
	"github.com/alecthomas/chroma/styles"
	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/tui/theme"
)

// HighlightCode applies syntax highlighting to code using chroma.
// Returns the highlighted string with ANSI escape codes.
func HighlightCode(code, language string, t theme.Theme) string {
	lexer := lexers.Get(language)
	if lexer == nil {
		lexer = lexers.Analyse(code)
	}
	if lexer == nil {
		lexer = lexers.Fallback
	}
	lexer = chroma.Coalesce(lexer)

	style := chromaStyleFromTheme(t)
	formatter := formatters.Get("terminal256")
	if formatter == nil {
		formatter = formatters.Fallback
	}

	iterator, err := lexer.Tokenise(nil, code)
	if err != nil {
		return code
	}

	var buf bytes.Buffer
	if err := formatter.Format(&buf, style, iterator); err != nil {
		return code
	}

	return strings.TrimRight(buf.String(), "\n")
}

// DetectLanguage detects the programming language from code content.
func DetectLanguage(code string) string {
	lexer := lexers.Analyse(code)
	if lexer != nil {
		cfg := lexer.Config()
		if cfg != nil {
			return cfg.Name
		}
	}
	return ""
}

// RenderCodeBlock renders a syntax-highlighted code block with language label and line numbers.
func RenderCodeBlock(code, language string, t theme.Theme, width int) string {
	if width < 20 {
		width = 80
	}

	var headerParts []string

	if language != "" {
		label := lipgloss.NewStyle().
			Foreground(t.BadgeForeground).
			Background(t.Brand).
			Padding(0, 1).
			Bold(true).
			Render(language)
		headerParts = append(headerParts, label)
	}

	copyHint := lipgloss.NewStyle().
		Foreground(t.TextMuted).
		Faint(true).
		Render("⎚ press c to copy")

	if len(headerParts) > 0 {
		left := strings.Join(headerParts, " ")
		leftW := lipgloss.Width(left)
		rightW := lipgloss.Width(copyHint)
		gap := width - leftW - rightW - 4
		if gap < 1 {
			gap = 1
		}
		header := left + strings.Repeat(" ", gap) + copyHint
		return header + "\n" + renderCodeLines(code, language, t, width)
	}

	return copyHint + "\n" + renderCodeLines(code, language, t, width)
}

func renderCodeLines(code, language string, t theme.Theme, width int) string {
	highlighted := HighlightCode(code, language, t)
	codeLines := strings.Split(highlighted, "\n")

	lineNumWidth := len(fmt.Sprintf("%d", len(codeLines)))
	if lineNumWidth < 2 {
		lineNumWidth = 2
	}

	var lines []string
	for i, line := range codeLines {
		lineNum := lipgloss.NewStyle().
			Foreground(t.TextMuted).
			Faint(true).
			Width(lineNumWidth).
			Align(lipgloss.Right).
			Render(fmt.Sprintf("%d", i+1))
		styled := lipgloss.NewStyle().
			Background(t.CodeBG).
			Width(width - lineNumWidth - 3).
			Render(" " + line)
		lines = append(lines, lipgloss.JoinHorizontal(lipgloss.Top, lineNum, styled))
	}

	return strings.Join(lines, "\n")
}

func chromaStyleFromTheme(t theme.Theme) *chroma.Style {
	if t.Mode == theme.ModeLight {
		return styles.Get("github")
	}
	return styles.Get("monokai")
}
