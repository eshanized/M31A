package dashboard

import (
	"fmt"
	"github.com/eshanized/M31A/internal/ui/tui/tuitypes"
	"strings"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/core/types"
	"github.com/eshanized/M31A/internal/ui/tui/components"
	"github.com/eshanized/M31A/internal/ui/tui/theme"
)

// DashboardModel shows the workflow pipeline overview.
type DashboardModel struct {
	theme       theme.Theme
	phases      []string
	current     string
	completed   map[string]bool
	goal        string
	modelName   string
	provider    string
	cost        float64
	totalTokens int
	activity    []components.TimelineEntry
	width       int
	height      int
}

// NewDashboardModel creates a DashboardModel.
func NewDashboardModel(t theme.Theme, w, h int) *DashboardModel {
	phases := []string{
		string(types.PhaseInitialize),
		string(types.PhaseDiscuss),
		string(types.PhasePlan),
		string(types.PhaseExecute),
		string(types.PhaseVerify),
		string(types.PhaseShip),
	}
	return &DashboardModel{
		theme:     t,
		phases:    phases,
		completed: make(map[string]bool),
		width:     w,
		height:    h,
	}
}

// SetWorkflowState updates the dashboard with current workflow state.
func (dm *DashboardModel) SetWorkflowState(phase types.WorkflowPhase, goal, model, provider string) {
	dm.current = string(phase)
	dm.goal = goal
	dm.modelName = model
	dm.provider = provider

	dm.completed = make(map[string]bool)
	reached := false
	for _, p := range dm.phases {
		if p == dm.current {
			reached = true
		}
		if !reached && p != dm.current {
			dm.completed[p] = true
		}
	}
}

// SetTokenInfo updates token count and cost for metric display.
func (dm *DashboardModel) SetTokenInfo(tokens int, cost float64) {
	dm.totalTokens = tokens
	dm.cost = cost
}

// SetTheme updates the theme.
func (dm *DashboardModel) SetTheme(t theme.Theme) { dm.theme = t }

// SetDimensions updates dimensions.
func (dm *DashboardModel) SetDimensions(w, h int) {
	dm.width = w
	dm.height = h
}

// Init implements tea.Model.
func (dm *DashboardModel) Init() tea.Cmd { return nil }

// Update implements tuitypes.Screenable.
func (dm *DashboardModel) Update(msg tea.Msg) (tea.Model, tea.Cmd) {
	switch msg := msg.(type) {
	case tea.WindowSizeMsg:
		dm.SetDimensions(msg.Width, msg.Height)
	case tea.KeyMsg:
		switch msg.String() {
		case "esc", "q":
			return dm, func() tea.Msg { return tuitypes.PopScreenMsg{} }
		case "enter":
			// Navigate to the current workflow phase screen
			var screen tuitypes.Screen
			switch types.WorkflowPhase(dm.current) {
			case types.PhaseInitialize:
				screen = tuitypes.ScreenREPL
			case types.PhaseDiscuss:
				screen = tuitypes.ScreenDiscuss
			case types.PhasePlan:
				screen = tuitypes.ScreenPlan
			case types.PhaseExecute:
				screen = tuitypes.ScreenExecute
			case types.PhaseVerify:
				screen = tuitypes.ScreenVerify
			case types.PhaseRuntime:
				screen = tuitypes.ScreenRuntimeCheck
			case types.PhaseShip:
				screen = tuitypes.ScreenShip
			default:
				return dm, nil
			}
			return dm, func() tea.Msg {
				return tuitypes.AppMsg{Screen: screen}
			}
		}
	}
	return dm, nil
}

// View implements tea.Model.
func (dm *DashboardModel) View() string {
	t := dm.theme
	w := dm.width
	if w <= 0 {
		w = 80 // only when uninitialized
	}

	phaseBar := components.WorkflowPhaseBar{
		Phases:    dm.phases,
		Current:   dm.current,
		Completed: dm.completed,
		Theme:     t,
		Width:     w,
	}.View()

	// Metric cards row
	var metrics []components.MetricCard
	if dm.totalTokens > 0 {
		metrics = append(metrics, components.MetricCard{
			Value: components.FormatMetric(dm.totalTokens),
			Label: "Tokens",
			Theme: t,
		})
	}
	if dm.cost > 0 {
		metrics = append(metrics, components.MetricCard{
			Value: components.FormatCost(dm.cost),
			Label: "Cost",
			Theme: t,
		})
	}
	// Calculate progress percentage
	completedCount := 0
	for _, done := range dm.completed {
		if done {
			completedCount++
		}
	}
	if dm.current != "" {
		progressPct := fmt.Sprintf("%d%%", int(float64(completedCount)/float64(len(dm.phases))*100))
		metrics = append(metrics, components.MetricCard{
			Value: progressPct,
			Label: "Complete",
			Theme: t,
		})
	}
	if dm.modelName != "" {
		metrics = append(metrics, components.MetricCard{
			Value: dm.modelName,
			Label: "Model",
			Theme: t,
		})
	}

	metricRow := ""
	if len(metrics) > 0 {
		metricRow = components.MetricRow(metrics, w-4)
	}

	// Info section using KeyValueGrid style
	var info []string
	if dm.goal != "" {
		goalDisplay := dm.goal
		if len(goalDisplay) > w-16 {
			goalDisplay = goalDisplay[:w-19] + "..."
		}
		info = append(info, lipgloss.NewStyle().Foreground(t.TextSecondary).Render("  Goal:     ")+
			lipgloss.NewStyle().Foreground(t.TextPrimary).Render(goalDisplay))
	}
	if dm.provider != "" {
		info = append(info, lipgloss.NewStyle().Foreground(t.TextSecondary).Render("  Provider: ")+
			lipgloss.NewStyle().Foreground(t.Brand).Render(dm.provider))
	}
	if dm.current != "" {
		info = append(info, lipgloss.NewStyle().Foreground(t.TextSecondary).Render("  Phase:    ")+
			lipgloss.NewStyle().Foreground(t.Brand).Bold(true).Render(dm.current))
	}

	timeline := ""
	if len(dm.activity) > 0 {
		timeline = components.TimelineView{
			Entries: dm.activity,
			Theme:   t,
			Width:   w,
			Height:  dm.height - 16,
		}.View()
	}

	footer := lipgloss.NewStyle().Foreground(t.TextMuted).PaddingLeft(2).
		Render("[enter] Go to current phase   [esc] Back")

	parts := []string{"", phaseBar, ""}
	if metricRow != "" {
		parts = append(parts, "  "+metricRow, "")
	}
	if len(info) > 0 {
		parts = append(parts, strings.Join(info, "\n"), "")
	}
	if timeline != "" {
		parts = append(parts, lipgloss.NewStyle().Foreground(t.TextSecondary).Bold(true).PaddingLeft(2).
			Render("Recent Activity:"), timeline, "")
	}
	parts = append(parts, footer)

	return lipgloss.JoinVertical(lipgloss.Left, parts...)
}
