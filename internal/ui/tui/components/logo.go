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
// If customLogoText is non-empty, it is used instead of the embedded default.
func RenderLogo(version string, bold bool, brandColor lipgloss.Color, customLogoText ...string) string {
	logoStr := logo
	if len(customLogoText) > 0 && customLogoText[0] != "" {
		logoStr = customLogoText[0]
	}
	if version != "" {
		logoStr = logoStr + version
	}
	style := lipgloss.NewStyle().Foreground(brandColor)
	if bold {
		style = style.Bold(true)
	}
	lines := strings.Split(logoStr, "\n")
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
// maxW limits the total output width; pass 0 for no limit.
// Optional customLogoText overrides the embedded bigLogo.
func RenderBigLogo(brandColor lipgloss.Color, glow bool, maxW int, customLogoText ...string) string {
	logoStr := bigLogo
	if len(customLogoText) > 0 && customLogoText[0] != "" {
		logoStr = customLogoText[0]
	}
	lines := strings.Split(logoStr, "\n")
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

	return logoBlock + "\n" + tagline + "\n" + RenderLogoGlow(brandColor, maxW)
}

// RenderLogoGlow renders a gradient glow line centered beneath the big logo.
// Characters fade from solid (█) at the center to transparent (·) at edges.
// The width parameter limits the glow to fit the available space.
func RenderLogoGlow(brandColor lipgloss.Color, maxWidth int) string {
	// Match the width of the big logo block (52 visible chars), capped by maxWidth
	glowW := 54
	if maxWidth > 0 && glowW > maxWidth {
		glowW = maxWidth
	}
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
