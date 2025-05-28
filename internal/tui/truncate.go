package tui

import (
	"strings"

	"github.com/charmbracelet/lipgloss"
	"github.com/mattn/go-runewidth"
)

// TruncateWithEllipsis truncates a string containing ANSI escape codes to a
// given display width, appending "...". It preserves ANSI codes up to the
// truncation point and closes any open SGR sequences.
func TruncateWithEllipsis(s string, maxWidth int) string {
	if lipgloss.Width(s) <= maxWidth {
		return s
	}

	ellips := "..."
	ellipsWidth := runewidth.StringWidth(ellips)
	maxVisible := maxWidth - ellipsWidth
	if maxVisible < 0 {
		maxVisible = 0
	}

	var result strings.Builder
	visible := 0
	inEscape := false

	for i := 0; i < len(s); i++ {
		b := s[i]
		if b == '\x1b' {
			inEscape = true
			result.WriteByte(b)
			continue
		}
		if inEscape {
			result.WriteByte(b)
			if b == 'm' {
				inEscape = false
			}
			continue
		}
		charWidth := runewidth.RuneWidth(rune(b))
		if visible+charWidth > maxVisible {
			break
		}
		visible += charWidth
		result.WriteByte(b)
	}

	result.WriteString("\x1b[0m")
	result.WriteString(ellips)
	return result.String()
}
