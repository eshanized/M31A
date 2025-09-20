package tui

import (
	"fmt"
	"math"
	"strings"
	"time"

	"github.com/charmbracelet/bubbles/spinner"
	tea "github.com/charmbracelet/bubbletea"
	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/tui/components"
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
// Must be called from Update(), never from View().
func (m *ExecuteModel) recordTaskMetric(metric string) {
	switch metric {
	case "completed":
		m.tasksCompleted++
	case "failed":
		m.tasksFailed++
	}
}

// NewExecuteModel creates an Execute screen model. width/height are
// required non-zero dimensions so the screen renders immediately
// on creation without waiting for a separate WindowSizeMsg (D-03 fix).
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
		// If transitioning, allow Esc to cancel
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
			// Skip current task
			if m.current < len(m.tasks) && m.tasks[m.current].Status == types.StatusPending {
				m.tasks[m.current].Status = types.StatusSkipped
			}
		case "p", "P":
			m.paused = !m.paused
		case "r", "R":
			m.paused = false
		case "enter":
			if m.allDone {
				return nil, &AppMsg{Screen: ScreenVerify}
			}
		}

		// Check if all done — start transition timer
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

func (m *ExecuteModel) View() string {
	if m.width == 0 {
		return m.spinner.View() + " Loading execute..."
	}

	var sb strings.Builder

	// Header: Mission Live
	sb.WriteString(m.renderHeader())
	sb.WriteString("\n")

	// Live metrics bar (dense single-line)
	sb.WriteString(m.renderLiveMetrics())
	sb.WriteString("\n")

	// Separator
	sb.WriteString(strings.Repeat("─", m.width-2))
	sb.WriteString("\n")

	// Progress bar with completion percentage
	sb.WriteString(m.renderProgressBar())
	sb.WriteString("\n")

	// Running task panel (double-border) if any task is running
	runningTask := m.findRunningTask()
	if runningTask != nil {
		sb.WriteString(m.renderRunningPanel(runningTask))
		sb.WriteString("\n")
	}

	// Compact task list
	sb.WriteString(m.renderCompactTaskList())

	// Tool card (shown below task list if no running panel)
	if m.toolCard != "" && runningTask == nil {
		sb.WriteString("\n")
		sb.WriteString(lipgloss.NewStyle().
			Border(lipgloss.RoundedBorder()).
			BorderForeground(m.theme.Border).
			Padding(0, 1).
			Render(m.toolCard))
	}

	// Keys
	sb.WriteString("\n")
	sb.WriteString(lipgloss.NewStyle().
		Foreground(m.theme.TextSecondary).
		Render("P=Pause  R=Resume  S=Skip  ↑↓=Navigate"))

	// Show completion summary when all tasks are done
	allDone := true
	hasFailures := false
	for _, task := range m.tasks {
		switch task.Status {
		case types.StatusDone:
			// OK
		case types.StatusFailed, types.StatusSkipped:
			hasFailures = true
		default:
			allDone = false
		}
	}
	if allDone {
		sb.WriteString("\n\n")
		if hasFailures {
			sb.WriteString(lipgloss.NewStyle().
				Foreground(m.theme.Warning).
				Render("⚠ Some tasks failed — review in Verify"))
		} else if m.transitioning {
			sb.WriteString(lipgloss.NewStyle().
				Foreground(m.theme.Success).
				Render(fmt.Sprintf("✓ All tasks complete — transitioning to Verify in %ds (Esc to cancel)", m.transitionSec)))
		} else {
			sb.WriteString(lipgloss.NewStyle().
				Foreground(m.theme.Success).
				Render("✓ All tasks complete"))
		}
	}

	return sb.String()
}

// renderHeader renders the Mission Live header bar.
func (m *ExecuteModel) renderHeader() string {
	headerStyle := lipgloss.NewStyle().
		Foreground(m.theme.Brand).
		Bold(true)

	statusBadge := "▶ Running"
	if m.paused {
		statusBadge = "⏸ Paused"
	}

	return headerStyle.Render(" Mission Live (Execute) ") + " " +
		lipgloss.NewStyle().Foreground(m.theme.TextSecondary).Render(statusBadge)
}

// renderLiveMetrics renders the dense live metrics bar.
//
//	Elapsed: 2m 14s  │  3/8 tasks  │  47 tool calls  │  $0.08  │  89K ctx
func (m *ExecuteModel) renderLiveMetrics() string {
	elapsed := time.Since(m.startedAt)
	elapsedSec := int(elapsed.Seconds())

	done := 0
	for _, t := range m.tasks {
		if t.Status == types.StatusDone {
			done++
		}
	}

	separator := lipgloss.NewStyle().Foreground(m.theme.Border).Render("│")

	metrics := []string{
		lipgloss.NewStyle().Foreground(m.theme.TextSecondary).Render(
			fmt.Sprintf("Elapsed: %s", components.FormatDuration(elapsedSec))),
		separator,
		lipgloss.NewStyle().Foreground(m.theme.TextSecondary).Render(
			fmt.Sprintf("%d/%d tasks", done, len(m.tasks))),
		separator,
		lipgloss.NewStyle().Foreground(m.theme.TextSecondary).Render(
			fmt.Sprintf("%d tool calls", m.toolCalls)),
		separator,
		lipgloss.NewStyle().Foreground(m.theme.TextSecondary).Render(
			fmt.Sprintf("$%.2f", m.totalCost)),
		separator,
		lipgloss.NewStyle().Foreground(m.theme.TextSecondary).Render(
			fmt.Sprintf("%s ctx", components.FormatMetric(m.totalTokens))),
	}

	return strings.Join(metrics, " ")
}

// renderProgressBar renders a progress bar with completion percentage.
//
//	████████████████░░░░░░░░░░░░░░░░░░░░░░░░  3/8 complete
func (m *ExecuteModel) renderProgressBar() string {
	done := 0
	for _, t := range m.tasks {
		if t.Status == types.StatusDone {
			done++
		}
	}
	total := len(m.tasks)
	if total == 0 {
		return ""
	}

	pct := float64(done) / float64(total)
	barWidth := m.width - 20
	if barWidth < 20 {
		barWidth = 20
	}

	filledWidth := int(math.Round(pct * float64(barWidth)))
	if filledWidth > barWidth {
		filledWidth = barWidth
	}
	emptyWidth := barWidth - filledWidth

	filled := lipgloss.NewStyle().Foreground(m.theme.Brand).Render(strings.Repeat("█", filledWidth))
	empty := lipgloss.NewStyle().Foreground(m.theme.Border).Render(strings.Repeat("░", emptyWidth))

	pctStr := lipgloss.NewStyle().Foreground(m.theme.TextSecondary).Render(
		fmt.Sprintf("  %d/%d complete", done, total))

	return filled + empty + pctStr
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

// renderRunningPanel renders the running task in a double-border panel.
func (m *ExecuteModel) renderRunningPanel(task *types.Task) string {
	doubleBorder := lipgloss.Border{
		Top:         "═",
		Bottom:      "═",
		Left:        "║",
		Right:       "║",
		TopLeft:     "╔",
		TopRight:    "╗",
		BottomLeft:  "╚",
		BottomRight: "╝",
	}

	headerStyle := lipgloss.NewStyle().
		Foreground(m.theme.Warning).
		Bold(true)

	content := lipgloss.JoinVertical(lipgloss.Left,
		headerStyle.Render(fmt.Sprintf("  ▶ RUNNING  Task #%d: %s", task.ID, task.Description)),
		"",
	)

	// Add tool card if present
	if m.toolCard != "" {
		content = lipgloss.JoinVertical(lipgloss.Left,
			content,
			"  "+m.toolCard,
		)
	}

	boxWidth := m.width - 2
	if boxWidth < 30 {
		boxWidth = 30
	}

	return lipgloss.NewStyle().
		Border(doubleBorder).
		BorderForeground(m.theme.Warning).
		Width(boxWidth).
		Padding(0, 1).
		Render(content)
}

// renderCompactTaskList renders a compact task list with status icons,
// elapsed time, and tool call count.
func (m *ExecuteModel) renderCompactTaskList() string {
	var sb strings.Builder

	for _, task := range m.tasks {
		var statusIcon string
		var iconStyle lipgloss.Style
		extra := ""

		switch task.Status {
		case types.StatusDone:
			statusIcon = "[✓]"
			iconStyle = lipgloss.NewStyle().Foreground(m.theme.Success)
		case types.StatusRunning:
			statusIcon = "[▶]"
			iconStyle = lipgloss.NewStyle().Foreground(m.theme.Brand).Bold(true)
		case types.StatusSkipped:
			statusIcon = "·"
			iconStyle = lipgloss.NewStyle().Foreground(m.theme.Warning)
			extra = lipgloss.NewStyle().Foreground(m.theme.TextMuted).Render("  ← skipped")
		case types.StatusFailed:
			statusIcon = "✗"
			iconStyle = lipgloss.NewStyle().Foreground(m.theme.Error)
			extra = lipgloss.NewStyle().Foreground(m.theme.TextMuted).Render("  ← failed")
		case types.StatusPending:
			statusIcon = "·"
			iconStyle = lipgloss.NewStyle().Foreground(m.theme.TextMuted)
			// Check if blocked
			var blockingDeps []int
			for _, dep := range task.Dependencies {
				for _, other := range m.tasks {
					if other.ID == dep && (other.Status == types.StatusPending || other.Status == types.StatusRunning) {
						blockingDeps = append(blockingDeps, dep)
						break
					}
				}
			}
			if len(blockingDeps) > 0 {
				depStr := ""
				if len(blockingDeps) <= 5 {
					ds := make([]string, len(blockingDeps))
					for j, d := range blockingDeps {
						ds[j] = fmt.Sprintf("#%d", d)
					}
					depStr = strings.Join(ds, ", ")
				} else {
					depStr = fmt.Sprintf("%d tasks", len(blockingDeps))
				}
				extra = lipgloss.NewStyle().Foreground(m.theme.TextMuted).Render(
					fmt.Sprintf("  [blocked by: %s]", depStr))
			}
		default:
			statusIcon = "·"
			iconStyle = lipgloss.NewStyle().Foreground(m.theme.TextMuted)
		}

		// Truncate description
		desc := task.Description
		maxDescWidth := m.width - 30
		if maxDescWidth > 0 {
			desc = TruncateWithEllipsis(desc, maxDescWidth)
		}

		// Format: ✓  1  Install stripe-node v14                   0m 23s  12✦
		icon := iconStyle.Render(statusIcon)
		taskID := lipgloss.NewStyle().Foreground(m.theme.TextSecondary).Render(fmt.Sprintf("%d", task.ID))

		sb.WriteString(fmt.Sprintf("%s  %s  %s%s\n", icon, taskID, desc, extra))
	}

	// Total tool call count at bottom
	if m.toolCalls > 0 {
		sb.WriteString(lipgloss.NewStyle().
			Foreground(m.theme.TextSecondary).
			Render(fmt.Sprintf("  %d✦ total", m.toolCalls)))
		sb.WriteString("\n")
	}

	return sb.String()
}

// transitionTick returns a tea.Cmd that emits a TransitionTickMsg after 1 second.
func (m *ExecuteModel) transitionTick() tea.Cmd {
	return tea.Tick(time.Second, func(t time.Time) tea.Msg {
		return TransitionTickMsg{}
	})
}

// TransitionTickMsg is emitted every second during the transition countdown.
type TransitionTickMsg struct{}

func (m *ExecuteModel) renderMetricsHeader() string {
	// Calculate elapsed time
	elapsed := time.Since(m.startedAt)
	elapsedSec := int(elapsed.Seconds())

	// Count task statuses
	done := 0
	running := 0
	failed := 0
	for _, t := range m.tasks {
		switch t.Status {
		case types.StatusDone:
			done++
		case types.StatusRunning:
			running++
		case types.StatusFailed:
			failed++
		}
	}

	// Build metrics
	metrics := []components.MetricCard{
		{Value: components.FormatDuration(elapsedSec), Label: "Elapsed", Theme: m.theme},
		{Value: fmt.Sprintf("%d/%d", done, len(m.tasks)), Label: "Complete", Theme: m.theme},
		{Value: fmt.Sprintf("%d", m.toolCalls), Label: "Tool Calls", Theme: m.theme},
	}

	if m.totalTokens > 0 {
		metrics = append(metrics, components.MetricCard{
			Value: components.FormatMetric(m.totalTokens),
			Label: "Tokens",
			Theme: m.theme,
		})
	}

	if m.totalCost > 0 {
		metrics = append(metrics, components.MetricCard{
			Value: components.FormatCost(m.totalCost),
			Label: "Cost",
			Theme: m.theme,
		})
	}

	return components.MetricRow(metrics, m.width-4)
}
