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
// Uses semantic styles for consistent appearance across themes.
func RenderCodeBlock(code, language string, cache *theme.StyleCache, width int) string {
	if width < 20 {
		width = 80
	}
	s := cache.S

	// Language label — refined pill badge
	var headerLeft string
	if language != "" {
		headerLeft = s.CodeKeyword.
			Border(lipgloss.RoundedBorder()).
			BorderForeground(cache.Theme.Brand).
			Padding(0, 1).
			Render(language)
	}

	// Copy hint — subtle, right-aligned
	copyHint := s.Muted.Render("c  copy")

	var header string
	if headerLeft != "" {
		leftW := lipgloss.Width(headerLeft)
		rightW := lipgloss.Width(copyHint)
		gap := width - leftW - rightW - 4
		if gap < 1 {
			gap = 1
		}
		header = headerLeft + strings.Repeat(" ", gap) + copyHint
	} else {
		header = copyHint
	}

	codeLines := renderCodeLines(code, language, cache, width)
	return header + "\n" + codeLines
}

func renderCodeLines(code, language string, cache *theme.StyleCache, width int) string {
	highlighted := HighlightCode(code, language, cache.Theme)
	codeLines := strings.Split(highlighted, "\n")

	lineNumWidth := len(fmt.Sprintf("%d", len(codeLines)))
	if lineNumWidth < 2 {
		lineNumWidth = 2
	}

	s := cache.S
	codeWidth := width - lineNumWidth - 3

	var lines []string
	for i, line := range codeLines {
		lineNum := s.CodeLineNum.
			Width(lineNumWidth).
			Align(lipgloss.Right).
			Render(fmt.Sprintf("%d", i+1))
		styled := s.CodeBlock.
			Width(codeWidth).
			Render(" " + line)
		lines = append(lines, lipgloss.JoinHorizontal(lipgloss.Top, lineNum, styled))
	}

	return strings.Join(lines, "\n")
}

func chromaStyleFromTheme(t theme.Theme) *chroma.Style {
	return styles.Get("monokai")
}
