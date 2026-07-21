package repl

import (
	"strings"

	"github.com/charmbracelet/bubbles/viewport"
	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/ui/tui/theme"
	"github.com/eshanized/M31A/internal/ui/tui/tuitypes"
)

// overlayScrollbar overlays a 1-column scrollbar track on the right edge of
// the rendered viewport content. The thumb indicates the currently-visible
// window within the total scrollable content.
//
// Characters used:
//
//	·  track (empty portion of the scroll track)
//	█  thumb (visible window)
//
// The scrollbar is only rendered when the content exceeds the viewport height.
// When content fits entirely, the original content is returned unchanged.
func overlayScrollbar(rendered string, vp viewport.Model, s theme.SemanticStyles, width int) string {
	if width < 20 || vp.Height < 2 {
		return rendered
	}

	totalLines := vp.TotalLineCount()
	if totalLines <= vp.Height {
		return rendered
	}

	lines := strings.Split(rendered, "\n")

	// Thumb size: proportion of viewport to total, minimum 1 row.
	thumbSize := vp.Height * vp.Height / totalLines
	if thumbSize < 1 {
		thumbSize = 1
	}
	if thumbSize > vp.Height {
		thumbSize = vp.Height
	}

	// Thumb top row: map YOffset into [0, vp.Height-thumbSize].
	maxScroll := totalLines - vp.Height
	if maxScroll <= 0 {
		maxScroll = 1
	}
	thumbTop := vp.YOffset * (vp.Height - thumbSize) / maxScroll
	if thumbTop < 0 {
		thumbTop = 0
	}
	if thumbTop > vp.Height-thumbSize {
		thumbTop = vp.Height - thumbSize
	}
	thumbBottom := thumbTop + thumbSize

	trackChar := s.ScrollbarTrack.Render("·")
	thumbChar := s.Scrollbar.Render("█")

	// Pad or clip lines to exactly vp.Height rows.
	for len(lines) < vp.Height {
		lines = append(lines, "")
	}
	if len(lines) > vp.Height {
		lines = lines[:vp.Height]
	}

	scrollCol := width - 1

	for i := 0; i < vp.Height; i++ {
		line := lines[i]
		lineW := lipgloss.Width(line)

		var ch string
		if i >= thumbTop && i < thumbBottom {
			ch = thumbChar
		} else {
			ch = trackChar
		}

		switch {
		case lineW <= scrollCol:
			line = line + strings.Repeat(" ", scrollCol-lineW) + ch
		case lineW == scrollCol+1:
			line = truncateStyledToWidth(line, scrollCol) + ch
		default:
			line = truncateStyledToWidth(line, scrollCol-1) + " " + ch
		}

		lines[i] = line
	}

	return strings.Join(lines, "\n")
}

// truncateStyledToWidth truncates a styled (ANSI-escaped) string to at most
// maxW visible columns. Uses tuitypes.TruncateWithEllipsis to handle escape sequences.
func truncateStyledToWidth(s string, maxW int) string {
	w := lipgloss.Width(s)
	if w <= maxW {
		return s
	}
	return tuitypes.TruncateWithEllipsis(s, maxW)
}
