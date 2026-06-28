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
	s := theme.BuildSemanticStyles(t)
	return s.BrandBold.
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

	s := theme.BuildSemanticStyles(t)

	phases := []types.WorkflowPhase{
		types.PhaseInitialize,
		types.PhaseDiscuss,
		types.PhasePlan,
		types.PhaseExecute,
		types.PhaseVerify,
		types.PhaseShip,
	}

	currentIdx := -1
	for i, p := range phases {
		if p == phase {
			currentIdx = i
			break
		}
	}

	var parts []string
	for i, p := range phases {
		var style = s.Muted
		if p == phase {
			style = s.BrandBold
		} else if currentIdx >= 0 && i < currentIdx {
			style = s.Muted.Faint(true)
		}
		parts = append(parts, style.Render(string(p)))
	}
	sep := s.SeparatorV.Render(" › ")
	return strings.Join(parts, sep)
}
