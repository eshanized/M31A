package components

import (
	"strings"

	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/ui/tui/theme"
)

// LightweightMarkdown provides fast inline markdown rendering for streaming
// content. Unlike Glamour, it does not re-parse the entire document on every
// tick — it applies simple regex-free string scans for the most common
// inline formatting primitives. This keeps streaming at 10fps without
// the O(n) cost of a full markdown pipeline.
type LightweightMarkdown struct {
	boldStyle     lipgloss.Style
	italicStyle   lipgloss.Style
	codeStyle     lipgloss.Style
	linkStyle     lipgloss.Style
	linkTextStyle lipgloss.Style
}

// NewLightweightMarkdown creates a parser styled with the given theme.
func NewLightweightMarkdown(t theme.Theme) *LightweightMarkdown {
	return &LightweightMarkdown{
		boldStyle:     lipgloss.NewStyle().Foreground(t.TextPrimary).Bold(true),
		italicStyle:   lipgloss.NewStyle().Foreground(t.TextPrimary).Italic(true),
		codeStyle:     lipgloss.NewStyle().Foreground(t.Brand).Background(t.CodeBG),
		linkStyle:     lipgloss.NewStyle().Foreground(t.Brand).Underline(true),
		linkTextStyle: lipgloss.NewStyle().Foreground(t.Brand).Bold(true),
	}
}

// Render applies lightweight inline markdown formatting to text.
// Supported: **bold**, *italic*, `inline code`, ```code blocks```,
// [text](url), and # headings (first line only).
// This is intentionally fast — no nested parsing, no tree walking.
func (lm *LightweightMarkdown) Render(text string, width int) string {
	if text == "" {
		return ""
	}

	lines := strings.Split(text, "\n")
	var out []string

	for _, line := range lines {
		rendered := lm.renderLine(line, width)
		out = append(out, rendered)
	}

	result := strings.Join(out, "\n")
	return lipgloss.NewStyle().Width(width).Render(result)
}

// renderLine applies inline formatting to a single line.
func (lm *LightweightMarkdown) renderLine(line string, width int) string {
	// Headings: # Title, ## Title, ### Title
	if strings.HasPrefix(line, "# ") {
		return lm.boldStyle.Render(strings.TrimPrefix(line, "# "))
	}
	if strings.HasPrefix(line, "## ") {
		return lm.boldStyle.Render(strings.TrimPrefix(line, "## "))
	}
	if strings.HasPrefix(line, "### ") {
		return lm.boldStyle.Render(strings.TrimPrefix(line, "### "))
	}

	// Code blocks: ``` ... ```
	if strings.HasPrefix(line, "```") {
		return lm.codeStyle.Render(line)
	}

	// Apply inline formatting
	result := line
	result = lm.applyInlineCode(result)
	result = lm.applyBold(result)
	result = lm.applyItalic(result)
	result = lm.applyLinks(result)

	return result
}

// applyInlineCode replaces `code` with styled inline code.
func (lm *LightweightMarkdown) applyInlineCode(text string) string {
	return replaceBetween(text, "`", "`", lm.codeStyle)
}

// applyBold replaces **text** with styled bold text.
func (lm *LightweightMarkdown) applyBold(text string) string {
	return replaceBetween(text, "**", "**", lm.boldStyle)
}

// applyItalic replaces *text* with styled italic text (but not ** which is bold).
func (lm *LightweightMarkdown) applyItalic(text string) string {
	// Simple approach: replace single * that aren't part of **
	// First, temporarily replace ** with a placeholder
	const boldPlaceholder = "\x00BOLD\x00"
	text = strings.ReplaceAll(text, "**", boldPlaceholder)
	text = replaceBetween(text, "*", "*", lm.italicStyle)
	text = strings.ReplaceAll(text, boldPlaceholder, "**")
	return text
}

// applyLinks replaces [text](url) with styled link text.
func (lm *LightweightMarkdown) applyLinks(text string) string {
	for {
		start := strings.Index(text, "[")
		if start < 0 {
			break
		}
		mid := strings.Index(text[start:], "](")
		if mid < 0 {
			break
		}
		mid += start
		end := strings.Index(text[mid:], ")")
		if end < 0 {
			break
		}
		end += mid

		linkText := text[start+1 : mid]
		// url := text[mid+2 : end] // unused for now, just style the text

		styled := lm.linkTextStyle.Render(linkText)
		text = text[:start] + styled + text[end+1:]
	}
	return text
}

// replaceBetween replaces text between open and close markers with styled text.
// Handles multiple occurrences in the same string.
func replaceBetween(text, open, close string, style lipgloss.Style) string {
	for {
		start := strings.Index(text, open)
		if start < 0 {
			break
		}
		end := strings.Index(text[start+len(open):], close)
		if end < 0 {
			break
		}
		end += start + len(open)

		inner := text[start+len(open) : end]
		styled := style.Render(inner)
		text = text[:start] + styled + text[end+len(close):]
	}
	return text
}
