package tui

import (
	"time"

	"github.com/charmbracelet/bubbles/spinner"
	tea "github.com/charmbracelet/bubbletea"
	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/tui/theme"
	"github.com/eshanized/M31A/internal/types"
)

// ExecuteModel displays task execution progress.
type ExecuteModel struct {
	theme         theme.Theme
	tasks         []types.Task
	current       int
	width         int
	height        int
	toolCard      string
	startedAt     time.Time
	taskStarted   time.Time
	totalTokens   int
	totalCost     float64
	toolCalls     int
	tasksCompleted int
	tasksFailed    int
	paused        bool
	spinner       spinner.Model
	allDone       bool
	transitioning bool
	transitionSec int
	sessionID     string
}

// recordTaskMetric records a task completion or failure metric.
func (m *ExecuteModel) recordTaskMetric(metric string) {
	switch metric {
	case "completed":
		m.tasksCompleted++
	case "failed":
		m.tasksFailed++
	}
}

// NewExecuteModel creates an Execute screen model.
func NewExecuteModel(tasks []types.Task, t theme.Theme, width, height int) *ExecuteModel {
	sp := spinner.New()
	sp.Spinner = spinner.Dot
	sp.Style = lipgloss.NewStyle().Foreground(t.Brand)
	return &ExecuteModel{
		theme:     t,
		tasks:     tasks,
		width:     width,
		height:    height,
		startedAt: time.Now(),
		spinner:   sp,
	}
}

func (m *ExecuteModel) Init() tea.Cmd {
	return m.spinner.Tick
}

// UpdateTaskStatus updates the status of a task by ID.
func (m *ExecuteModel) UpdateTaskStatus(taskID int, status types.TaskStatus) {
	for i := range m.tasks {
		if m.tasks[i].ID == taskID {
			m.tasks[i].Status = status
			break
		}
	}
}

func (m *ExecuteModel) Update(msg tea.Msg) ([]tea.Cmd, *AppMsg) {
	switch msg := msg.(type) {
	case tea.WindowSizeMsg:
		m.width = msg.Width
		m.height = msg.Height
		return nil, nil

	case spinner.TickMsg:
		var cmd tea.Cmd
		m.spinner, cmd = m.spinner.Update(msg)
		return []tea.Cmd{cmd}, nil

	case TransitionTickMsg:
		if m.transitioning {
			m.transitionSec--
			if m.transitionSec <= 0 {
				return nil, &AppMsg{Screen: ScreenVerify}
			}
			return []tea.Cmd{m.transitionTick()}, nil
		}

	case tea.KeyMsg:
		if m.transitioning {
			switch msg.String() {
			case "esc":
				m.transitioning = false
				m.transitionSec = 0
				return nil, nil
			}
			return nil, nil
		}

		switch msg.String() {
		case "up", "k":
			if m.current > 0 {
				m.current--
			}
		case "down", "j":
			if m.current < len(m.tasks)-1 {
				m.current++
			}
		case "s", "S":
			if m.current < len(m.tasks) && m.tasks[m.current].Status == types.StatusPending {
				m.tasks[m.current].Status = types.StatusSkipped
			}
		case "p", "P":
			m.paused = !m.paused
			return []tea.Cmd{func() tea.Msg {
				return ExecutePauseMsg{Paused: m.paused}
			}}, nil
		case "r", "R":
			m.paused = false
			return []tea.Cmd{func() tea.Msg {
				return ExecutePauseMsg{Paused: false}
			}}, nil
		case "enter":
			if m.allDone {
				return nil, &AppMsg{Screen: ScreenVerify}
			}
		}

		allDone := true
		for _, t := range m.tasks {
			if t.Status == types.StatusPending || t.Status == types.StatusRunning {
				allDone = false
				break
			}
		}
		if allDone && !m.allDone {
			m.allDone = true
			m.transitioning = true
			m.transitionSec = 3
			return []tea.Cmd{m.transitionTick()}, nil
		}
	}
	return nil, nil
}

// findRunningTask returns the currently running task, or nil.
func (m *ExecuteModel) findRunningTask() *types.Task {
	for i := range m.tasks {
		if m.tasks[i].Status == types.StatusRunning {
			return &m.tasks[i]
		}
	}
	return nil
}

// transitionTick returns a tea.Cmd that emits a TransitionTickMsg after 1 second.
func (m *ExecuteModel) transitionTick() tea.Cmd {
	return tea.Tick(time.Second, func(t time.Time) tea.Msg {
		return TransitionTickMsg{}
	})
}

// TransitionTickMsg is emitted every second during the transition countdown.
type TransitionTickMsg struct{}
