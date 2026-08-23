package artifacts

import (
	"strings"
)

func slugify(s string) string {
	s = strings.ToLower(s)
	s = strings.ReplaceAll(s, " ", "-")
	// Remove non-alphanumeric except hyphens
	var b strings.Builder
	for _, r := range s {
		if (r >= 'a' && r <= 'z') || (r >= '0' && r <= '9') || r == '-' {
			b.WriteRune(r)
		}
	}
	// Remove consecutive hyphens
	result := strings.ReplaceAll(b.String(), "--", "-")
	return strings.Trim(result, "-")
}