package components

import (
	"fmt"
	"strings"

	"github.com/charmbracelet/lipgloss"
)

// logo is the ASCII art for M31A (compact version for header/sidebar).
const logo = `  __  _______  __
 /  |/  / __ \/ _/
 / /|_/ / /_/ / _/
 /_/  /_/\____/_/ `

// bigLogo is the user-specified large ASCII art logo for the welcome/first-run screens.
const bigLogo = `___  ___ _____  __    ___  
|  \/  ||____ |/  |  / _ \ 
| .  . |    / /` + "`" + `| | / /_\ \
| |\/| |    \ \ | | |  _  |
| |  | |.___/ /_| |_| | | |
\_|  |_/\____/ \___/\_| |_/`

// bigLogoTagline is displayed beneath the big logo in muted text.
const bigLogoTagline = "autonomous coding agent"

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

// RenderBigLogo renders the large ASCII art logo, optionally with a
// multi-row gradient glow effect beneath it. When glow is true the glow
// fades from full brand color at the center to transparent at the edges
// using fade characters (█ ▓ ▒ ░ ·). The tagline "autonomous coding agent"
// is always rendered below in muted secondary color.
func RenderBigLogo(brandColor lipgloss.Color, glow bool) string {
	lines := strings.Split(bigLogo, "\n")
	styled := make([]string, len(lines))
	for i, line := range lines {
		styled[i] = lipgloss.NewStyle().Foreground(brandColor).Bold(true).Render(line)
	}
	logoBlock := lipgloss.JoinVertical(lipgloss.Top, styled...)

	// Tagline centered below logo
	taglineW := len(bigLogoTagline)
	logoLineW := 0
	for _, l := range lines {
		if len(l) > logoLineW {
			logoLineW = len(l)
		}
	}
	pad := (logoLineW - taglineW) / 2
	if pad < 0 {
		pad = 0
	}
	tagline := lipgloss.NewStyle().
		Foreground(lipgloss.Color("#6B7280")).
		Italic(true).
		Render(strings.Repeat(" ", pad) + bigLogoTagline)

	if !glow {
		return logoBlock + "\n" + tagline
	}

	return logoBlock + "\n" + tagline + "\n" + RenderLogoGlow(brandColor)
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
