package tui

import "github.com/eshanized/M31A/internal/tui/components"

// TruncateWithEllipsis truncates a string to maxWidth visible columns,
// appending "..." if truncation occurred. Delegates to components.TruncateWithEllipsis.
func TruncateWithEllipsis(s string, maxWidth int) string {
	return components.TruncateWithEllipsis(s, maxWidth)
}

// TruncateMiddle truncates a string by showing the start and end with "..." in the
// middle. Delegates to components.TruncateMiddle.
func TruncateMiddle(s string, maxLen int) string {
	return components.TruncateMiddle(s, maxLen)
}

// TruncateEnd truncates a string by cutting at maxLen and appending "...".
// Delegates to components.TruncateEnd.
func TruncateEnd(s string, maxLen int) string {
	return components.TruncateEnd(s, maxLen)
}
