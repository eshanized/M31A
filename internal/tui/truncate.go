package tui

import "github.com/charmbracelet/lipgloss"

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
