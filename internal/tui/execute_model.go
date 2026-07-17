package tui

import (
	"fmt"
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

	// Workflow engine for pause/resume/skip/cancel
	workflowEngine WorkflowEngine

	// Exit confirmation dialog state
	confirmExit       bool
	confirmExitChoice int // 0=pause, 1=cancel, 2=stay
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
			// Width is set proportionally in View(); initialized to a safe default
			Width:   max(10, w/5),
			ShowPct: false,
			Theme:   t,
		},
		prevDone: 0,
	}
	em.initViewport()
	return em
}

// SetWorkflowEngine sets the workflow engine for pause/resume/skip/cancel support.
func (em *ExecuteModel) SetWorkflowEngine(engine WorkflowEngine) {
	em.workflowEngine = engine
}

// Init implements Screenable.
func (em *ExecuteModel) Init() tea.Cmd { return nil }

// SetDimensions implements Screenable.
func (em *ExecuteModel) SetDimensions(w, h int) {
	em.width = w
	em.height = h
	em.initViewport()
}

// SetTheme implements Screenable.
func (em *ExecuteModel) SetTheme(t theme.Theme) {
	em.theme = t
	em.animatedProg.Theme = t
	em.refreshContent()
}

func (em *ExecuteModel) initViewport() {
	// executeViewChrome: progressLine(1) + separator(1) + hints(1) + margins(3)
	const executeViewChrome = 6
	h := em.height - executeViewChrome
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

// hasActiveTasks returns true if any tasks are running or pending (not all done/failed).
func (em *ExecuteModel) hasActiveTasks() bool {
	for _, t := range em.tasks {
		if t.Status == types.StatusRunning || t.Status == types.StatusPending || t.Status == "" {
			return true
		}
	}
	return false
}

// Update handles execute screen key events.
func (em *ExecuteModel) Update(msg tea.Msg) (tea.Model, tea.Cmd) {
	switch msg := msg.(type) {
	case tea.MouseMsg:
		if msg.Action == tea.MouseActionPress {
			switch msg.Button {
			case tea.MouseButtonWheelUp:
				em.viewport.LineUp(3)
			case tea.MouseButtonWheelDown:
				em.viewport.LineDown(3)
			}
		}
	case tea.KeyMsg:
		// Handle exit confirmation dialog
		if em.confirmExit {
			switch msg.String() {
			case "up", "k":
				if em.confirmExitChoice > 0 {
					em.confirmExitChoice--
				}
				return em, nil
			case "down", "j":
				if em.confirmExitChoice < 2 {
					em.confirmExitChoice++
				}
				return em, nil
			case "enter", " ":
				switch em.confirmExitChoice {
				case 0: // Pause
					em.paused = true
					em.confirmExit = false
					return em, func() tea.Msg {
						return ExecutePauseMsg{Paused: true}
					}
				case 1: // Cancel
					if em.workflowEngine != nil {
						em.workflowEngine.CancelGroup()
					}
					em.confirmExit = false
					return em, func() tea.Msg {
						return PopScreenMsg{}
					}
				case 2: // Stay
					em.confirmExit = false
					return em, nil
				}
			case "esc", "q", "n":
				em.confirmExit = false
				return em, nil
			}
			return em, nil
		}

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
		case "s":
			// Skip current task (only when paused)
			if em.paused && em.currentTask >= 0 && em.currentTask < len(em.tasks) {
				taskID := em.tasks[em.currentTask].ID
				if em.workflowEngine != nil {
					em.workflowEngine.SkipCurrentTask(taskID)
				}
			}
		case "c":
			// Cancel current task (only when paused)
			if em.paused && em.currentTask >= 0 && em.currentTask < len(em.tasks) {
				taskID := em.tasks[em.currentTask].ID
				if em.workflowEngine != nil {
					em.workflowEngine.CancelCurrentTask(taskID)
				}
			}
		case "x":
			// Cancel entire group (only when paused)
			if em.paused && em.workflowEngine != nil {
				em.workflowEngine.CancelGroup()
			}
		case "esc", "q":
			// Show confirmation if execution is active (not paused, tasks running)
			if !em.paused && em.hasActiveTasks() {
				em.confirmExit = true
				em.confirmExitChoice = 0
				return em, nil
			}
			return em, func() tea.Msg {
				return PopScreenMsg{}
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

	done, total, failed := em.countTasks()

	// ── Current activity label (replaces stats line) ─────────────────────
	activityLabel := em.currentActivityLabel()

	// ── Single subtle progress line ───────────────────────────────────────
	var progressParts []string
	if failed > 0 {
		progressParts = append(progressParts, lipgloss.NewStyle().Foreground(t.Error).
			Render(fmt.Sprintf("%d failed", failed)))
	}
	if done > 0 {
		progressParts = append(progressParts, lipgloss.NewStyle().Foreground(t.TextMuted).
			Render(fmt.Sprintf("%d/%d", done, total)))
	}
	progressLine := strings.Join(progressParts, " ")

	if em.paused {
		progressLine += "  " + lipgloss.NewStyle().Foreground(t.Warning).Bold(true).Render("PAUSED")
	}

	// ── Pause controls (shown when paused) ──────────────────────────────
	var pauseControls string
	if em.paused {
		controls := []string{
			lipgloss.NewStyle().Foreground(t.TextMuted).Render("[s] Skip task"),
			lipgloss.NewStyle().Foreground(t.TextMuted).Render("[c] Cancel task"),
			lipgloss.NewStyle().Foreground(t.TextMuted).Render("[x] Cancel group"),
			lipgloss.NewStyle().Foreground(t.TextMuted).Render("[p] Resume"),
		}
		pauseControls = strings.Join(controls, "  ")
	}

	// ── Assemble ──────────────────────────────────────────────────────────
	parts := []string{
		activityLabel,
		progressLine,
		em.viewport.View(),
	}
	if pauseControls != "" {
		parts = append(parts, pauseControls)
	}

	// ── Exit confirmation dialog ──────────────────────────────────────
	if em.confirmExit {
		options := []string{"Pause execution", "Cancel execution", "Stay on screen"}
		var dialogLines []string
		dialogLines = append(dialogLines, "")
		dialogLines = append(dialogLines, lipgloss.NewStyle().Foreground(t.Warning).Bold(true).Render("  Execution is running. What would you like to do?"))
		dialogLines = append(dialogLines, "")
		for i, opt := range options {
			cursor := "  "
			style := lipgloss.NewStyle().Foreground(t.TextSecondary)
			if i == em.confirmExitChoice {
				cursor = lipgloss.NewStyle().Foreground(t.Brand).Render("▸ ")
				style = lipgloss.NewStyle().Foreground(t.TextPrimary)
			}
			dialogLines = append(dialogLines, "    "+cursor+style.Render(opt))
		}
		dialogLines = append(dialogLines, "")
		dialogLines = append(dialogLines, lipgloss.NewStyle().Foreground(t.TextMuted).Render("    [↑↓] Navigate  [Enter] Select  [Esc] Cancel"))
		parts = append(parts, strings.Join(dialogLines, "\n"))
	}

	return lipgloss.JoinVertical(lipgloss.Left, parts...)
}

// currentActivityLabel returns a human-readable label of what's happening now.
func (em *ExecuteModel) currentActivityLabel() string {
	t := em.theme

	if em.currentTask >= 0 && em.currentTask < len(em.tasks) {
		task := em.tasks[em.currentTask]
		frame := em.spinner.Peek()
		return lipgloss.NewStyle().Foreground(t.Brand).Render(frame) + " " +
			lipgloss.NewStyle().Foreground(t.Text).Render(task.Action)
	}

	// Check if all done
	done, total, _ := em.countTasks()
	if done == total && total > 0 {
		return lipgloss.NewStyle().Foreground(t.Success).Render("✓ All tasks complete")
	}

	return lipgloss.NewStyle().Foreground(t.TextMuted).Render("Preparing tasks…")
}

func (em *ExecuteModel) renderTasks() string {
	t := em.theme
	var lines []string

	for i, task := range em.tasks {
		// Dim everything when paused
		var lineStyle lipgloss.Style
		if em.paused {
			lineStyle = lipgloss.NewStyle().Faint(true)
		} else {
			lineStyle = lipgloss.NewStyle()
		}

		// ── Completed tasks: collapse to single line with ✓ ───────────────
		if task.Status == types.StatusDone {
			num := lineStyle.Foreground(t.TextMuted).Render(fmt.Sprintf("%3d.", i+1))
			checkmark := lipgloss.NewStyle().Foreground(t.Success).Render("✓")
			action := lineStyle.Foreground(t.TextSecondary).Render(task.Action)
			lines = append(lines, fmt.Sprintf("  %s %s %s", num, checkmark, action))
			continue
		}

		// ── Failed tasks: show error inline (red, expanded) ───────────────
		if task.Status == types.StatusFailed {
			num := lineStyle.Foreground(t.TextMuted).Render(fmt.Sprintf("%3d.", i+1))
			crossmark := lipgloss.NewStyle().Foreground(t.Error).Render("✗")
			action := lipgloss.NewStyle().Foreground(t.Error).Render(task.Action)
			lines = append(lines, fmt.Sprintf("  %s %s %s", num, crossmark, action))
			continue
		}

		// ── Pending tasks: dimmed with ○ badge ──────────────────────────
		if task.Status == types.StatusPending || task.Status == "" {
			num := lipgloss.NewStyle().Foreground(t.TextMuted).Faint(true).Render(fmt.Sprintf("%3d.", i+1))
			action := lipgloss.NewStyle().Faint(true).Render(task.Action)
			lines = append(lines, fmt.Sprintf("  %s ○ %s", num, action))
			continue
		}

		// ── Running task: show with spinner ───────────────────────────────
		statusBadge := taskStatusBadge(task.Status, t)
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
			// Cap live output to 1/3 of viewport height, min 3
			maxLines := em.viewport.Height / 3
			if maxLines < 3 {
				maxLines = 3
			}
			if len(showLines) > maxLines {
				showLines = showLines[len(showLines)-maxLines:]
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
