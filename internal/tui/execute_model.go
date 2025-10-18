package tui

import (
	"fmt"
	"strings"
	"time"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/charmbracelet/bubbles/viewport"
	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/tui/theme"
	"github.com/eshanized/M31A/internal/types"
)

// ExecuteModel displays real-time task execution progress.
type ExecuteModel struct {
	tasks       []types.Task
	theme       theme.Theme
	sessionID   string
	width       int
	height      int
	viewport    viewport.Model
	toolCalls   int
	totalTokens int
	totalCost   float64
	paused      bool
	startedAt   time.Time
}

// NewExecuteModel creates an ExecuteModel.
func NewExecuteModel(tasks []types.Task, t theme.Theme, w, h int) *ExecuteModel {
	em := &ExecuteModel{
		tasks:     tasks,
		theme:     t,
		width:     w,
		height:    h,
		startedAt: time.Now(),
	}
	em.initViewport()
	return em
}

func (em *ExecuteModel) initViewport() {
	h := em.height - 7
	if h < 5 {
		h = 5
	}
	em.viewport = viewport.New(em.width, h)
	em.refreshContent()
}

func (em *ExecuteModel) refreshContent() {
	em.viewport.SetContent(em.renderTasks())
	em.viewport.GotoBottom()
}

// UpdateTaskStatus updates a task's status by ID.
func (em *ExecuteModel) UpdateTaskStatus(taskID int, status types.TaskStatus) {
	for i := range em.tasks {
		if em.tasks[i].ID == taskID {
			em.tasks[i].Status = status
			break
		}
	}
	em.refreshContent()
}

// Update handles execute screen key events.
func (em *ExecuteModel) Update(msg tea.Msg) (*ExecuteModel, tea.Cmd) {
	switch msg := msg.(type) {
	case tea.KeyMsg:
		switch msg.String() {
		case "j", "down":
			em.viewport.LineDown(1)
		case "k", "up":
			em.viewport.LineUp(1)
		case "p":
			em.paused = !em.paused
			return em, func() tea.Msg {
				return ExecutePauseMsg{Paused: em.paused}
			}
		case "esc", "q":
			return em, func() tea.Msg {
				return AppMsg{Screen: ScreenREPL}
			}
		}
	}
	return em, nil
}

// View renders the execute progress screen.
func (em *ExecuteModel) View() string {
	t := em.theme
	w := em.width

	// Header
	elapsed := time.Since(em.startedAt)
	elapsedStr := fmt.Sprintf("%ds", int(elapsed.Seconds()))
	done, total, failed := em.countTasks()

	title := lipgloss.NewStyle().Foreground(t.Brand).Bold(true).PaddingLeft(2).
		Render(fmt.Sprintf("⚡ Execute — %d/%d tasks", done, total))

	if failed > 0 {
		title += " " + lipgloss.NewStyle().Foreground(t.Error).Render(fmt.Sprintf("(%d failed)", failed))
	}

	meta := lipgloss.NewStyle().Foreground(t.TextMuted).
		Render(fmt.Sprintf("  %s elapsed", elapsedStr))
	if em.toolCalls > 0 {
		meta += lipgloss.NewStyle().Foreground(t.TextMuted).
			Render(fmt.Sprintf("  %d tool calls", em.toolCalls))
	}

	divider := lipgloss.NewStyle().Foreground(t.TextMuted).Render(strings.Repeat("─", w))

	// Pause indicator
	pauseHint := ""
	if em.paused {
		pauseHint = lipgloss.NewStyle().Foreground(t.Warning).Bold(true).Render("  ⏸ PAUSED")
	}

	footer := lipgloss.NewStyle().Foreground(t.TextMuted).PaddingLeft(2).
		Render("p pause/resume  j/k scroll  q back")

	return lipgloss.JoinVertical(lipgloss.Left,
		title+meta+pauseHint,
		divider,
		em.viewport.View(),
		divider,
		footer,
	)
}

func (em *ExecuteModel) renderTasks() string {
	t := em.theme
	var lines []string
	for i, task := range em.tasks {
		icon, color := taskStatusIcon(task.Status, t)
		num := lipgloss.NewStyle().Foreground(t.TextMuted).Render(fmt.Sprintf("%3d.", i+1))
		statusIcon := lipgloss.NewStyle().Foreground(color).Render(icon)
		action := lipgloss.NewStyle().Foreground(t.Text).Render(task.Action)
		lines = append(lines, fmt.Sprintf("  %s %s %s", num, statusIcon, action))
	}
	return strings.Join(lines, "\n")
}

func (em *ExecuteModel) countTasks() (done, total, failed int) {
	total = len(em.tasks)
	for _, t := range em.tasks {
		switch t.Status {
		case types.StatusDone:
			done++
		case types.StatusFailed:
			failed++
		}
	}
	return
}

func taskStatusIcon(status types.TaskStatus, t theme.Theme) (string, lipgloss.Color) {
	switch status {
	case types.StatusDone:
		return "✓", t.Success
	case types.StatusFailed:
		return "✗", t.Error
	case types.StatusRunning:
		return "▸", t.Brand
	case types.StatusSkipped:
		return "─", t.TextMuted
	default:
		return "○", t.TextMuted
	}
}
