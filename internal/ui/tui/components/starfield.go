package components

import (
	"strings"

	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/ui/tui/theme"
)

// RenderStarfield renders a grid of width x height characters with scattered
// Unicode middle dot characters (·) for the FirstRun galaxy metaphor.
// Uses theme.TextMuted color. Approximately 3-5% of cells have a dot.
// Returns a multi-line string suitable for lipgloss.JoinVertical.
// The seed parameter ensures deterministic output (same seed = same output).
func RenderStarfield(width, height int, seed int64, t theme.Theme) string {
	return renderStarfield(width, height, seed, true, t)
}

// RenderStarfieldPlain renders the same deterministic dot grid as
// RenderStarfield but without ANSI styling. Every returned line is exactly
// width runes long, which is required when callers splice styled overlays
// into the grid (ANSI escape codes would otherwise break byte-level splits).
func RenderStarfieldPlain(width, height int, seed int64) string {
	return renderStarfield(width, height, seed, false, theme.Theme{})
}

func renderStarfield(width, height int, seed int64, styled bool, t theme.Theme) string {
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

	dotStyle := lipgloss.NewStyle()
	if styled {
		dotStyle = dotStyle.Foreground(t.TextMuted)
	}
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

// RenderStarfieldWithWindow renders a deterministic starfield grid with a
// blank rectangular window cut out of the center. The window is sized to
// exactly fit an overlay panel of windowW x windowH cells, so callers can
// safely JoinHorizontal/JoinVertical the panel into the window without any
// post-hoc byte-level splicing. Every returned line is exactly width runes
// long; lines are plain (no ANSI) so they concatenate predictably with any
// styled panel content.
//
// Returns a slice of height lines. Each line is structured as:
//
//	leftMargin (plain with dots) + windowRow + rightMargin (plain with dots)
//
// Rows inside the window have plain spaces in the window columns; rows
// outside the window are a full starfield row.
func RenderStarfieldWithWindow(width, height, windowW, windowH int, seed int64) []string {
	if width <= 0 || height <= 0 {
		return nil
	}
	if width > 200 {
		width = 200
	}
	if height > 100 {
		height = 100
	}

	if windowW > width {
		windowW = width
	}
	if windowH > height {
		windowH = height
	}
	if windowW < 0 {
		windowW = 0
	}
	if windowH < 0 {
		windowH = 0
	}

	leftW := (width - windowW) / 2
	if leftW < 0 {
		leftW = 0
	}
	rightW := width - windowW - leftW
	topH := (height - windowH) / 2
	if topH < 0 {
		topH = 0
	}

	lines := make([]string, height)
	windowSpaces := strings.Repeat(" ", windowW)
	for y := 0; y < height; y++ {
		inWindowY := y >= topH && y < topH+windowH
		var b strings.Builder
		// Left margin
		for x := 0; x < leftW; x++ {
			h := hashPosition(seed, int64(x), int64(y))
			if h%100 < 4 {
				b.WriteRune('·')
			} else {
				b.WriteRune(' ')
			}
		}
		// Middle (window or starfield)
		if inWindowY {
			b.WriteString(windowSpaces)
		} else {
			for x := leftW; x < leftW+windowW; x++ {
				h := hashPosition(seed, int64(x), int64(y))
				if h%100 < 4 {
					b.WriteRune('·')
				} else {
					b.WriteRune(' ')
				}
			}
		}
		// Right margin
		for x := leftW + windowW; x < leftW+windowW+rightW; x++ {
			h := hashPosition(seed, int64(x), int64(y))
			if h%100 < 4 {
				b.WriteRune('·')
			} else {
				b.WriteRune(' ')
			}
		}
		lines[y] = b.String()
	}
	return lines
}

// hashPosition produces a deterministic hash from seed and coordinates.
func hashPosition(seed, x, y int64) int64 {
	h := seed
	h = (h*31 + x) & 0x7FFFFFFFFFFFFFFF
	h = (h*31 + y) & 0x7FFFFFFFFFFFFFFF
	return h
}
