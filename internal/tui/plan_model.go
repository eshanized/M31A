package tui

import (
	"github.com/charmbracelet/bubbles/spinner"
	tea "github.com/charmbracelet/bubbletea"
	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/tui/theme"
	"github.com/eshanized/M31A/internal/types"
	"github.com/eshanized/M31A/pkg/arbitrage"
)

// PlanModel displays the task plan for user review.
type PlanModel struct {
	theme       theme.Theme
	tasks       []types.Task
	selected    int
	width       int
	height      int
	modelID     string
	modelName   string
	provider    string
	estCost     float64
	estTime     string
	showDiff    bool
	showGraph   bool
	spinner     spinner.Model
	sessionID   string
}

// NewPlanModel creates a Plan screen model.
func NewPlanModel(tasks []types.Task, t theme.Theme, modelID string, modelName string, providerName string, estCost float64, estTime string, width, height int) *PlanModel {
	sp := spinner.New()
	sp.Spinner = spinner.Dot
	sp.Style = lipgloss.NewStyle().Foreground(t.Brand)
	return &PlanModel{
		theme:     t,
		tasks:     tasks,
		modelID:   modelID,
		modelName: modelName,
		provider:  providerName,
		estCost:   estCost,
		estTime:   estTime,
		width:     width,
		height:    height,
		spinner:   sp,
	}
}

func (m *PlanModel) Init() tea.Cmd {
	return m.spinner.Tick
}

// UpdateTasks replaces the task list and adjusts selection if out of bounds.
func (m *PlanModel) UpdateTasks(tasks []types.Task) {
	m.tasks = tasks
	if m.selected >= len(tasks) {
		m.selected = len(tasks) - 1
	}
	if m.selected < 0 {
		m.selected = 0
	}
}

// SetDimensions updates the plan model's width and height.
func (m *PlanModel) SetDimensions(width, height int) {
	m.width = width
	m.height = height
}

// ApplyArbitrage applies model cost optimization recommendations to the plan.
func (m *PlanModel) ApplyArbitrage(recs []arbitrage.ArbitrageRecommendation) {
	for _, rec := range recs {
		if rec.Savings > 0 {
			m.estCost -= rec.Savings
			if m.estCost < 0 {
				m.estCost = 0
			}
		}
	}
}

func (m *PlanModel) Update(msg tea.Msg) ([]tea.Cmd, *AppMsg) {
	switch msg := msg.(type) {
	case tea.WindowSizeMsg:
		m.width = msg.Width
		m.height = msg.Height
		return nil, nil

	case spinner.TickMsg:
		var cmd tea.Cmd
		m.spinner, cmd = m.spinner.Update(msg)
		return []tea.Cmd{cmd}, nil

	case tea.KeyMsg:
		switch msg.String() {
		case "up", "k":
			if m.selected > 0 {
				m.selected--
			}
		case "down", "j":
			if m.selected < len(m.tasks)-1 {
				m.selected++
			}
		case "a", "A":
			return nil, &AppMsg{Screen: ScreenExecute}
		case "r", "R":
			return nil, &AppMsg{Screen: ScreenREPL}
		case "esc":
			return nil, &AppMsg{Screen: ScreenREPL}
		case "d", "D":
			m.showDiff = !m.showDiff
			m.showGraph = false
		case "tab":
			m.showGraph = !m.showGraph
			m.showDiff = false
		case "o", "O":
			return []tea.Cmd{func() tea.Msg {
				return SlashCommandMsg{Command: "/optimize"}
			}}, nil
		}
	}
	return nil, nil
}

// actionBadge returns a styled action badge for a task.
func (m *PlanModel) actionBadge(action string) string {
	badgeStyle := lipgloss.NewStyle().Foreground(m.theme.TextSecondary)

	switch action {
	case "Create", "create":
		return badgeStyle.Foreground(m.theme.Success).Render("  ✦ NEW")
	case "Modify", "modify":
		return badgeStyle.Foreground(m.theme.Warning).Render("  ~ MOD")
	case "Delete", "delete":
		return badgeStyle.Foreground(m.theme.Error).Render("  ✗ DEL")
	default:
		return badgeStyle.Foreground(m.theme.Warning).Render("  ~ MOD")
	}
}
