package tui

import (
	"fmt"
	"strings"

	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/tui/theme"
	"github.com/eshanized/M31A/internal/types"
)

// RenderHeader renders the application header bar (1 line).
//
// Layout: left (M31A brand) | center (phase badge or git branch) | right (model + provider badge + ctx meter)
// No version, no health indicator, no full-width background.
func RenderHeader(
	t theme.Theme,
	activeProvider string,
	modelID string,
	modelName string,
	phase types.WorkflowPhase,
	gitBranch string,
	ctxUsed int,
	ctxTotal int,
	width int,
) string {
	if width < 20 {
		return ""
	}

	// ── Left: Brand ───────────────────────────────────────────────────────────
	left := lipgloss.NewStyle().Foreground(t.Brand).Bold(true).Render("M31A")

	// ── Center: Phase badge or git branch ─────────────────────────────────────
	var center string
	if phase != types.PhaseIdle && phase != "" {
		center = RenderPhaseBadge(t, string(phase))
	} else if gitBranch != "" {
		center = lipgloss.NewStyle().Foreground(t.TextMuted).Render("⎇ " + gitBranch)
	}

	// ── Right: Context meter + model/provider badge ──────────────────────────
	var rightParts []string

	// Context meter (visual bar)
	if ctxTotal > 0 {
		rightParts = append(rightParts, renderContextMeter(ctxUsed, ctxTotal, t))
	}

	// Model/provider badge
	if modelName != "" || activeProvider != "" {
		display := modelName
		if display == "" {
			display = modelID
		}
		modelStr := lipgloss.NewStyle().Foreground(t.TextMuted).Render(display)
		if activeProvider != "" {
			providerStr := renderProviderBadge(t, activeProvider)
			rightParts = append(rightParts, modelStr+" "+providerStr)
		} else {
			rightParts = append(rightParts, modelStr)
		}
	}

	right := strings.Join(rightParts, "  ")

	// ── Layout ────────────────────────────────────────────────────────────
	leftW := lipgloss.Width(left)
	centerW := lipgloss.Width(center)
	rightW := lipgloss.Width(right)

	totalUsed := leftW + centerW + rightW
	padding := width - totalUsed
	if padding < 2 {
		center = ""
		centerW = 0
		padding = width - leftW - rightW
		if padding < 0 {
			padding = 0
		}
	}

	padLeft := (padding - centerW) / 2
	padRight := padding - padLeft - centerW
	if padLeft < 0 {
		padLeft = 0
	}
	if padRight < 0 {
		padRight = 0
	}

	return left +
		strings.Repeat(" ", padLeft) +
		center +
		strings.Repeat(" ", padRight) +
		right
}

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

// renderContextMeter renders a compact visual context usage bar:
//
//	ctx [████░░░░] 42%
func renderContextMeter(used, total int, t theme.Theme) string {
	if total <= 0 {
		return ""
	}
	pct := float64(used) / float64(total)
	var ctxColor lipgloss.Color
	switch {
	case pct >= 0.9:
		ctxColor = t.Error
	case pct >= 0.7:
		ctxColor = t.Warning
	default:
		ctxColor = t.TextMuted
	}

	const barSegments = 8
	filled := int(pct * barSegments)
	if filled > barSegments {
		filled = barSegments
	}
	if filled < 0 {
		filled = 0
	}

	bar := "["
	bar += strings.Repeat("█", filled)
	bar += strings.Repeat("░", barSegments-filled)
	bar += "]"

	return lipgloss.NewStyle().Foreground(ctxColor).Render(
		fmt.Sprintf("ctx %s %d%%", bar, int(pct*100)),
	)
}

// renderProviderBadge renders a short provider badge like [OR] or [ZEN].
// OpenRouter gets brand color, Zen gets info color.
func renderProviderBadge(t theme.Theme, activeProvider string) string {
	shortName := strings.ToUpper(ProviderShortName(activeProvider))
	var fg lipgloss.Color
	if shortName == "OR" {
		fg = t.Brand
	} else {
		fg = t.Info
	}
	return lipgloss.NewStyle().
		Border(lipgloss.RoundedBorder()).
		BorderForeground(fg).
		Foreground(fg).
		Padding(0, 1).
		Render("[" + shortName + "]")
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

// formatSI formats an integer with SI suffix (K, M).
func formatSI(n int) string {
	switch {
	case n >= 1_000_000:
		return fmt.Sprintf("%.1fM", float64(n)/1_000_000)
	case n >= 1_000:
		return fmt.Sprintf("%.0fK", float64(n)/1_000)
	default:
		return fmt.Sprintf("%d", n)
	}
}

// formatDurationMs formats a duration in milliseconds as a human-readable string.
func formatDurationMs(ms int64) string {
	if ms < 0 {
		return "0s"
	}
	s := ms / 1000
	m := s / 60
	h := m / 60
	switch {
	case h > 0:
		return fmt.Sprintf("%dh%dm", h, m%60)
	case m > 0:
		return fmt.Sprintf("%dm%ds", m, s%60)
	default:
		return fmt.Sprintf("%ds", s)
	}
}
