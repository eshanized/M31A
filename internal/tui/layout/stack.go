package layout

import (
	"strings"

	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/tui/theme"
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
func RenderModalOverlay(base string, modal string, width, height int, t theme.Theme) string {
	dimmed := RenderDimmed(base, t)
	modalLines := strings.Split(modal, "\n")
	modalH := len(modalLines)
	modalW := 0
	for _, line := range modalLines {
		lw := lipgloss.Width(line)
		if lw > modalW {
			modalW = lw
		}
	}

	// Center the modal
	x := (width - modalW) / 2
	y := (height - modalH) / 2
	if x < 0 {
		x = 0
	}
	if y < 0 {
		y = 0
	}

	dimmedLines := strings.Split(dimmed, "\n")
	// Pad dimmed to exact dimensions
	for len(dimmedLines) < height {
		dimmedLines = append(dimmedLines, strings.Repeat(" ", width))
	}
	if len(dimmedLines) > height {
		dimmedLines = dimmedLines[:height]
	}

	for dy, modalLine := range modalLines {
		absY := y + dy
		if absY < 0 || absY >= height {
			continue
		}
		baseLine := dimmedLines[absY]
		baseRunes := []rune(baseLine)

		// Ensure base line is wide enough
		for len(baseRunes) < x+modalW {
			baseRunes = append(baseRunes, ' ')
		}

		// Stamp modal line
		col := 0
		for _, r := range modalLine {
			absX := x + col
			if absX >= 0 && absX < len(baseRunes) {
				baseRunes[absX] = r
			}
			col++
		}

		// Rebuild line, preserving ANSI in non-modal areas
		dimmedLines[absY] = string(baseRunes)
	}

	return strings.Join(dimmedLines, "\n")
}

