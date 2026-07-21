package layout

import (
	"strings"

	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/ui/tui/theme"
)

// StackLayer represents one layer in a Z-order composition.
type StackLayer struct {
	Content string
	X       int // horizontal offset from left
	Y       int // vertical offset from top
	Width   int // 0 = auto from content
	Height  int // 0 = auto from content
}

// RenderStack composites multiple layers at different Z-depths.
// Later layers render on top of earlier ones.
// The base layer is filled with spaces.
func RenderStack(width, height int, layers []StackLayer) string {
	// Build a 2D grid of cells
	grid := make([][]rune, height)
	for y := range grid {
		grid[y] = make([]rune, width)
		for x := range grid[y] {
			grid[y][x] = ' '
		}
	}

	// Stamp each layer onto the grid
	for _, layer := range layers {
		lines := strings.Split(layer.Content, "\n")
		for dy, line := range lines {
			absY := layer.Y + dy
			if absY < 0 || absY >= height {
				continue
			}
			col := 0
			for _, r := range line {
				absX := layer.X + col
				if absX >= 0 && absX < width {
					grid[absY][absX] = r
				}
				col++
			}
		}
	}

	var result []string
	for _, row := range grid {
		result = append(result, string(row))
	}
	return strings.Join(result, "\n")
}

// RenderDimmed applies a dimming effect to content by prepending the ANSI
// faint attribute to each line, preserving existing styling.
func RenderDimmed(content string, t theme.Theme) string {
	lines := strings.Split(content, "\n")
	// ANSI faint (dim) attribute: \x1b[2m ... \x1b[22m (normal intensity)
	var result []string
	for _, line := range lines {
		result = append(result, "\x1b[2m"+line+"\x1b[22m")
	}
	return strings.Join(result, "\n")
}

// RenderDimmedOverlay composites a dimmed base with a centered overlay.
func RenderDimmedOverlay(base string, overlay string, width, height int, t theme.Theme) string {
	dimmed := RenderDimmed(base, t)
	return RenderOverlay(dimmed, overlay, width, height, lipgloss.Width(overlay), t)
}

// RenderModalOverlay renders a modal centered on a dimmed background.
// Uses string-based line composition (left + modal + right segments joined by
// concatenation) to avoid corrupting ANSI escape codes that rune-by-rune
// stamping would break.
func RenderModalOverlay(base string, modal string, width, height int, t theme.Theme) string {
	modalLines := strings.Split(modal, "\n")
	modalH := len(modalLines)
	modalW := 0
	for _, line := range modalLines {
		lw := lipgloss.Width(line)
		if lw > modalW {
			modalW = lw
		}
	}

	x := (width - modalW) / 2
	if x < 0 {
		x = 0
	}
	y := (height - modalH) / 2
	if y < 0 {
		y = 0
	}

	baseLines := strings.Split(base, "\n")
	for len(baseLines) < height {
		baseLines = append(baseLines, "")
	}
	if len(baseLines) > height {
		baseLines = baseLines[:height]
	}

	faintOn := "\x1b[2m"
	faintOff := "\x1b[22m"

	var result []string
	for i := 0; i < height; i++ {
		baseLine := baseLines[i]
		baseVW := lipgloss.Width(baseLine)

		inModal := i >= y && i < y+modalH
		if !inModal {
			// Non-modal row: dim the full base line
			if baseVW >= width {
				result = append(result, faintOn+baseLine+faintOff)
			} else {
				result = append(result, faintOn+baseLine+strings.Repeat(" ", width-baseVW)+faintOff)
			}
			continue
		}

		// Modal row: dim left/right padding, modal at normal brightness
		modalLine := modalLines[i-y]
		mVW := lipgloss.Width(modalLine)
		rightStart := x + mVW

		var left string
		if x > 0 {
			if baseVW >= x {
				left = truncateToWidth(baseLine, x)
			} else {
				left = baseLine + strings.Repeat(" ", x-baseVW)
			}
		}

		var right string
		if rightStart < width {
			rightW := width - rightStart
			if baseVW > rightStart {
				rightContent := baseLine
				if baseVW > width {
					rightContent = truncateToWidth(baseLine, width)
				}
				right = rightContent
				rightVW := lipgloss.Width(right)
				if rightVW > rightW {
					right = truncateToWidth(right, rightW)
				} else if rightVW < rightW {
					right += strings.Repeat(" ", rightW-rightVW)
				}
			} else {
				right = strings.Repeat(" ", rightW)
			}
		}

		// Pad modal line to exact visual width if needed
		if mVW < modalW {
			modalLine += strings.Repeat(" ", modalW-mVW)
		}

		result = append(result, faintOn+left+faintOff+modalLine+faintOn+right+faintOff)
	}

	return strings.Join(result, "\n")
}
