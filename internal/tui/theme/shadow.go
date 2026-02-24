package theme

import (
	"strings"

	"github.com/charmbracelet/lipgloss"
)

// RenderWithShadow adds a shadow effect to content
func RenderWithShadow(content string, shadowColor lipgloss.Color, offsetRight, offsetDown int) string {
	lines := strings.Split(content, "\n")
	maxLen := 0
	for _, line := range lines {
		if len(line) > maxLen {
			maxLen = len(line)
		}
	}

	shadowStyle := lipgloss.NewStyle().Foreground(shadowColor)
	var result []string

	// Add content lines
	result = append(result, lines...)

	// Add shadow lines
	for i := 0; i < offsetDown; i++ {
		shadowLine := strings.Repeat(" ", offsetRight) + strings.Repeat("▄", maxLen)
		result = append(result, shadowStyle.Render(shadowLine))
	}

	return strings.Join(result, "\n")
}
