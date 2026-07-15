package tui

import (
	"strings"

	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/tui/theme"
)

// RenderTransition composites the old and new frames with a visual effect.
func RenderTransition(prevFrame, nextFrame string, progress float64, tt TransitionType, width, height int, t theme.Theme) string {
	if progress >= 1.0 {
		return nextFrame
	}
	if progress <= 0.0 {
		return prevFrame
	}

	switch tt {
	case TransitionSlideLeft:
		return renderSlide(prevFrame, nextFrame, progress, width, height, true)
	case TransitionSlideRight:
		return renderSlide(prevFrame, nextFrame, progress, width, height, false)
	case TransitionFade:
		return renderFade(prevFrame, nextFrame, progress, width, height)
	default:
		return nextFrame
	}
}

// renderSlide performs a horizontal slide transition.
// If leftward is true, old content slides left and new enters from right.
func renderSlide(prev, next string, progress float64, width, height int, leftward bool) string {
	prevLines := padFrameLines(strings.Split(prev, "\n"), width, height)
	nextLines := padFrameLines(strings.Split(next, "\n"), width, height)

	offset := int(float64(width) * progress)
	if offset > width {
		offset = width
	}

	var result []string
	for y := 0; y < height; y++ {
		prevLine := ""
		if y < len(prevLines) {
			prevLine = prevLines[y]
		}
		nextLine := ""
		if y < len(nextLines) {
			nextLine = nextLines[y]
		}

		if leftward {
			// Old slides left: take tail of prev + head of next
			if offset >= width {
				result = append(result, nextLine)
			} else {
				tail := sliceVisibleTail(prevLine, width-offset)
				head := sliceVisibleHead(nextLine, offset)
				result = append(result, tail+head)
			}
		} else {
			// Old slides right: take tail of next + head of prev
			if offset >= width {
				result = append(result, nextLine)
			} else {
				head := sliceVisibleHead(prevLine, width-offset)
				tail := sliceVisibleTail(nextLine, offset)
				result = append(result, tail+head)
			}
		}
	}

	return strings.Join(result, "\n")
}

// renderFade performs a cross-fade using Unicode block density characters.
func renderFade(prev, next string, progress float64, width, height int) string {
	prevLines := padFrameLines(strings.Split(prev, "\n"), width, height)
	nextLines := padFrameLines(strings.Split(next, "\n"), width, height)

	// Density characters for blending
	densityChars := []rune{' ', '░', '▒', '▓', '█'}

	var result []string
	for y := 0; y < height; y++ {
		prevLine := prevLines[y]
		nextLine := nextLines[y]

		// At low progress, show more prev; at high progress, show more next
		if progress < 0.5 {
			// Blend prev with density characters
			density := int(progress * 2.0 * float64(len(densityChars)-1))
			if density >= len(densityChars) {
				density = len(densityChars) - 1
			}
			blendChar := densityChars[density]
			result = append(result, blendLine(prevLine, blendChar, width))
		} else {
			// Blend next emerging from density
			density := int((1.0 - progress) * 2.0 * float64(len(densityChars)-1))
			if density >= len(densityChars) {
				density = len(densityChars) - 1
			}
			blendChar := densityChars[density]
			result = append(result, blendLine(nextLine, blendChar, width))
		}
	}

	return strings.Join(result, "\n")
}

// blendLine replaces visible characters in a line with a blend character,
// preserving spacing structure.
func blendLine(line string, blendChar rune, width int) string {
	var b strings.Builder
	col := 0
	inEsc := false

	for _, r := range line {
		if inEsc {
			b.WriteRune(r)
			if r == 'm' {
				inEsc = false
			}
			continue
		}
		if r == '\x1b' {
			inEsc = true
			b.WriteRune(r)
			continue
		}
		if r == ' ' || r == '\t' {
			b.WriteRune(r)
		} else {
			b.WriteRune(blendChar)
		}
		col++
	}

	// Pad to width (only visible characters count toward width)
	for col < width {
		b.WriteRune(' ')
		col++
	}

	return b.String()
}

// padFrameLines ensures exactly `height` lines each of visible width `width`.
func padFrameLines(lines []string, width, height int) []string {
	result := make([]string, height)
	for i := 0; i < height; i++ {
		if i < len(lines) {
			line := lines[i]
			lw := lipgloss.Width(line)
			if lw < width {
				line += strings.Repeat(" ", width-lw)
			}
			result[i] = line
		} else {
			result[i] = strings.Repeat(" ", width)
		}
	}
	return result
}
