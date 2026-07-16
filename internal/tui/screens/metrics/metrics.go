package metrics


import (
	"github.com/eshanized/M31A/internal/tui/tuitypes"
	"fmt"
	"strings"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/session"
	"github.com/eshanized/M31A/internal/tui/components"
	"github.com/eshanized/M31A/internal/tui/theme"
)

// metricsStats holds aggregate stats loaded from session data.
type metricsStats struct {
	SessionCount   int
	TotalMessages  int
	TotalTokens    int
	AvgTokens      int
	ActiveSessions int
	Providers      map[string]int // provider → session count
	Models         map[string]int // model → session count
	Phases         map[string]int // phase → session count
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

// metricsLoadedMsg carries the result of an async LoadStats call.
type metricsLoadedMsg struct {
	stats  metricsStats
	loaded bool
}

// LoadStatsCmd returns a tea.Cmd that loads metrics asynchronously (TU-11 fix).
func (mm *MetricsModel) LoadStatsCmd(sessionManager *session.Manager) tea.Cmd {
	return func() tea.Msg {
		sessions, err := sessionManager.ListSessions()
		if err != nil || len(sessions) == 0 {
			return metricsLoadedMsg{
				stats: metricsStats{
					Providers: make(map[string]int),
					Models:    make(map[string]int),
					Phases:    make(map[string]int),
				},
				loaded: true,
			}
		}

		s := metricsStats{
			SessionCount: len(sessions),
			Providers:    make(map[string]int),
			Models:       make(map[string]int),
			Phases:       make(map[string]int),
		}

		for _, info := range sessions {
			s.TotalMessages += info.MessageCount

			if info.Provider != "" {
				s.Providers[info.Provider]++
			}
			if info.Model != "" {
				s.Models[info.Model]++
			}
			if info.WorkflowPhase != "" {
				s.Phases[string(info.WorkflowPhase)]++
			}
			if info.WorkflowPhase != "" && info.WorkflowPhase != "idle" {
				s.ActiveSessions++
			}

			sess, err := sessionManager.LoadSession(info.ID)
			if err != nil {
				continue
			}
			for _, msg := range sess.Messages {
				if msg.Usage != nil {
					s.TotalTokens += msg.Usage.TotalTokens
				}
			}
		}

		if s.SessionCount > 0 {
			s.AvgTokens = s.TotalTokens / s.SessionCount
		}

		return metricsLoadedMsg{stats: s, loaded: true}
	}
}

// ApplyStats applies loaded stats to the model (called from Update on metricsLoadedMsg).
func (mm *MetricsModel) ApplyStats(s metricsStats) {
	mm.stats = s
	mm.loaded = true
}

// Init implements tuitypes.Screenable.
func (mm *MetricsModel) Init() tea.Cmd { return nil }

// Update implements tuitypes.Screenable.
func (mm *MetricsModel) Update(msg tea.Msg) (tea.Model, tea.Cmd) {
	switch msg := msg.(type) {
	case tea.WindowSizeMsg:
		mm.width = msg.Width
		mm.height = msg.Height
	case tea.KeyMsg:
		switch msg.String() {
		case "esc", "q":
			return mm, func() tea.Msg {
				return tuitypes.PopScreenMsg{}
			}
		}
	}
	return mm, nil
}

// View implements tea.Model — content only, chrome handled by PageLayout.
func (mm *MetricsModel) View() string {
	t := mm.theme
	w := mm.width
	if w <= 0 {
		w = 80 // only when uninitialized
	}

	if !mm.loaded {
		return components.LoadingIndicator{Label: "Loading metrics", Theme: t}.Render()
	}

	s := mm.stats

	// Row 1: Core counts
	cardW := (w - 6) / 3
	if cardW < 14 {
		cardW = 14
	}

	row1 := components.MetricRow([]components.MetricCard{
		{
			Value: fmt.Sprintf("%d", s.SessionCount),
			Label: "Sessions",
			Width: cardW,
			Theme: t,
		},
		{
			Value: components.FormatMetric(s.TotalMessages),
			Label: "Messages",
			Width: cardW,
			Theme: t,
		},
		{
			Value: fmt.Sprintf("%d", s.ActiveSessions),
			Label: "With Workflow",
			Width: cardW,
			Theme: t,
		},
	}, w)

	// Row 2: Token stats
	row2 := components.MetricRow([]components.MetricCard{
		{
			Value: components.FormatMetric(s.TotalTokens),
			Label: "Total Tokens",
			Width: cardW,
			Theme: t,
		},
		{
			Value: components.FormatMetric(s.AvgTokens),
			Label: "Avg Tokens / Session",
			Width: cardW,
			Theme: t,
		},
		{
			Value: fmt.Sprintf("%d", len(s.Providers)),
			Label: "Providers Used",
			Width: cardW,
			Theme: t,
		},
	}, w)

	// Row 3: Breakdowns
	var breakdowns []string

	if len(s.Providers) > 0 {
		var parts []string
		for name, count := range s.Providers {
			parts = append(parts, fmt.Sprintf("%s: %d", name, count))
		}
		breakdowns = append(breakdowns,
			lipgloss.NewStyle().Foreground(t.TextSecondary).PaddingLeft(2).
				Render("Providers: "+strings.Join(parts, "  ")))
	}

	if len(s.Models) > 0 {
		var parts []string
		for name, count := range s.Models {
			display := name
			if len(display) > 30 {
				display = display[:27] + "..."
			}
			parts = append(parts, fmt.Sprintf("%s: %d", display, count))
		}
		breakdowns = append(breakdowns,
			lipgloss.NewStyle().Foreground(t.TextSecondary).PaddingLeft(2).
				Render("Models: "+strings.Join(parts, "  ")))
	}

	if len(s.Phases) > 0 {
		var parts []string
		for phase, count := range s.Phases {
			parts = append(parts, fmt.Sprintf("%s: %d", phase, count))
		}
		breakdowns = append(breakdowns,
			lipgloss.NewStyle().Foreground(t.TextSecondary).PaddingLeft(2).
				Render("Phases: "+strings.Join(parts, "  ")))
	}

	var content []string
	content = append(content, "", "  "+row1, "", "  "+row2)
	if len(breakdowns) > 0 {
		content = append(content, "")
		content = append(content, breakdowns...)
	}

	return lipgloss.JoinVertical(lipgloss.Left, content...)
}
