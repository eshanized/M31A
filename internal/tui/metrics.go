package tui

import (
	"fmt"
	"math"
	"strings"
	"time"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/tui/components"
	"github.com/eshanized/M31A/internal/tui/theme"
	"github.com/eshanized/M31A/internal/types"
	"github.com/eshanized/M31A/pkg/session"
)

// MetricsModel is the view model for ScreenMetrics ("Flight Data").
// It reads session data locally — no new API calls, no telemetry.
type MetricsModel struct {
	theme   theme.Theme
	width   int
	height  int
	loaded  bool
	errMsg  string

	// Aggregate stats
	totalSessions int
	totalTokens   int64
	totalCost     float64
	totalToolCalls int

	// Per-model stats
	modelStats []modelUsageStat

	// Daily usage (last 14 days): day index → session count
	dailyUsage []dailyStat

	// Sparkline data for daily usage visualization
	sparklineData []float64
}

type modelUsageStat struct {
	modelName string
	sessions  int
	pct       float64
	cost      float64
}

type dailyStat struct {
	label    string // "Jun 05"
	sessions int
}

// NewMetricsModel creates a MetricsModel. Call LoadStats() to populate data.
func NewMetricsModel(t theme.Theme) *MetricsModel {
	return &MetricsModel{theme: t}
}

// LoadStats populates the metrics model from the session manager.
// Must be called from Update(), never from goroutines.
func (m *MetricsModel) LoadStats(mgr *session.Manager) {
	if mgr == nil {
		m.errMsg = "Session manager unavailable"
		m.loaded = true
		return
	}

	sessions, err := mgr.ListSessions()
	if err != nil {
		m.errMsg = fmt.Sprintf("Cannot load sessions: %v", err)
		m.loaded = true
		return
	}

	m.totalSessions = len(sessions)

	// Build daily usage map (last 14 days)
	now := time.Now()
	dailyMap := make(map[string]int)
	var dayLabels []string
	for i := 13; i >= 0; i-- {
		d := now.AddDate(0, 0, -i)
		key := d.Format(types.DateFormat)
		label := d.Format("Jan 02")
		dailyMap[key] = 0
		dayLabels = append(dayLabels, label)
	}

	// Build model usage map
	modelMap := make(map[string]int)

	for _, s := range sessions {
		// Daily usage
		key := s.StartedAt.Format(types.DateFormat)
		if _, ok := dailyMap[key]; ok {
			dailyMap[key]++
		}
		// Model distribution
		if s.Model != "" {
			modelMap[s.Model]++
		}
	}

	// Build daily stats slice in order
	m.dailyUsage = nil
	m.sparklineData = nil
	for i := 13; i >= 0; i-- {
		d := now.AddDate(0, 0, -i)
		key := d.Format(types.DateFormat)
		sessions := dailyMap[key]
		m.dailyUsage = append(m.dailyUsage, dailyStat{
			label:    dayLabels[13-i],
			sessions: sessions,
		})
		m.sparklineData = append(m.sparklineData, float64(sessions))
	}
	_ = dayLabels

	// Build model stats
	m.modelStats = nil
	total := float64(m.totalSessions)
	if total == 0 {
		total = 1
	}
	for model, count := range modelMap {
		m.modelStats = append(m.modelStats, modelUsageStat{
			modelName: model,
			sessions:  count,
			pct:       float64(count) / total * 100,
		})
	}
	// Sort by session count descending (simple insertion sort for small slices)
	for i := 1; i < len(m.modelStats); i++ {
		for j := i; j > 0 && m.modelStats[j].sessions > m.modelStats[j-1].sessions; j-- {
			m.modelStats[j], m.modelStats[j-1] = m.modelStats[j-1], m.modelStats[j]
		}
	}

	m.loaded = true
}

func (m *MetricsModel) Init() tea.Cmd { return nil }

func (m *MetricsModel) Update(msg tea.Msg) ([]tea.Cmd, *AppMsg) {
	switch msg := msg.(type) {
	case tea.WindowSizeMsg:
		m.width = msg.Width
		m.height = msg.Height
	case tea.KeyMsg:
		switch msg.String() {
		case "esc", "q":
			return nil, &AppMsg{Screen: ScreenREPL}
		}
	}
	return nil, nil
}

func (m *MetricsModel) View() string {
	if m.width == 0 {
		return "Loading metrics..."
	}

	t := m.theme
	var sb strings.Builder

	// ╭─ Flight Data ── Session Analytics ─────────────────────────────────────╮
	headerLine := fmt.Sprintf(" Flight Data ── Session Analytics ── Last 14 days ")
	innerW := m.width - 4
	if innerW < 30 {
		innerW = 30
	}
	runes := []rune(headerLine)
	if len(runes) < innerW {
		headerLine += strings.Repeat("─", innerW-len(runes))
	}
	sb.WriteString(lipgloss.NewStyle().Foreground(t.Brand).Bold(true).Render("╭" + headerLine + "╮"))
	sb.WriteString("\n")

	if m.errMsg != "" {
		sb.WriteString(lipgloss.NewStyle().Foreground(t.Error).Render("  " + m.errMsg))
		sb.WriteString("\n")
		sb.WriteString(lipgloss.NewStyle().Foreground(t.TextMuted).Render("  Esc back"))
		return sb.String()
	}

	sb.WriteString("\n")

	// ── 4-stat summary cards ───────────────────────────────────────────────
	sb.WriteString(lipgloss.NewStyle().Foreground(t.TextSecondary).Bold(true).Render("  ── Usage Overview "))
	sb.WriteString(lipgloss.NewStyle().Foreground(t.Border).Render(strings.Repeat("─", max(0, m.width-22))))
	sb.WriteString("\n")
	sb.WriteString(m.renderStatCards())
	sb.WriteString("\n\n")

	// ── Daily usage bar chart (last 14 days) ────────────────────────────────
	sb.WriteString(lipgloss.NewStyle().Foreground(t.TextSecondary).Bold(true).Render("  ── Daily Usage (last 14 days) "))
	sb.WriteString(lipgloss.NewStyle().Foreground(t.Border).Render(strings.Repeat("─", max(0, m.width-34))))
	sb.WriteString("\n")
	sb.WriteString(m.renderDailyUsage())
	sb.WriteString("\n")

	// ── Model distribution ────────────────────────────────────────────────
	if len(m.modelStats) > 0 {
		sb.WriteString("\n")
		sb.WriteString(lipgloss.NewStyle().Foreground(t.TextSecondary).Bold(true).Render("  ── Model Distribution "))
		sb.WriteString(lipgloss.NewStyle().Foreground(t.Border).Render(strings.Repeat("─", max(0, m.width-26))))
		sb.WriteString("\n")
		sb.WriteString(m.renderModelDistribution())
	}

	sb.WriteString("\n")
	sb.WriteString(lipgloss.NewStyle().Foreground(t.TextMuted).Render("  Esc back"))
	return sb.String()
}

// renderStatCards renders 4 stat boxes: sessions, tokens, cost, tool calls.
func (m *MetricsModel) renderStatCards() string {
	t := m.theme

	stats := []struct {
		value string
		label string
	}{
		{formatMetricLarge(int(m.totalTokens)), "Total Tokens"},
		{fmt.Sprintf("$%.2f", m.totalCost), "Total Cost"},
		{fmt.Sprintf("%d", m.totalSessions), "Sessions"},
		{fmt.Sprintf("%d", m.totalToolCalls), "Tool Calls"},
	}

	cardW := (m.width - 8) / 4
	if cardW < 12 {
		cardW = 12
	}

	cards := make([]string, len(stats))
	for i, s := range stats {
		valStyle := t.MetricValue.Copy().Width(cardW - 4)
		labelStyle := t.MetricLabel.Copy()

		content := lipgloss.JoinVertical(lipgloss.Left,
			valStyle.Render(s.value),
			labelStyle.Render(s.label),
		)
		cards[i] = lipgloss.NewStyle().
			Border(lipgloss.RoundedBorder()).
			BorderForeground(t.Border).
			Padding(0, 1).
			Width(cardW).
			Render(content)
	}

	return "  " + lipgloss.JoinHorizontal(lipgloss.Top, cards...)
}

// renderDailyUsage renders a horizontal bar chart for session activity with sparkline.
func (m *MetricsModel) renderDailyUsage() string {
	t := m.theme

	if len(m.dailyUsage) == 0 {
		return lipgloss.NewStyle().Foreground(t.TextMuted).Render("  No data")
	}

	maxSessions := 0
	for _, d := range m.dailyUsage {
		if d.sessions > maxSessions {
			maxSessions = d.sessions
		}
	}
	if maxSessions == 0 {
		maxSessions = 1
	}

	// Sparkline visualization
	var sparklineStr string
	if len(m.sparklineData) > 0 {
		sparkWidth := m.width - 20
		if sparkWidth < 10 {
			sparkWidth = 10
		}
		if sparkWidth > 40 {
			sparkWidth = 40
		}
		sparklineStr = components.RenderSparkline(m.sparklineData, sparkWidth, t)
		if sparklineStr != "" {
			sparklineStr = "  " + sparklineStr + "\n"
		}
	}

	barWidth := m.width - 30
	if barWidth < 10 {
		barWidth = 10
	}
	if barWidth > 40 {
		barWidth = 40
	}

	var sb strings.Builder
	if sparklineStr != "" {
		sb.WriteString(sparklineStr)
	}
	for _, d := range m.dailyUsage {
		filled := int(math.Round(float64(d.sessions) / float64(maxSessions) * float64(barWidth)))
		empty := barWidth - filled

		bar := lipgloss.NewStyle().Foreground(t.Brand).Render(strings.Repeat("█", filled)) +
			lipgloss.NewStyle().Foreground(t.Border).Render(strings.Repeat("░", empty))

		label := lipgloss.NewStyle().Foreground(t.TextMuted).Render(fmt.Sprintf("  %-6s  ", d.label))
		count := lipgloss.NewStyle().Foreground(t.TextSecondary).Render(
			fmt.Sprintf("  %d sessions", d.sessions))

		sb.WriteString(label + bar + count + "\n")
	}
	return sb.String()
}

// renderModelDistribution renders a distribution bar per model.
func (m *MetricsModel) renderModelDistribution() string {
	t := m.theme

	barWidth := m.width - 50
	if barWidth < 10 {
		barWidth = 10
	}
	if barWidth > 30 {
		barWidth = 30
	}

	var sb strings.Builder
	for _, ms := range m.modelStats {
		filled := int(math.Round(ms.pct / 100 * float64(barWidth)))
		empty := barWidth - filled

		bar := lipgloss.NewStyle().Foreground(t.Brand).Render(strings.Repeat("█", filled)) +
			lipgloss.NewStyle().Foreground(t.Border).Render(strings.Repeat("░", empty))

		name := ms.modelName
		if len(name) > 22 {
			name = name[:19] + "..."
		}

		line := fmt.Sprintf("  %-22s  %s  %4.0f%%  ·  %d sessions",
			name, bar, ms.pct, ms.sessions)
		sb.WriteString(lipgloss.NewStyle().Foreground(t.TextSecondary).Render(line))
		sb.WriteString("\n")
	}
	return sb.String()
}

// formatMetricLarge formats a large number as "1.2M", "45.2K", etc.
func formatMetricLarge(n int) string {
	switch {
	case n >= 1_000_000:
		return fmt.Sprintf("%.1fM", float64(n)/1_000_000)
	case n >= 1_000:
		return fmt.Sprintf("%.1fK", float64(n)/1_000)
	default:
		return fmt.Sprintf("%d", n)
	}
}
