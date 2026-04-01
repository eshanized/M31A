package tui

import (
	"fmt"
	"math"
	"strings"
	"time"

	"github.com/charmbracelet/bubbles/viewport"
	tea "github.com/charmbracelet/bubbletea"
	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/tui/components"
	"github.com/eshanized/M31A/internal/tui/theme"
	"github.com/eshanized/M31A/internal/types"
)

// ExecuteModel displays real-time task execution progress.
type ExecuteModel struct {
	tasks     []types.Task
	theme     theme.Theme
	sessionID string
	width     int
	height    int
	viewport  viewport.Model
	paused    bool
	startedAt time.Time

	// Live output tracking for the currently running task
	liveOutput  []string
	currentTask int // index of the currently running task (-1 if none)

	// Animated spinner for task progress
	spinner components.Spinner

	// Animated progress bar
	animatedProg components.AnimatedProgressBar

	// Previous done count for detecting changes
	prevDone int
}

// NewExecuteModel creates an ExecuteModel.
func NewExecuteModel(tasks []types.Task, t theme.Theme, w, h int) *ExecuteModel {
	em := &ExecuteModel{
		tasks:       tasks,
		theme:       t,
		width:       w,
		height:      h,
		startedAt:   time.Now(),
		currentTask: -1,
		spinner:     components.NewSpinner(),
		animatedProg: components.AnimatedProgressBar{
			Animated: components.AnimatedProgress{
				Total: len(tasks),
			},
			Width:   10,
			ShowPct: false,
			Theme:   t,
		},
		prevDone: 0,
	}
	em.initViewport()
	return em
}

func (em *ExecuteModel) initViewport() {
	h := em.height - 6
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
	oldDone, _, _ := em.countTasks()

	for i := range em.tasks {
		if em.tasks[i].ID == taskID {
			em.tasks[i].Status = status
			break
		}
	}

	newDone, newTotal, newFailed := em.countTasks()

	// Trigger progress animation on status changes
	if newDone > oldDone {
		em.animatedProg.Animated.UpdateProgress(em.prevDone, newDone, newTotal)
		em.prevDone = newDone

		// Flash green on full completion, red on failure
		if newDone == newTotal {
			em.animatedProg.Flash.StartFlash(em.theme.Success)
		} else if newFailed > 0 {
			em.animatedProg.Flash.StartFlash(em.theme.Error)
		}
	} else if newFailed > 0 {
		em.animatedProg.Flash.StartFlash(em.theme.Error)
	}

	em.refreshContent()
}

// SetCurrentTask marks which task is currently executing.
func (em *ExecuteModel) SetCurrentTask(taskIdx int) {
	em.currentTask = taskIdx
	em.liveOutput = nil
	em.refreshContent()
}

// AppendLiveOutput appends output lines for the currently running task.
func (em *ExecuteModel) AppendLiveOutput(lines []string) {
	em.liveOutput = append(em.liveOutput, lines...)
	// Keep only last 50 lines
	if len(em.liveOutput) > 50 {
		em.liveOutput = em.liveOutput[len(em.liveOutput)-50:]
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
	case TickMsg:
		if em.currentTask >= 0 {
			em.spinner.Next()
		}
		em.animatedProg.Animated.Tick()
		em.animatedProg.Flash.Tick()
		em.refreshContent()
	}
	return em, nil
}

// View renders the execute progress screen content.
// Header, footer, and chrome are handled by the unified PageLayout.
func (em *ExecuteModel) View() string {
	t := em.theme
	w := em.width

	// Progress info row (inline with task content)
	done, total, failed := em.countTasks()
	pct := 0
	if total > 0 {
		pct = int(math.Round(float64(done) / float64(total) * 100))
	}

	progressLine := lipgloss.NewStyle().Foreground(t.TextSecondary).
		Render(fmt.Sprintf("%d/%d tasks", done, total))

	if failed > 0 {
		progressLine += " " + lipgloss.NewStyle().Foreground(t.Error).
			Render(fmt.Sprintf("(%d failed)", failed))
	}

	progressLine += " " + lipgloss.NewStyle().Foreground(t.TextMuted).
		Render(fmt.Sprintf("%d%%", pct))

	elapsed := time.Since(em.startedAt)
	progressLine += " " + lipgloss.NewStyle().Foreground(t.TextMuted).
		Render(fmt.Sprintf("· %ds elapsed", int(elapsed.Seconds())))

	if em.paused {
		progressLine += "  " + lipgloss.NewStyle().Foreground(t.Warning).Bold(true).Render("PAUSED")
	}

	// Thin separator
	sep := lipgloss.NewStyle().Foreground(t.BorderSubtle).
		Render(strings.Repeat("─", w))

	// Key hints
	hints := lipgloss.NewStyle().Foreground(t.TextMuted).
		Render("j/k: scroll  p: pause  Esc: back")

	return lipgloss.JoinVertical(lipgloss.Left,
		progressLine,
		sep,
		em.viewport.View(),
		hints,
	)
}

func (em *ExecuteModel) renderTasks() string {
	t := em.theme
	var lines []string
	for i, task := range em.tasks {
		statusBadge := taskStatusBadge(task.Status, t)

		// Dim everything when paused
		var lineStyle lipgloss.Style
		if em.paused {
			lineStyle = lipgloss.NewStyle().Faint(true)
		} else {
			lineStyle = lipgloss.NewStyle()
		}

		spinner := ""
		if !em.paused && task.Status == types.StatusRunning {
			spinner = " " + renderTaskSpinner(em)
		}
		num := lineStyle.Foreground(t.TextMuted).Render(fmt.Sprintf("%3d.", i+1))
		action := lineStyle.Foreground(t.Text).Render(task.Action)
		lines = append(lines, fmt.Sprintf("  %s %s %s%s", num, statusBadge, action, spinner))

		// Show live output for the currently running task
		if !em.paused && em.currentTask >= 0 && em.tasks[i].ID == em.tasks[em.currentTask].ID &&
			task.Status == types.StatusRunning && len(em.liveOutput) > 0 {
			showLines := em.liveOutput
			if len(showLines) > 10 {
				showLines = showLines[len(showLines)-10:]
			}
			for _, l := range showLines {
				lines = append(lines, lipgloss.NewStyle().
					Foreground(t.TextMuted).
					PaddingLeft(6).
					Render(l))
			}
		}
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

func taskStatusBadge(status types.TaskStatus, t theme.Theme) string {
	var badgeType components.BadgeType
	var text string
	switch status {
	case types.StatusDone:
		badgeType = components.BadgeSuccess
		text = "✓ done"
	case types.StatusFailed:
		badgeType = components.BadgeError
		text = "✗ failed"
	case types.StatusRunning:
		badgeType = components.BadgeBrand
		text = "● running"
	case types.StatusSkipped:
		badgeType = components.BadgeNeutral
		text = "— skipped"
	default:
		badgeType = components.BadgeNeutral
		text = "○ pending"
	}
	return components.SimpleBadge{
		Text:    text,
		Type:    badgeType,
		Compact: true,
		Theme:   t,
	}.Render()
}
