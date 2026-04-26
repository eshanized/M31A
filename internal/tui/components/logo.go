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

// bigLogo is a larger, bolder block-style logo for the welcome screen.
// Each line is the same width for clean glow alignment.
const bigLogo = `  ███╗   ███╗ █████╗ ██████╗  ██████╗ ██╗  ██╗
  ████╗ ████║██╔══██╗██╔══██╗██╔═══██╗██║  ██║
  ██╔████╔██║███████║██║  ██║██║   ██║███████║
  ██║╚██╔╝██║██╔══██║██║  ██║██║   ██║██╔══██║
  ██║ ╚═╝ ██║██║  ██║██████╔╝╚██████╔╝██║  ██║
  ╚═╝     ╚═╝╚═╝  ╚═╝╚═════╝  ╚═════╝ ╚═╝  ╚═╝`

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

// RenderBigLogo renders the large block-style M31A logo, optionally with a
// multi-row gradient glow effect beneath it. When glow is true the glow
// fades from full brand color at the center to transparent at the edges
// using fade characters (█ ▓ ▒ ░ ·).
func RenderBigLogo(brandColor lipgloss.Color, glow bool) string {
	lines := strings.Split(bigLogo, "\n")
	styled := make([]string, len(lines))
	for i, line := range lines {
		styled[i] = lipgloss.NewStyle().Foreground(brandColor).Bold(true).Render(line)
	}
	logoBlock := lipgloss.JoinVertical(lipgloss.Top, styled...)

	if !glow {
		return logoBlock
	}

	return logoBlock + "\n" + RenderLogoGlow(brandColor)
}

// RenderLogoGlow renders a gradient glow line centered beneath the big logo.
// Characters fade from solid (█) at the center to transparent (·) at edges.
func RenderLogoGlow(brandColor lipgloss.Color) string {
	// Match the width of the big logo block (52 visible chars)
	glowW := 54
	fadeChars := []rune{'█', '▓', '▒', '░', '·'}

	halfW := glowW / 2
	var b strings.Builder
	for i := 0; i < halfW; i++ {
		// Map position to fade index: center = 0 (solid), edges = last (faint)
		idx := i * len(fadeChars) / halfW
		if idx >= len(fadeChars) {
			idx = len(fadeChars) - 1
		}
		b.WriteString(lipgloss.NewStyle().Foreground(brandColor).Render(string(fadeChars[idx])))
	}
	left := b.String()

	// Mirror for right side
	b.Reset()
	for i := halfW - 1; i >= 0; i-- {
		idx := i * len(fadeChars) / halfW
		if idx >= len(fadeChars) {
			idx = len(fadeChars) - 1
		}
		b.WriteString(lipgloss.NewStyle().Foreground(brandColor).Render(string(fadeChars[idx])))
	}
	right := b.String()

	return left + right
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
