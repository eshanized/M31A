package components

import (
	"fmt"
	"math"
	"strings"

	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/tui/theme"
)

// ProgressBar renders a visual progress indicator.
type ProgressBar struct {
	Progress float64 // 0.0 to 1.0
	Width    int     // total width including percentage
	ShowPct  bool    // show percentage text
	Style    ProgressBarStyle
}

// ProgressBarStyle defines the visual style of the progress bar.
type ProgressBarStyle int

const (
	ProgressBarThin ProgressBarStyle = iota
	ProgressBarThick
	ProgressBarBlock
	ProgressBarRounded
)

// Render returns the progress bar as a string.
func (p ProgressBar) Render() string {
	if p.Width <= 0 {
		p.Width = 30
	}

	t := theme.Default()
	pct := p.Progress
	if pct < 0 {
		pct = 0
	}
	if pct > 1 {
		pct = 1
	}

	filledWidth := int(math.Round(pct * float64(p.Width)))
	if filledWidth > p.Width {
		filledWidth = p.Width
	}
	emptyWidth := p.Width - filledWidth

	var bar string

	switch p.Style {
	case ProgressBarThin:
		filled := strings.Repeat("━", filledWidth)
		empty := strings.Repeat("─", emptyWidth)
		bar = filled + empty

	case ProgressBarThick:
		filled := strings.Repeat("█", filledWidth)
		empty := strings.Repeat("░", emptyWidth)
		bar = filled + empty

	case ProgressBarBlock:
		filled := strings.Repeat("█", filledWidth)
		empty := strings.Repeat("░", emptyWidth)
		bar = filled + empty

	case ProgressBarRounded:
		filled := strings.Repeat("▓", filledWidth)
		empty := strings.Repeat("░", emptyWidth)
		bar = filled + empty

	default:
		filled := strings.Repeat("█", filledWidth)
		empty := strings.Repeat("░", emptyWidth)
		bar = filled + empty
	}

	// Apply styling
	progressStyle := lipgloss.NewStyle().Foreground(t.Brand)
	if pct >= 1.0 {
		progressStyle = progressStyle.Foreground(t.Success)
	} else if pct == 0 {
		progressStyle = progressStyle.Foreground(t.TextSecondary)
	}

	bar = progressStyle.Render(bar)

	if p.ShowPct {
		pctStr := fmt.Sprintf("%d%%", int(math.Round(pct*100)))
		pctStyle := lipgloss.NewStyle().
			Foreground(t.TextSecondary)
		bar = bar + " " + pctStyle.Render(pctStr)
	}

	return bar
}

// SegmentedBar renders a bar segmented by different states.
type SegmentedBar struct {
	Segments []Segment
	Width    int
}

// Segment represents a portion of a segmented bar.
type Segment struct {
	Count int
	Color lipgloss.Color
	Label string
}

// Render returns the segmented bar as a string.
func (s SegmentedBar) Render() string {
	if s.Width <= 0 {
		s.Width = 40
	}

	total := 0
	for _, seg := range s.Segments {
		total += seg.Count
	}

	if total == 0 {
		return strings.Repeat("░", s.Width)
	}

	var bar strings.Builder
	remaining := s.Width

	for _, seg := range s.Segments {
		if remaining <= 0 {
			break
		}

		width := int(math.Round(float64(seg.Count) / float64(total) * float64(s.Width)))
		if width > remaining {
			width = remaining
		}

		style := lipgloss.NewStyle().Foreground(seg.Color)
		bar.WriteString(style.Render(strings.Repeat("█", width)))
		remaining -= width
	}

	// Fill remaining with background
	if remaining > 0 {
		t := theme.Default()
		bar.WriteString(lipgloss.NewStyle().Foreground(t.Border).Render(strings.Repeat("░", remaining)))
	}

	return bar.String()
}

// CompactProgress renders a compact inline progress indicator.
type CompactProgress struct {
	Progress  float64
	ShowChars bool // show unicode progress chars
}

// Render returns a compact progress string.
func (c CompactProgress) Render() string {
	pct := c.Progress
	if pct < 0 {
		pct = 0
	}
	if pct > 1 {
		pct = 1
	}

	if c.ShowChars {
		// Show progress as fraction of 10 blocks
		blocks := int(math.Round(pct * 10))
		filled := strings.Repeat("█", blocks)
		empty := strings.Repeat("░", 10-blocks)
		return filled + empty
	}

	return fmt.Sprintf("[%d%%]", int(math.Round(pct*100)))
}
