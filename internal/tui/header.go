package tui

import (
	"strings"

	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/tui/theme"
	"github.com/eshanized/M31A/internal/types"
)

// RenderPhaseBadge renders a compact phase indicator for the header.
// Shows e.g. "[init]" in brand color with brand border when workflow is active.
func RenderPhaseBadge(t theme.Theme, phase string) string {
	return lipgloss.NewStyle().
		Foreground(t.Brand).
		Border(lipgloss.RoundedBorder()).
		BorderForeground(t.Brand).
		Padding(0, 1).
		Render(phase)
}

// RenderPhaseBreadcrumb renders the workflow phase breadcrumb using theme.PhaseActive/PhasePast/PhaseFuture.
func RenderPhaseBreadcrumb(t theme.Theme, phase types.WorkflowPhase) string {
	if phase == types.PhaseIdle || phase == "" {
		return ""
	}

	phases := []types.WorkflowPhase{
		types.PhaseInitialize,
		types.PhaseDiscuss,
		types.PhasePlan,
		types.PhaseExecute,
		types.PhaseVerify,
		types.PhaseShip,
	}

	// Find position of current phase
	currentIdx := -1
	for i, p := range phases {
		if p == phase {
			currentIdx = i
			break
		}
	}

	var parts []string
	for i, p := range phases {
		var style lipgloss.Style
		if p == phase {
			// Active phase: use theme.PhaseActive (brand + bold)
			style = t.PhaseActive
			if style.GetForeground() == (lipgloss.Color("")) {
				style = lipgloss.NewStyle().Foreground(t.Brand).Bold(true)
			}
		} else if currentIdx >= 0 && i < currentIdx {
			// Past phase: use theme.PhasePast (muted)
			style = t.PhasePast
			if style.GetForeground() == (lipgloss.Color("")) {
				style = lipgloss.NewStyle().Foreground(t.TextMuted).Faint(true)
			}
		} else {
			// Future phase: use theme.PhaseFuture (even more muted)
			style = t.PhaseFuture
			if style.GetForeground() == (lipgloss.Color("")) {
				style = lipgloss.NewStyle().Foreground(t.TextMuted)
			}
		}
		parts = append(parts, style.Render(string(p)))
	}
	sep := lipgloss.NewStyle().Foreground(t.TextMuted).Render(" › ")
	return strings.Join(parts, sep)
}
