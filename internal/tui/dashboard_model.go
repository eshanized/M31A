package tui

import (
	"fmt"
	"strings"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/tui/components"
	"github.com/eshanized/M31A/internal/tui/theme"
	"github.com/eshanized/M31A/internal/types"
)

// DashboardModel shows the workflow pipeline overview.
type DashboardModel struct {
	theme     theme.Theme
	phases    []string
	current   string
	completed map[string]bool
	goal      string
	modelName string
	provider  string
	cost      float64
	activity  []components.TimelineEntry
	width     int
	height    int
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

// SetTheme updates the theme.
func (dm *DashboardModel) SetTheme(t theme.Theme) { dm.theme = t }

// SetDimensions updates dimensions.
func (dm *DashboardModel) SetDimensions(w, h int) {
	dm.width = w
	dm.height = h
}

// Init implements tea.Model.
func (dm *DashboardModel) Init() tea.Cmd { return nil }

// Update implements tea.Model.
func (dm *DashboardModel) Update(msg tea.Msg) (tea.Model, tea.Cmd) {
	switch msg := msg.(type) {
	case tea.WindowSizeMsg:
		dm.SetDimensions(msg.Width, msg.Height)
	case tea.KeyMsg:
		switch msg.String() {
		case "esc", "q":
			return dm, func() tea.Msg { return PopScreenMsg{} }
		case "enter":
			// Navigate to the current workflow phase screen
			var screen Screen
			switch types.WorkflowPhase(dm.current) {
			case types.PhasePlan:
				screen = ScreenPlan
			case types.PhaseExecute:
				screen = ScreenExecute
			case types.PhaseVerify:
				screen = ScreenVerify
			case types.PhaseShip:
				screen = ScreenShip
			default:
				return dm, nil
			}
			return dm, func() tea.Msg {
				return AppMsg{Screen: screen}
			}
		}
	}
	return dm, nil
}

// View implements tea.Model.
func (dm *DashboardModel) View() string {
	t := dm.theme
	w := dm.width
	if w < 30 {
		w = 80
	}

	phaseBar := components.WorkflowPhaseBar{
		Phases:    dm.phases,
		Current:   dm.current,
		Completed: dm.completed,
		Theme:     t,
		Width:     w,
	}.View()

	var info []string
	if dm.goal != "" {
		info = append(info, fmt.Sprintf("  Goal:     %s", dm.goal))
	}
	if dm.modelName != "" {
		info = append(info, fmt.Sprintf("  Model:    %s", dm.modelName))
	}
	if dm.provider != "" {
		info = append(info, fmt.Sprintf("  Provider: %s", dm.provider))
	}
	if dm.current != "" {
		info = append(info, fmt.Sprintf("  Phase:    %s", dm.current))
	}
	if dm.cost > 0 {
		info = append(info, fmt.Sprintf("  Cost:     $%.4f", dm.cost))
	}

	timeline := ""
	if len(dm.activity) > 0 {
		timeline = components.TimelineView{
			Entries: dm.activity,
			Theme:   t,
			Width:   w,
			Height:  dm.height - 12,
		}.View()
	}

	footer := lipgloss.NewStyle().Foreground(t.TextMuted).PaddingLeft(2).
		Render("[enter] Go to current phase   [esc] Back")

	parts := []string{"", phaseBar, ""}
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
