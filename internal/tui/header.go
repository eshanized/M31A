package tui

import (
	"fmt"
	"hash/fnv"
	"strings"

	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/tui/theme"
	"github.com/eshanized/M31A/internal/types"
)

// headerCacheKey is a composite key for the header render cache.
type headerCacheKey struct {
	provider string
	modelID  string
	ctxUsed  int
	ctxTotal int
	health   string
	width    int
}

// computeHeaderKey hashes the key into a uint64 for fast comparison.
func computeHeaderKey(k headerCacheKey) uint64 {
	h := fnv.New64a()
	fmt.Fprintf(h, "%s|%s|%d|%d|%s|%d",
		k.provider, k.modelID, k.ctxUsed, k.ctxTotal, k.health, k.width)
	return h.Sum64()
}

// RenderHeader renders the application header bar.
//
// The header contains three zones:
//   - Left:   M31A branding + version
//   - Center: active phase breadcrumb
//   - Right:  model/provider badge + context window meter
func RenderHeader(
	t theme.Theme,
	version string,
	activeProvider string,
	modelID string,
	modelName string,
	phase types.WorkflowPhase,
	ctxUsed int,
	ctxTotal int,
	healthStatus string,
	showCost bool,
	width int,
) string {
	if width < 20 {
		return ""
	}

	// ── Left: brand ──────────────────────────────────────────────────────────
	brandStyle := lipgloss.NewStyle().Foreground(t.Brand).Bold(true)
	versionStyle := lipgloss.NewStyle().Foreground(t.TextMuted)

	v := version
	if v == "" {
		v = "dev"
	}
	left := brandStyle.Render("M31A") + " " + versionStyle.Render(v)

	// ── Center: phase breadcrumb ─────────────────────────────────────────────
	center := RenderPhaseBreadcrumb(t, phase)

	// ── Right: model badge + ctx meter ───────────────────────────────────────
	var rightParts []string

	// Context window meter (only shown when we have context info)
	if ctxTotal > 0 {
		pct := float64(ctxUsed) / float64(ctxTotal)
		var ctxColor lipgloss.Color
		switch {
		case pct >= 0.9:
			ctxColor = t.Error
		case pct >= 0.7:
			ctxColor = t.Warning
		default:
			ctxColor = t.TextMuted
		}
		ctxStr := fmt.Sprintf("%s/%s ctx",
			formatSI(ctxUsed),
			formatSI(ctxTotal),
		)
		rightParts = append(rightParts,
			lipgloss.NewStyle().Foreground(ctxColor).Render(ctxStr))
	}

	// Health indicator
	if healthStatus != "" && healthStatus != "ok" {
		switch healthStatus {
		case "degraded":
			rightParts = append(rightParts,
				lipgloss.NewStyle().Foreground(t.Warning).Render("⚡"))
		case "down":
			rightParts = append(rightParts,
				lipgloss.NewStyle().Foreground(t.Error).Render("✗"))
		}
	}

	// Model/provider badge
	if modelName != "" || activeProvider != "" {
		display := modelName
		if display == "" {
			display = modelID
		}
		if activeProvider != "" {
			display += " [" + ProviderShortName(activeProvider) + "]"
		}
		rightParts = append(rightParts,
			lipgloss.NewStyle().Foreground(t.TextMuted).Render(display))
	}

	right := strings.Join(rightParts, "  ")

	// ── Layout ───────────────────────────────────────────────────────────────
	leftW := lipgloss.Width(left)
	centerW := lipgloss.Width(center)
	rightW := lipgloss.Width(right)

	// Distribute available padding
	totalUsed := leftW + centerW + rightW
	padding := width - totalUsed
	if padding < 2 {
		// Truncate center if no room
		center = ""
		centerW = 0
		padding = width - leftW - rightW
		if padding < 0 {
			padding = 0
		}
	}

	// Distribute padding around center
	padLeft := (padding - centerW) / 2
	padRight := padding - padLeft - centerW
	if padLeft < 0 {
		padLeft = 0
	}
	if padRight < 0 {
		padRight = 0
	}

	bar := left +
		strings.Repeat(" ", padLeft) +
		center +
		strings.Repeat(" ", padRight) +
		right

	return lipgloss.NewStyle().
		Background(t.Surface).
		Foreground(t.Text).
		Width(width).
		Render(bar)
}

// RenderPhaseBreadcrumb renders the workflow phase breadcrumb.
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

	var parts []string
	for _, p := range phases {
		if p == phase {
			parts = append(parts, lipgloss.NewStyle().Foreground(t.Brand).Bold(true).Render(string(p)))
		} else {
			parts = append(parts, lipgloss.NewStyle().Foreground(t.TextMuted).Render(string(p)))
		}
	}
	return strings.Join(parts, lipgloss.NewStyle().Foreground(t.TextMuted).Render(" › "))
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
