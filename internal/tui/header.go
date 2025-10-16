package tui

import (
	"fmt"
	"hash/fnv"
	"os"
	"strings"

	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/tui/theme"
	"github.com/eshanized/M31A/internal/types"
)

// PhaseBreadcrumb holds the current workflow phase for header rendering.
type PhaseBreadcrumb struct {
	Current types.WorkflowPhase
}

// RenderHeader is a pure function that builds the header string from
// the provided parameters. It is also called by CachedRenderHeader
// when the cache is invalidated.
// Redesigned with block-character anchors (▓▓▓ M31A ▓) and phase breadcrumb.
func RenderHeader(t theme.Theme, provider string, model *types.ModelInfo,
	health types.HealthStatus, contextUsed int64, contextTotal int64, width int) string {

	// Left anchor: block characters for visual weight
	anchorStyle := lipgloss.NewStyle().
		Foreground(t.Brand).
		Bold(true)
	brand := anchorStyle.Render("▓▓▓ M31A ▓")

	// Provider badge
	var badgeText string
	var badgeStyle lipgloss.Style
	if provider == "" {
		badgeStyle = t.ModelBadge.Foreground(t.TextSecondary)
		badgeText = "[  ]"
	} else {
		badge, style := ProviderBadge(t, provider)
		badgeText = "[" + badge + "]"
		badgeStyle = style
	}
	badge := badgeStyle.Render(badgeText)

	// Model name — truncated to fit, neutral color
	var modelSegment string
	if model != nil {
		name := model.Name
		runes := []rune(name)
		if len(runes) > 20 {
			name = string(runes[:20]) + "..."
		}
		modelSegment = lipgloss.NewStyle().Foreground(t.TextSecondary).Render(name)
	}

	// Context bar — right-aligned, color-coded
	var contextSegment string
	if contextTotal == 0 {
		contextSegment = t.ContextBar.Render("--/-- ctx")
	} else {
		usedStr := formatContextValue(contextUsed)
		totalStr := formatContextValue(contextTotal)
		ratio := float64(contextUsed) / float64(contextTotal)
		var color lipgloss.Color
		switch {
		case ratio >= 0.95:
			color = t.Error
		case ratio >= types.ContextWarningThreshold:
			color = t.Warning
		default:
			color = t.TextSecondary
		}
		ctxText := fmt.Sprintf("%s/%s ctx", usedStr, totalStr)
		contextSegment = t.ContextBar.Foreground(color).Render(ctxText)
	}

	// Health status
	var healthSegment string
	switch health.Status {
	case "live":
		healthSegment = t.StatusLive.Render("[LIVE]")
	case "slow":
		healthSegment = t.StatusSlow.Render("[SLOW]")
	case "offline":
		healthSegment = t.StatusOffline.Render("[OFF]")
	default:
		healthSegment = t.StatusOffline.Render("[??]")
	}

	// Assemble left side
	segments := []string{brand}
	if badge != "" {
		segments = append(segments, badge)
	}
	if modelSegment != "" {
		segments = append(segments, modelSegment)
	}
	segments = append(segments, contextSegment, healthSegment)

	result := lipgloss.JoinHorizontal(lipgloss.Left, segments...)
	if lipgloss.Width(result) > width {
		if modelSegment != "" && lipgloss.Width(result)-lipgloss.Width(modelSegment) <= width {
			segments = removeModelSegment(segments)
			result = lipgloss.JoinHorizontal(lipgloss.Left, segments...)
		}
		if lipgloss.Width(result) > width && width >= 20 {
			result = TruncateWithEllipsis(result, width-3)
		} else if width < 20 {
			result = "..."
		}
	}

	return result
}

// RenderPhaseBreadcrumb renders a horizontal breadcrumb showing workflow phases.
// Past phases are muted, current is brand+bold with ↑ pointer, future is muted.
func RenderPhaseBreadcrumb(t theme.Theme, current types.WorkflowPhase, width int) string {
	phases := []struct {
		name string
		id   types.WorkflowPhase
	}{
		{"DISCUSS", types.PhaseDiscuss},
		{"PLAN", types.PhasePlan},
		{"EXECUTE", types.PhaseExecute},
		{"VERIFY", types.PhaseVerify},
		{"SHIP", types.PhaseShip},
	}

	var parts []string
	for i, p := range phases {
		var style lipgloss.Style
		if p.id == current {
			style = t.PhaseActive
		} else if phaseOrder(p.id) < phaseOrder(current) {
			style = t.PhasePast
		} else {
			style = t.PhaseFuture
		}

		text := p.name
		if p.id == current {
			text = "↑ " + text
		}

		parts = append(parts, style.Render(text))

		// Add separator between phases (not after last)
		if i < len(phases)-1 {
			sep := lipgloss.NewStyle().Foreground(t.TextMuted).Render(" ─── ")
			parts = append(parts, sep)
		}
	}

	result := strings.Join(parts, "")
	if lipgloss.Width(result) > width {
		result = TruncateWithEllipsis(result, width)
	}
	return result
}

// phaseOrder returns a numeric order for workflow phases.
func phaseOrder(p types.WorkflowPhase) int {
	switch p {
	case types.PhaseInitialize:
		return 0
	case types.PhaseDiscuss:
		return 1
	case types.PhasePlan:
		return 2
	case types.PhaseExecute:
		return 3
	case types.PhaseVerify:
		return 4
	case types.PhaseShip:
		return 5
	default:
		return -1
	}
}

// computeHeaderKey builds a FNV-1a hash over the header inputs.
// When any input changes the hash differs and the cache is invalidated.
// M-26: includes M31A_LOG_LEVEL so debug-mode headers invalidate correctly.
func computeHeaderKey(provider string, modelID string, contextUsed, contextTotal int64,
	healthStatus string, width int) uint64 {

	h := fnv.New64a()
	fmt.Fprintf(h, "%s|%s|%d|%d|%s|%d|%s",
		provider, modelID, contextUsed, contextTotal, healthStatus, width,
		os.Getenv("M31A_LOG_LEVEL"))
	return h.Sum64()
}

// CachedRenderHeader returns the header string, using a content-based cache
// to avoid redundant lipgloss re-renders on every TickMsg (H-10 fix).
// The cache key includes the log level env var (M-26 fix).
// Context values must be passed explicitly because AppState does not store them.
func (m *AppState) CachedRenderHeader(contextUsed, contextTotal int64) string {
	modelID := ""
	if m.activeModel != nil {
		modelID = m.activeModel.ID
	}
	// Atomic read for health status to avoid data races.
	var health types.HealthStatus
	if v := m.healthStatusAtomic.Load(); v != nil {
		health = v.(types.HealthStatus)
	} else {
		health = m.healthStatus // fallback for initialization
	}
	key := computeHeaderKey(
		m.activeProvider, modelID,
		contextUsed, contextTotal,
		health.Status, m.width,
	)
	if m.headerCacheValid && m.headerCacheKey == key {
		return m.headerCacheValue
	}
	t := m.themeManager.Current()
	result := RenderHeader(t, m.activeProvider, m.activeModel, health,
		contextUsed, contextTotal, m.width)
	m.headerCacheValue = result
	m.headerCacheKey = key
	m.headerCacheValid = true
	return result
}

func formatContextValue(v int64) string {
	if v >= 1000 {
		return fmt.Sprintf("%.1fK", float64(v)/1000)
	}
	return fmt.Sprintf("%d", v)
}

func removeModelSegment(segments []string) []string {
	for i, s := range segments {
		if s != "" && s != segments[0] {
			return append(segments[:i], segments[i+1:]...)
		}
	}
	return segments
}
