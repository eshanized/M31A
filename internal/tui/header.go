package tui

import (
	"fmt"
	"strings"

	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/tui/theme"
	"github.com/eshanized/M31A/internal/types"
)

func RenderHeader(t theme.Theme, provider string, model *types.ModelInfo,
	health types.HealthStatus, contextUsed int64, contextTotal int64, width int) string {

	brand := t.Header.Render("M31A")

	var badgeStyle lipgloss.Style
	var badgeText string
	if provider == "" {
		badgeStyle = t.ModelBadge.Foreground(t.TextSecondary)
		badgeText = "[  ]"
	} else {
		var badge string
		switch provider {
		case "openrouter":
			badgeStyle = t.ModelBadge.Foreground(t.Warning)
			badge = "OR"
		case "zen":
			badgeStyle = t.ModelBadge.Foreground(t.Thinking)
			badge = "ZEN"
		default:
			badgeStyle = t.ModelBadge.Foreground(t.TextSecondary)
			badge = strings.ToUpper(provider[:min(len(provider), 3)])
		}
		badgeText = "[" + badge + "]"
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
			result = truncateWithANSI(result, width-3)
		} else if width < 20 {
			result = "..."
		}
	}

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

// truncateWithANSI truncates a string containing ANSI escape codes to a given
// display width, appending ellipsis. It preserves ANSI codes up to the
// truncation point and closes any open SGR sequences.
func truncateWithANSI(s string, maxWidth int) string {
	if lipgloss.Width(s) <= maxWidth {
		return s
	}

	var result strings.Builder
	visible := 0
	inEscape := false

	for i := 0; i < len(s); i++ {
		b := s[i]
		if b == '\x1b' {
			inEscape = true
			result.WriteByte(b)
			continue
		}
		if inEscape {
			result.WriteByte(b)
			if b == 'm' {
				inEscape = false
			}
			continue
		}
		visible++
		if visible > maxWidth {
			break
		}
		result.WriteByte(b)
	}

	result.WriteString("\x1b[0m...")
	return result.String()
}
