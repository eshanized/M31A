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

// truncateMiddle truncates a string by showing the start and end with "..."
// in the middle. Useful for file paths and model names.
func truncateMiddle(s string, maxLen int) string {
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
