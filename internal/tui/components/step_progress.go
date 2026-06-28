package components

import (
	"fmt"
	"strings"

	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/tui/theme"
)

// StepProgress renders a step progress indicator with dots and optional labels.
// Used for multi-step workflows like setup wizards.
type StepProgress struct {
	Current     int      // Current step (0-indexed)
	Total       int      // Total number of steps
	Labels      []string // Optional labels for each step
	Theme       theme.Theme
	Width       int
	PaddingLeft int
}

// RenderDots renders progress as dots (● ○ ✓).
func (sp StepProgress) RenderDots() string {
	t := sp.Theme
	if t.Brand == "" {
		t = theme.Default()
	}
	s := theme.BuildSemanticStyles(t)

	padLeft := sp.PaddingLeft
	if padLeft == 0 {
		padLeft = 2
	}

	var parts []string
	for i := 0; i < sp.Total; i++ {
		var dot string
		switch {
		case i < sp.Current:
			dot = s.ProgressDone.Render("✓")
		case i == sp.Current:
			dot = s.ProgressFill.Bold(true).Render("●")
		default:
			dot = s.ProgressStep.Render("○")
		}
		parts = append(parts, dot)
	}

	return lipgloss.NewStyle().
		PaddingLeft(padLeft).
		Render(strings.Join(parts, " "))
}

// RenderBar renders progress as a labeled horizontal bar.
func (sp StepProgress) RenderBar() string {
	t := sp.Theme
	if t.Brand == "" {
		t = theme.Default()
	}
	s := theme.BuildSemanticStyles(t)

	padLeft := sp.PaddingLeft
	if padLeft == 0 {
		padLeft = 2
	}

	w := sp.Width
	if w < 20 {
		w = 40
	}

	// Calculate bar width (leave room for labels)
	barWidth := w - 4
	if barWidth < 10 {
		barWidth = 10
	}

	// Build progress bar
	filled := 0
	if sp.Total > 0 {
		filled = (sp.Current + 1) * barWidth / sp.Total
	}

	bar := s.ProgressFill.Render(strings.Repeat("━", filled)) +
		s.ProgressEmpty.Render(strings.Repeat("─", barWidth-filled))

	// Add step info
	info := fmt.Sprintf(" %d/%d", sp.Current+1, sp.Total)

	return lipgloss.NewStyle().
		PaddingLeft(padLeft).
		Render(bar + info)
}

// RenderLabeled renders progress with step labels below the dots.
func (sp StepProgress) RenderLabeled() string {
	t := sp.Theme
	if t.Brand == "" {
		t = theme.Default()
	}
	s := theme.BuildSemanticStyles(t)

	padLeft := sp.PaddingLeft
	if padLeft == 0 {
		padLeft = 2
	}

	var dotParts []string
	var labelParts []string

	for i := 0; i < sp.Total; i++ {
		var dot string
		switch {
		case i < sp.Current:
			dot = s.ProgressDone.Render("✓")
		case i == sp.Current:
			dot = s.ProgressFill.Bold(true).Render("●")
		default:
			dot = s.ProgressStep.Render("○")
		}
		dotParts = append(dotParts, dot)

		// Label
		if i < len(sp.Labels) && sp.Labels[i] != "" {
			labelStyle := s.Muted
			if i == sp.Current {
				labelStyle = s.Body
			} else if i < sp.Current {
				labelStyle = s.SuccessText
			}
			labelParts = append(labelParts, labelStyle.Render(sp.Labels[i]))
		} else {
			labelParts = append(labelParts, "")
		}
	}

	dots := lipgloss.NewStyle().
		PaddingLeft(padLeft).
		Render(strings.Join(dotParts, " "))

	labels := lipgloss.NewStyle().
		PaddingLeft(padLeft).
		Render(strings.Join(labelParts, "  "))

	return dots + "\n" + labels
}
