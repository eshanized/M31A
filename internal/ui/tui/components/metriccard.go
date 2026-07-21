package components

import (
	"fmt"

	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/ui/tui/theme"
)

// MetricCard renders a large number with label and optional trend.
type MetricCard struct {
	Value string
	Label string
	Trend string // optional, e.g. "+12%" or "↑ 3"
	Width int
	Align lipgloss.Position
	Theme theme.Theme
}

// Render returns the metric card as a styled string.
func (m MetricCard) Render() string {
	t := m.Theme
	if t.Text == "" {
		t = theme.Default()
	}
	s := theme.BuildSemanticStyles(t)

	valueStyle := s.BrandBold

	labelStyle := s.SecondaryText

	trendStyle := s.SuccessText

	// Parse trend to determine if it's positive or negative
	if m.Trend != "" {
		firstRune := []rune(m.Trend)[0]
		if firstRune == '-' || firstRune == '↓' {
			trendStyle = s.ErrorText
		}
	}

	value := valueStyle.Render(m.Value)
	label := labelStyle.Render(m.Label)

	var parts string
	if m.Trend != "" {
		trend := trendStyle.Render(m.Trend)
		parts = lipgloss.JoinVertical(lipgloss.Left,
			lipgloss.JoinHorizontal(lipgloss.Top, value, " ", trend),
			label,
		)
	} else {
		parts = lipgloss.JoinVertical(lipgloss.Left,
			value,
			label,
		)
	}

	if m.Width > 0 {
		align := lipgloss.Left
		switch m.Align {
		case lipgloss.Center:
			align = lipgloss.Center
		case lipgloss.Right:
			align = lipgloss.Right
		}
		parts = lipgloss.NewStyle().
			Width(m.Width).
			Align(align).
			Render(parts)
	}

	return parts
}

// FormatMetric formats a number with K/M suffix if large.
func FormatMetric(n int) string {
	if n >= 1000000 {
		return fmt.Sprintf("%.1fM", float64(n)/1000000)
	}
	if n >= 1000 {
		return fmt.Sprintf("%.1fK", float64(n)/1000)
	}
	return fmt.Sprintf("%d", n)
}

// FormatCost formats a cost value with $ prefix and 4 decimal places.
func FormatCost(cost float64) string {
	return fmt.Sprintf("$%.4f", cost)
}

// FormatDuration formats seconds to a human-readable duration.
func FormatDuration(seconds int) string {
	if seconds < 60 {
		return fmt.Sprintf("%ds", seconds)
	}
	if seconds < 3600 {
		return fmt.Sprintf("%dm %ds", seconds/60, seconds%60)
	}
	h := seconds / 3600
	m := (seconds % 3600) / 60
	s := seconds % 60
	return fmt.Sprintf("%dh %dm %ds", h, m, s)
}

// MetricRow renders multiple metric cards in a row.
func MetricRow(metrics []MetricCard, totalWidth int) string {
	if len(metrics) == 0 {
		return ""
	}

	cards := make([]string, len(metrics))
	cardWidth := totalWidth / len(metrics)
	if cardWidth < 8 {
		cardWidth = 8
	}
	// Ensure total card widths don't exceed available space
	if cardWidth*len(metrics) > totalWidth {
		cardWidth = totalWidth / len(metrics)
	}

	for i, m := range metrics {
		m.Width = cardWidth
		cards[i] = m.Render()
	}

	return lipgloss.JoinHorizontal(lipgloss.Top, cards...)
}
