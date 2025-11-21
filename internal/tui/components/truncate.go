package components

import "unicode/utf8"

// truncateEnd truncates a string by cutting at maxLen and appending "…".
// Useful for command output and single-line truncation.
func truncateEnd(s string, maxLen int) string {
	if maxLen <= 0 {
		return ""
	}
	if utf8.RuneCountInString(s) <= maxLen {
		return s
	}
	return string([]rune(s)[:maxLen-1]) + "…"
}


