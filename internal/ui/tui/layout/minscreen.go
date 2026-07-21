package layout

import (
	"strings"

	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/ui/tui/theme"
)

// RenderTooNarrow renders a centered "resize terminal" message when the
// terminal is below the minimum width threshold.
func RenderTooNarrow(width, height int, t theme.Theme) string {
	brand := lipgloss.NewStyle().Foreground(t.Brand).Bold(true).Render("M31A")
	msg := lipgloss.NewStyle().Foreground(t.TextMuted).Italic(true).
		Render("Terminal too narrow")
	hint := lipgloss.NewStyle().Foreground(t.TextMuted).
		Render("Resize to at least 40 columns")

	content := lipgloss.JoinVertical(lipgloss.Center,
		brand,
		"",
		msg,
		hint,
	)

	return lipgloss.Place(width, height, lipgloss.Center, lipgloss.Center, content)
}

// RenderOverlay renders a sidebar or modal overlay on top of existing content.
// The overlay occupies the right portion of the terminal with a dimmed backdrop.
func RenderOverlay(base string, overlay string, width, height, overlayWidth int, t theme.Theme) string {
	baseLines := strings.Split(base, "\n")

	// Pad base to exactly height rows and width columns
	for len(baseLines) < height {
		baseLines = append(baseLines, strings.Repeat(" ", width))
	}
	if len(baseLines) > height {
		baseLines = baseLines[:height]
	}

	overlayLines := strings.Split(overlay, "\n")

	// Position overlay on the right side
	leftWidth := width - overlayWidth
	if leftWidth < 0 {
		leftWidth = 0
	}

	var result []string
	for i := 0; i < height; i++ {
		baseLine := ""
		if i < len(baseLines) {
			baseLine = baseLines[i]
		}
		// Truncate or pad base line to leftWidth
		baseW := lipgloss.Width(baseLine)
		if baseW > leftWidth {
			baseLine = truncateToWidth(baseLine, leftWidth)
		} else if baseW < leftWidth {
			baseLine += strings.Repeat(" ", leftWidth-baseW)
		}

		overlayLine := ""
		if i < len(overlayLines) {
			overlayLine = overlayLines[i]
		}
		overlayW := lipgloss.Width(overlayLine)
		if overlayW > overlayWidth {
			overlayLine = truncateToWidth(overlayLine, overlayWidth)
		} else if overlayW < overlayWidth {
			overlayLine += strings.Repeat(" ", overlayWidth-overlayW)
		}

		// Style overlay with surface background
		styledOverlay := lipgloss.NewStyle().
			Background(t.Surface).
			Width(overlayWidth).
			Render(overlayLine)

		result = append(result, baseLine+styledOverlay)
	}

	return strings.Join(result, "\n")
}

// truncateToWidth truncates a styled string to at most maxW visible cells,
// preserving ANSI escape sequences.
func truncateToWidth(s string, maxW int) string {
	var out strings.Builder
	visible := 0
	truncated := false
	inEsc := false
	esc := strings.Builder{}
	for _, r := range s {
		if inEsc {
			esc.WriteRune(r)
			if r == 'm' {
				out.WriteString(esc.String())
				esc.Reset()
				inEsc = false
			}
			continue
		}
		if r == '\x1b' {
			inEsc = true
			esc.WriteRune(r)
			continue
		}
		if visible >= maxW {
			truncated = true
			break
		}
		out.WriteRune(r)
		visible++
	}
	if truncated {
		out.WriteString("\x1b[0m")
	}
	return out.String()
}
