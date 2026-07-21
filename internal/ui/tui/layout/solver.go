package layout

import (
	"strings"

	"github.com/charmbracelet/lipgloss"
)

// Solve recursively resolves the layout tree and returns the rendered string.
// The root Box's Width and Height define the available space. If 0, content
// sizes itself.
func Solve(root *Box) string {
	if root == nil {
		return ""
	}
	w := root.Width
	h := root.Height
	rendered := solveBox(root, w, h)
	return applyBoxStyle(rendered, root, w, h)
}

// solveBox recursively lays out children and returns the composed content string
// (without border/background/padding — those are applied by applyBoxStyle).
func solveBox(b *Box, availW, availH int) string {
	if len(b.Children) == 0 {
		if b.Content != nil {
			return b.Content.Render(availW, availH)
		}
		return ""
	}

	innerW := availW - b.Padding.Horizontal()
	innerH := availH - b.Padding.Vertical()
	if innerW < 0 {
		innerW = 0
	}
	if innerH < 0 {
		innerH = 0
	}

	if b.Direction == Row {
		return solveRow(b, innerW, innerH)
	}
	return solveColumn(b, innerW, innerH)
}

// solveRow lays out children horizontally.
func solveRow(b *Box, availW, availH int) string {
	children := b.Children
	if len(children) == 0 {
		return ""
	}

	// Pass 1: allocate fixed-width children, count flex children
	allocated := make([]int, len(children))
	totalFixed := 0
	totalGap := 0
	totalFlex := 0

	for i, child := range children {
		if i > 0 && b.Gap > 0 {
			totalGap += b.Gap
		}
		if child.Width > 0 {
			allocated[i] = child.Width
			totalFixed += child.Width
		} else if child.Flex > 0 {
			totalFlex += child.Flex
		}
	}

	// Pass 2: distribute remaining space to flex children
	remaining := availW - totalFixed - totalGap
	if remaining < 0 {
		remaining = 0
	}
	if totalFlex > 0 {
		flexTotal := 0
		for i, child := range children {
			if child.Flex > 0 && child.Width == 0 {
				allocated[i] = remaining * child.Flex / totalFlex
				flexTotal += allocated[i]
			}
		}
		// Distribute any remainder from integer truncation to the last flex child
		if remainder := remaining - flexTotal; remainder > 0 {
			for i := len(children) - 1; i >= 0; i-- {
				if children[i].Flex > 0 && children[i].Width == 0 {
					allocated[i] += remainder
					break
				}
			}
		}
	}

	// Pass 3: render each child and join horizontally
	var parts []string
	for i, child := range children {
		cw := allocated[i]
		if cw <= 0 && child.Width == 0 && child.Flex == 0 {
			continue
		}
		ch := availH
		if child.Height > 0 {
			ch = child.Height
		}

		rendered := solveBox(child, cw, ch)
		styled := applyBoxStyle(rendered, child, cw, ch)

		// Cross-axis alignment
		styled = alignVertical(styled, ch, b.Align)
		parts = append(parts, styled)
	}

	if len(parts) == 0 {
		return ""
	}

	var pos lipgloss.Position
	switch b.Align {
	case AlignCenter:
		pos = lipgloss.Center
	case AlignEnd:
		pos = lipgloss.Bottom
	default:
		pos = lipgloss.Top
	}

	result := lipgloss.JoinHorizontal(pos, parts...)

	// Insert gaps
	if b.Gap > 0 && len(parts) > 1 {
		gapStr := strings.Repeat(" ", b.Gap)
		var gapParts []string
		for i, p := range parts {
			gapParts = append(gapParts, p)
			if i < len(parts)-1 {
				gapParts = append(gapParts, gapStr)
			}
		}
		result = lipgloss.JoinHorizontal(pos, gapParts...)
	}

	return result
}

// solveColumn lays out children vertically.
func solveColumn(b *Box, availW, availH int) string {
	children := b.Children
	if len(children) == 0 {
		return ""
	}

	// Pass 1: allocate fixed-height children, count flex children
	allocated := make([]int, len(children))
	totalFixed := 0
	totalGap := 0
	totalFlex := 0

	for i, child := range children {
		if i > 0 && b.Gap > 0 {
			totalGap += b.Gap
		}
		if child.Height > 0 {
			allocated[i] = child.Height
			totalFixed += child.Height
		} else if child.Flex > 0 {
			totalFlex += child.Flex
		}
	}

	// Pass 2: distribute remaining space to flex children
	remaining := availH - totalFixed - totalGap
	if remaining < 0 {
		remaining = 0
	}
	if totalFlex > 0 {
		flexTotal := 0
		for i, child := range children {
			if child.Flex > 0 && child.Height == 0 {
				allocated[i] = remaining * child.Flex / totalFlex
				flexTotal += allocated[i]
			}
		}
		// Distribute any remainder from integer truncation to the last flex child
		if remainder := remaining - flexTotal; remainder > 0 {
			for i := len(children) - 1; i >= 0; i-- {
				if children[i].Flex > 0 && children[i].Height == 0 {
					allocated[i] += remainder
					break
				}
			}
		}
	}

	// Pass 3: render each child and join vertically
	var parts []string
	for i, child := range children {
		cw := availW
		if child.Width > 0 {
			cw = child.Width
		}
		ch := allocated[i]
		if ch <= 0 && child.Height == 0 && child.Flex == 0 {
			continue
		}

		rendered := solveBox(child, cw, ch)
		styled := applyBoxStyle(rendered, child, cw, ch)

		// Cross-axis alignment
		styled = alignHorizontal(styled, cw, b.Align)
		parts = append(parts, styled)
	}

	if len(parts) == 0 {
		return ""
	}

	var pos lipgloss.Position
	switch b.Align {
	case AlignCenter:
		pos = lipgloss.Center
	case AlignEnd:
		pos = lipgloss.Right
	default:
		pos = lipgloss.Left
	}

	result := lipgloss.JoinVertical(pos, parts...)

	// Insert gaps
	if b.Gap > 0 && len(parts) > 1 {
		gapStr := strings.Repeat("\n", b.Gap)
		var gapParts []string
		for i, p := range parts {
			gapParts = append(gapParts, p)
			if i < len(parts)-1 {
				gapParts = append(gapParts, gapStr)
			}
		}
		result = lipgloss.JoinVertical(pos, gapParts...)
	}

	return result
}

// applyBoxStyle wraps content with padding, border, and background.
func applyBoxStyle(content string, b *Box, w, h int) string {
	if content == "" && b.Content == nil && len(b.Children) == 0 {
		return ""
	}

	style := lipgloss.NewStyle()

	if b.Padding.Top > 0 || b.Padding.Right > 0 || b.Padding.Bottom > 0 || b.Padding.Left > 0 {
		style = style.Padding(b.Padding.Top, b.Padding.Right, b.Padding.Bottom, b.Padding.Left)
	}

	hasBorder := b.Border != (lipgloss.Border{})
	if hasBorder {
		style = style.Border(b.Border)
		if b.BorderColor != "" {
			style = style.BorderForeground(b.BorderColor)
		}
	}

	if b.Background != "" {
		style = style.Background(b.Background)
	}

	if w > 0 {
		style = style.Width(w)
	}

	return style.Render(content)
}

// alignVertical pads or positions content vertically within targetH.
func alignVertical(content string, targetH int, align Align) string {
	lines := strings.Split(content, "\n")
	h := len(lines)
	if h >= targetH {
		return content
	}
	pad := targetH - h
	switch align {
	case AlignCenter:
		top := pad / 2
		bottom := pad - top
		return strings.Repeat("\n", top) + content + strings.Repeat("\n", bottom)
	case AlignEnd:
		return strings.Repeat("\n", pad) + content
	default:
		return content
	}
}

// alignHorizontal pads or positions content horizontally within targetW.
func alignHorizontal(content string, targetW int, align Align) string {
	lines := strings.Split(content, "\n")
	var result []string
	for _, line := range lines {
		lineW := lipgloss.Width(line)
		if lineW >= targetW {
			result = append(result, line)
			continue
		}
		pad := targetW - lineW
		switch align {
		case AlignCenter:
			left := pad / 2
			right := pad - left
			result = append(result, strings.Repeat(" ", left)+line+strings.Repeat(" ", right))
		case AlignEnd:
			result = append(result, strings.Repeat(" ", pad)+line)
		default:
			result = append(result, line+strings.Repeat(" ", pad))
		}
	}
	return strings.Join(result, "\n")
}
