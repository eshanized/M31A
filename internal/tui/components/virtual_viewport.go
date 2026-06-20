package components

import (
	"strings"

	"github.com/charmbracelet/lipgloss"
)

// VirtualViewport renders only visible items in a scrollable list,
// avoiding full re-render of off-screen content.
type VirtualViewport struct {
	Items             []string
	ItemHeights       []int
	ScrollTop         int
	ViewportH         int
	ViewportW         int
	cachedTotalHeight int
	heightCacheValid  bool
}

// TotalHeight returns the sum of all item heights.
func (v *VirtualViewport) TotalHeight() int {
	if v.heightCacheValid {
		return v.cachedTotalHeight
	}
	total := 0
	for i, item := range v.Items {
		total += v.itemHeight(i, item)
	}
	v.cachedTotalHeight = total
	v.heightCacheValid = true
	return total
}

// SetScroll clamps and sets the scroll position.
func (v *VirtualViewport) SetScroll(top int) {
	totalH := v.TotalHeight()
	maxScroll := totalH - v.ViewportH
	if maxScroll < 0 {
		maxScroll = 0
	}
	if top < 0 {
		top = 0
	}
	if top > maxScroll {
		top = maxScroll
	}
	v.ScrollTop = top
}

// ScrollBy adjusts the scroll position by delta lines.
func (v *VirtualViewport) ScrollBy(delta int) {
	v.SetScroll(v.ScrollTop + delta)
}

// View renders only the visible portion of items by walking the item list
// and splitting only items that overlap the visible window.
func (v *VirtualViewport) View() string {
	if v.ViewportH <= 0 || len(v.Items) == 0 {
		return ""
	}

	// Clamp scroll
	v.SetScroll(v.ScrollTop)

	// Walk items to find the visible window without materializing all lines
	var result []string
	lineOffset := 0
	viewEnd := v.ScrollTop + v.ViewportH

	for i, item := range v.Items {
		h := v.itemHeight(i, item)
		itemEnd := lineOffset + h

		// Skip items entirely above the viewport
		if itemEnd <= v.ScrollTop {
			lineOffset = itemEnd
			continue
		}
		// Stop once we're past the viewport
		if lineOffset >= viewEnd {
			break
		}

		// This item overlaps the viewport — split it
		lines := strings.Split(item, "\n")
		for len(lines) < h {
			lines = append(lines, "")
		}

		// Determine which lines of this item are visible
		startLine := v.ScrollTop - lineOffset
		if startLine < 0 {
			startLine = 0
		}
		endLine := viewEnd - lineOffset
		if endLine > len(lines) {
			endLine = len(lines)
		}

		// If viewport starts before this item, pad with blank lines
		for pad := 0; pad < -startLine && len(result) < v.ViewportH; pad++ {
			result = append(result, "")
		}

		for j := startLine; j < endLine && len(result) < v.ViewportH; j++ {
			line := lines[j]
			lw := lipgloss.Width(line)
			if lw < v.ViewportW {
				line += strings.Repeat(" ", v.ViewportW-lw)
			}
			result = append(result, line)
		}

		lineOffset = itemEnd
	}

	// Pad to viewport height
	for len(result) < v.ViewportH {
		result = append(result, strings.Repeat(" ", v.ViewportW))
	}

	return strings.Join(result, "\n")
}

// ScrollRatio returns the scroll position as a ratio 0.0–1.0.
func (v *VirtualViewport) ScrollRatio() float64 {
	totalH := v.TotalHeight()
	maxScroll := totalH - v.ViewportH
	if maxScroll <= 0 {
		return 0
	}
	return float64(v.ScrollTop) / float64(maxScroll)
}

// ScrollToBottom scrolls to the last item.
func (v *VirtualViewport) ScrollToBottom() {
	v.SetScroll(v.TotalHeight())
}

// ScrollToTop scrolls to the first item.
func (v *VirtualViewport) ScrollToTop() {
	v.SetScroll(0)
}

func (v *VirtualViewport) itemHeight(idx int, content string) int {
	if idx < len(v.ItemHeights) && v.ItemHeights[idx] > 0 {
		return v.ItemHeights[idx]
	}
	h := strings.Count(content, "\n") + 1
	if content == "" {
		h = 1
	}
	for len(v.ItemHeights) <= idx {
		v.ItemHeights = append(v.ItemHeights, 0)
	}
	v.ItemHeights[idx] = h
	return h
}

// InvalidateHeightCache clears the cached heights, forcing recomputation.
func (v *VirtualViewport) InvalidateHeightCache() {
	v.ItemHeights = nil
	v.heightCacheValid = false
}
