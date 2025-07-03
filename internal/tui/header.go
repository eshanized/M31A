package tui

import (
	"fmt"
	"hash/fnv"
	"os"

	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/tui/theme"
	"github.com/eshanized/M31A/internal/types"
)

// RenderHeader is a pure function that builds the header string from
// the provided parameters. It is also called by CachedRenderHeader
// when the cache is invalidated.
func RenderHeader(t theme.Theme, provider string, model *types.ModelInfo,
	health types.HealthStatus, contextUsed int64, contextTotal int64, width int) string {

	brand := t.Header.Render("M31A")

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

	var modelSegment string
	if model != nil {
		name := model.Name
		runes := []rune(name)
		if len(runes) > 20 {
			name = string(runes[:20]) + "..."
		}
		modelSegment = t.ModelBadge.Render(name)
	}

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
		case ratio >= 0.80:
			color = t.Warning
		default:
			color = t.TextSecondary
		}
		ctxText := fmt.Sprintf("%s/%s ctx", usedStr, totalStr)
		contextSegment = t.ContextBar.Foreground(color).Render(ctxText)
	}

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
	key := computeHeaderKey(
		m.activeProvider, modelID,
		contextUsed, contextTotal,
		m.healthStatus.Status, m.width,
	)
	if m.headerCacheValid && m.headerCacheKey == key {
		return m.headerCacheValue
	}
	t := m.themeManager.Current()
	result := RenderHeader(t, m.activeProvider, m.activeModel, m.healthStatus,
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
