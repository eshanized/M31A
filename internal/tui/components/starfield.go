package components

import (
	"strings"

	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/tui/theme"
)

// RenderStarfield renders a grid of width x height characters with scattered
// Unicode middle dot characters (·) for the FirstRun galaxy metaphor.
// Uses theme.TextMuted color. Approximately 3-5% of cells have a dot.
// Returns a multi-line string suitable for lipgloss.JoinVertical.
// The seed parameter ensures deterministic output (same seed = same output).
func RenderStarfield(width, height int, seed int64, t theme.Theme) string {
	if width <= 0 || height <= 0 {
		return ""
	}

	// Cap dimensions for performance (threat T-24-02)
	if width > 200 {
		width = 200
	}
	if height > 100 {
		height = 100
	}

	dotStyle := lipgloss.NewStyle().Foreground(t.TextMuted)
	lines := make([]string, height)

	for y := 0; y < height; y++ {
		var b strings.Builder
		for x := 0; x < width; x++ {
			// Simple deterministic hash from seed + position
			h := hashPosition(seed, int64(x), int64(y))
			// ~4% of cells get a dot
			if h%100 < 4 {
				b.WriteString(dotStyle.Render("·"))
			} else {
				b.WriteRune(' ')
			}
		}
		lines[y] = b.String()
	}

	return strings.Join(lines, "\n")
}

// hashPosition produces a deterministic hash from seed and coordinates.
func hashPosition(seed, x, y int64) int64 {
	h := seed
	h = (h*31 + x) & 0x7FFFFFFFFFFFFFFF
	h = (h*31 + y) & 0x7FFFFFFFFFFFFFFF
	return h
}
