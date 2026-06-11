package components

import (
	"unicode/utf8"

	"github.com/charmbracelet/lipgloss"
)

// TruncateWithEllipsis truncates a string to maxWidth visible columns,
// appending "..." if truncation occurred. Respects ANSI escape sequences
// via lipgloss.Width so styled strings are measured correctly.
func TruncateWithEllipsis(s string, maxWidth int) string {
	if maxWidth <= 0 {
		return ""
	}
	w := lipgloss.Width(s)
	if w <= maxWidth {
		return s
	}
	// Binary search for the cut point
	runes := []rune(s)
	lo, hi := 0, len(runes)
	for lo < hi {
		mid := (lo + hi + 1) / 2
		if lipgloss.Width(string(runes[:mid])) <= maxWidth-3 {
			lo = mid
		} else {
			hi = mid - 1
		}
	}
	if lo == 0 {
		return "..."
	}
	return string(runes[:lo]) + "..."
}

// TruncateMiddle truncates a string by showing the start and end with "..." in the
// middle. Useful for file paths (src/.../file.go) and model names (claude...slt-20241022).
func TruncateMiddle(s string, maxLen int) string {
	if maxLen <= 0 {
		return ""
	}
	if utf8.RuneCountInString(s) <= maxLen {
		return s
	}
	if maxLen < 5 {
		return string([]rune(s)[:maxLen])
	}
	half := (maxLen - 3) / 2
	runes := []rune(s)
	return string(runes[:half]) + "..." + string(runes[len(runes)-half:])
}

// TruncateEnd truncates a string by cutting at maxLen and appending "...".
// Useful for command output and single-line truncation.
func TruncateEnd(s string, maxLen int) string {
	if maxLen <= 0 {
		return ""
	}
	if utf8.RuneCountInString(s) <= maxLen {
		return s
	}
	return string([]rune(s)[:maxLen-1]) + "\u2026"
}

// TruncateError truncates an error message to show the first 200 runes plus "[...]".
func TruncateError(s string) string {
	if utf8.RuneCountInString(s) <= 200 {
		return s
	}
	runes := []rune(s)
	return string(runes[:200]) + "[...]"
}
