package components

import (
	"fmt"
	"strings"

	"github.com/charmbracelet/lipgloss"
)

// logo is the ASCII art for M31A.
const logo = `  __  _______  __
 /  |/  / __ \/ _/
 / /|_/ / /_/ / _/
 /_/  /_/\____/_/ `

// RenderLogo renders the M31A logo with optional version and bold styling.
func RenderLogo(version string, bold bool, brandColor lipgloss.Color) string {
	logoText := logo
	if version != "" {
		logoText = logo + version
	}
	style := lipgloss.NewStyle().Foreground(brandColor)
	if bold {
		style = style.Bold(true)
	}
	lines := strings.Split(logoText, "\n")
	styled := make([]string, len(lines))
	for i, line := range lines {
		styled[i] = style.Render(line)
	}
	return lipgloss.JoinVertical(lipgloss.Top, styled...)
}

// RenderSectionHeader renders a section header with a title and horizontal rule.
func RenderSectionHeader(title string, width int) string {
	prefix := fmt.Sprintf("── %s ", title)
	remaining := width - lipgloss.Width(prefix)
	if remaining < 0 {
		remaining = 0
	}
	return prefix + strings.Repeat("─", remaining)
}
