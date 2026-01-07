package theme

import "strings"

// NormalizeTabs replaces tab characters with spaces
func NormalizeTabs(s string, width int) string {
	if width <= 0 {
		width = 4
	}
	return strings.ReplaceAll(s, "\t", strings.Repeat(" ", width))
}