package layout

import (
	"strings"

	"github.com/charmbracelet/lipgloss"
)

// RenderBox is a convenience function that creates a Box, solves it, and
// returns the rendered string. It handles the common case of a single
// bordered, padded container.
func RenderBox(content string, w, h int, opts ...BoxOption) string {
	b := &Box{
		Width:  w,
		Height: h,
		Content: RenderFunc(func(width, height int) string {
			return content
		}),
	}
	for _, opt := range opts {
		opt(b)
	}
	return Solve(b)
}

// BoxOption configures a Box for RenderBox.
type BoxOption func(*Box)

// WithBoxBorder sets a border on the Box.
func WithBoxBorder(border lipgloss.Border, color lipgloss.Color) BoxOption {
	return func(b *Box) {
		b.Border = border
		b.BorderColor = color
	}
}

// WithBoxPadding sets uniform padding.
func WithBoxPadding(p int) BoxOption {
	return func(b *Box) {
		b.Padding = UniformInsets(p)
	}
}

// WithBoxBackground sets the background color.
func WithBoxBackground(bg lipgloss.Color) BoxOption {
	return func(b *Box) {
		b.Background = bg
	}
}

// RenderCard renders a card-style box with title, content, border, and padding.
func RenderCard(title, content string, w int, border lipgloss.Border, borderColor, bg lipgloss.Color) string {
	var body string
	if title != "" {
		headerLine := lipgloss.NewStyle().
			Foreground(borderColor).
			Bold(true).
			Render(title)
		body = headerLine + "\n" + content
	} else {
		body = content
	}

	return RenderBox(body, w, 0,
		WithBoxBorder(border, borderColor),
		WithBoxPadding(1),
		WithBoxBackground(bg),
	)
}

// SplitHorizontal splits available width among N children with optional gaps.
// Returns a slice of widths. Fixed widths are honored first, then remaining
// space is divided equally.
func SplitHorizontal(totalW int, count int, gap int, fixed ...int) []int {
	if count <= 0 {
		return nil
	}
	widths := make([]int, count)
	totalGap := 0
	if count > 1 {
		totalGap = gap * (count - 1)
	}
	totalFixed := 0
	flexCount := 0
	for i := 0; i < count; i++ {
		if i < len(fixed) && fixed[i] > 0 {
			widths[i] = fixed[i]
			totalFixed += fixed[i]
		} else {
			flexCount++
		}
	}
	remaining := totalW - totalFixed - totalGap
	if remaining < 0 {
		remaining = 0
	}
	if flexCount > 0 {
		each := remaining / flexCount
		flexTotal := 0
		for i := 0; i < count; i++ {
			if i >= len(fixed) || fixed[i] <= 0 {
				widths[i] = each
				flexTotal += each
			}
		}
		// Distribute remainder from integer truncation to the last flex slot
		if remainder := remaining - flexTotal; remainder > 0 {
			for i := count - 1; i >= 0; i-- {
				if i >= len(fixed) || fixed[i] <= 0 {
					widths[i] += remainder
					break
				}
			}
		}
	}
	return widths
}

// SplitVertical splits available height among N children with optional gaps.
func SplitVertical(totalH int, count int, gap int, fixed ...int) []int {
	if count <= 0 {
		return nil
	}
	heights := make([]int, count)
	totalGap := 0
	if count > 1 {
		totalGap = gap * (count - 1)
	}
	totalFixed := 0
	flexCount := 0
	for i := 0; i < count; i++ {
		if i < len(fixed) && fixed[i] > 0 {
			heights[i] = fixed[i]
			totalFixed += fixed[i]
		} else {
			flexCount++
		}
	}
	remaining := totalH - totalFixed - totalGap
	if remaining < 0 {
		remaining = 0
	}
	if flexCount > 0 {
		each := remaining / flexCount
		flexTotal := 0
		for i := 0; i < count; i++ {
			if i >= len(fixed) || fixed[i] <= 0 {
				heights[i] = each
				flexTotal += each
			}
		}
		// Distribute remainder from integer truncation to the last flex slot
		if remainder := remaining - flexTotal; remainder > 0 {
			for i := count - 1; i >= 0; i-- {
				if i >= len(fixed) || fixed[i] <= 0 {
					heights[i] += remainder
					break
				}
			}
		}
	}
	return heights
}

// FitContent ensures content fits within exactly w columns and h rows,
// padding with spaces or clipping as needed.
func FitContent(content string, w, h int) string {
	lines := strings.Split(content, "\n")

	if len(lines) > h {
		lines = lines[:h]
	}
	for len(lines) < h {
		lines = append(lines, "")
	}

	var result []string
	for _, line := range lines {
		lineW := lipgloss.Width(line)
		if lineW > w {
			line = truncateToWidth(line, w)
			lineW = lipgloss.Width(line)
		}
		if lineW < w {
			line += strings.Repeat(" ", w-lineW)
		}
		result = append(result, line)
	}
	return strings.Join(result, "\n")
}
