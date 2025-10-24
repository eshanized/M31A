package tui

import (
	"fmt"
	"strings"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/tui/components"
	"github.com/eshanized/M31A/internal/tui/theme"
	"github.com/eshanized/M31A/pkg/session"
)

// metricsStats holds aggregate stats loaded from session data.
type metricsStats struct {
	SessionCount  int
	TotalTokens   int
	TotalCost     float64
	AvgCost       float64
	TotalMessages int
}

// MetricsModel displays session analytics.
type MetricsModel struct {
	theme  theme.Theme
	stats  metricsStats
	loaded bool
	width  int
	height int
}

// NewMetricsModel creates a MetricsModel.
func NewMetricsModel(t theme.Theme) *MetricsModel {
	return &MetricsModel{theme: t}
}

// SetTheme updates the theme.
func (mm *MetricsModel) SetTheme(t theme.Theme) {
	mm.theme = t
}

// SetDimensions updates the metrics model dimensions.
func (mm *MetricsModel) SetDimensions(w, h int) {
	mm.width = w
	mm.height = h
}

// LoadStats computes aggregate metrics from the session manager.
func (mm *MetricsModel) LoadStats(sessionManager *session.Manager) {
	sessions, err := sessionManager.ListSessions()
	if err != nil || len(sessions) == 0 {
		mm.stats = metricsStats{}
		mm.loaded = true
		return
	}

	s := metricsStats{SessionCount: len(sessions)}
	for _, info := range sessions {
		s.TotalMessages += info.MessageCount
	}
	if s.SessionCount > 0 {
		s.AvgCost = s.TotalCost / float64(s.SessionCount)
	}
	mm.stats = s
	mm.loaded = true
}

// Init implements tea.Model.
func (mm *MetricsModel) Init() tea.Cmd { return nil }

// Update implements tea.Model.
func (mm *MetricsModel) Update(msg tea.Msg) (tea.Model, tea.Cmd) {
	switch msg := msg.(type) {
	case tea.WindowSizeMsg:
		mm.width = msg.Width
		mm.height = msg.Height
	case tea.KeyMsg:
		switch msg.String() {
		case "esc", "q":
			return mm, func() tea.Msg {
				return AppMsg{Screen: ScreenREPL}
			}
		}
	}
	return mm, nil
}

// View implements tea.Model.
func (mm *MetricsModel) View() string {
	t := mm.theme
	w := mm.width
	if w < 30 {
		w = 80
	}

	title := lipgloss.NewStyle().Foreground(t.Brand).Bold(true).PaddingLeft(1).
		Render("  Session Metrics")
	divider := lipgloss.NewStyle().Foreground(t.Border).
		Render(strings.Repeat("─", w))

	if !mm.loaded {
		body := lipgloss.NewStyle().Foreground(t.TextMuted).PaddingLeft(2).
			Render("Loading metrics...")
		return lipgloss.JoinVertical(lipgloss.Left, title, divider, body)
	}

	cardW := (w - 6) / 3
	if cardW < 14 {
		cardW = 14
	}

	cards := components.MetricRow([]components.MetricCard{
		{
			Value: fmt.Sprintf("%d", mm.stats.SessionCount),
			Label: "Sessions",
			Width: cardW,
			Theme: t,
		},
		{
			Value: components.FormatMetric(mm.stats.TotalMessages),
			Label: "Total Messages",
			Width: cardW,
			Theme: t,
		},
		{
			Value: components.FormatCost(mm.stats.AvgCost),
			Label: "Avg Cost / Session",
			Width: cardW,
			Theme: t,
		},
	}, w)

	footer := lipgloss.NewStyle().Foreground(t.TextMuted).PaddingLeft(2).
		Render("esc back")

	return lipgloss.JoinVertical(lipgloss.Left,
		title, divider, "", "  "+cards, "", divider, footer)
}
