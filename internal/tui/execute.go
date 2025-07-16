package tui

import (
	"fmt"
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
	paused        bool
	spinner       spinner.Model
	allDone       bool
	transitioning bool
	transitionSec int
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

	// Header with execution status
	headerStyle := lipgloss.NewStyle().
		Foreground(m.theme.Brand).
		Bold(true)

	statusBadge := "▶ Running"
	if m.paused {
		statusBadge = "⏸ Paused"
	}
	sb.WriteString(headerStyle.Render(" Execute "))
	sb.WriteString(" ")
	sb.WriteString(lipgloss.NewStyle().
		Foreground(m.theme.TextSecondary).
		Render(statusBadge))
	sb.WriteString("\n\n")

	// Metrics dashboard header
	sb.WriteString(m.renderMetricsHeader())
	sb.WriteString("\n\n")

	// Progress bar
	completed := 0
	running := 0
	failed := 0
	skipped := 0
	for _, t := range m.tasks {
		switch t.Status {
		case types.StatusDone:
			completed++
		case types.StatusRunning:
			running++
		case types.StatusSkipped:
			skipped++
		case types.StatusFailed:
			failed++
		}
	}
	total := len(m.tasks)

	// Segmented progress bar showing done/skipped/failed/pending
	segBar := components.SegmentedBar{
		Segments: []components.Segment{
			{Count: completed, Color: m.theme.Success, Label: "done"},
			{Count: skipped, Color: m.theme.Warning, Label: "skipped"},
			{Count: failed, Color: m.theme.Error, Label: "failed"},
		},
		Width: m.width - 4,
		Theme: m.theme,
	}
	sb.WriteString(fmt.Sprintf("%d/%d tasks completed", completed, total))
	if skipped > 0 {
		sb.WriteString(fmt.Sprintf(" (%d skipped)", skipped))
	}
	if failed > 0 {
		sb.WriteString(fmt.Sprintf(" (%d failed)", failed))
	}
	sb.WriteString("\n")
	sb.WriteString(segBar.Render())

	// Task list
	for i, task := range m.tasks {
		statusIcon := "[ ]"
		extra := ""
		switch task.Status {
		case types.StatusDone:
			statusIcon = lipgloss.NewStyle().Foreground(m.theme.Success).Render("[✓]")
		case types.StatusRunning:
			statusIcon = lipgloss.NewStyle().Foreground(m.theme.Brand).Bold(true).Render("[▶]")
			extra = "  " + lipgloss.NewStyle().Foreground(m.theme.Brand).Bold(true).Render("← running") + " " + m.spinner.View()
		case types.StatusSkipped:
			statusIcon = lipgloss.NewStyle().Foreground(m.theme.Warning).Render("[-]")
			extra = "  ← skipped"
		case types.StatusFailed:
			statusIcon = lipgloss.NewStyle().Foreground(m.theme.Error).Render("[✗]")
			extra = "  ← failed"
		case types.StatusPending:
			// Check if blocked
			blocked := false
			for _, dep := range task.Dependencies {
				for _, other := range m.tasks {
					if other.ID == dep && (other.Status == types.StatusPending || other.Status == types.StatusRunning) {
						blocked = true
						break
					}
				}
			}
			if blocked {
				// Collect all blocking dependency IDs
				var blockingDeps []int
				for _, dep := range task.Dependencies {
					for _, other := range m.tasks {
						if other.ID == dep && (other.Status == types.StatusPending || other.Status == types.StatusRunning) {
							blockingDeps = append(blockingDeps, dep)
							break
						}
					}
				}
				depStr := ""
				if len(blockingDeps) <= 5 {
					ds := make([]string, len(blockingDeps))
					for i, d := range blockingDeps {
						ds[i] = fmt.Sprintf("%d", d)
					}
					depStr = strings.Join(ds, ", ")
				} else {
					depStr = fmt.Sprintf("%d tasks", len(blockingDeps))
				}
				extra = lipgloss.NewStyle().Foreground(m.theme.TextMuted).Render(
					fmt.Sprintf("  ← blocked by: %s", depStr))
			}
		}

		prefix := statusIcon
		if i == m.current {
			prefix = lipgloss.NewStyle().
				Background(m.theme.Surface).
				Foreground(m.theme.Brand).
				Bold(true).
				Render(" " + statusIcon + " ")
		}

		desc := task.Description
		if i != m.current {
			maxDescWidth := m.width - 20
			if maxDescWidth > 0 {
				desc = TruncateWithEllipsis(desc, maxDescWidth)
			}
		}
		sb.WriteString(fmt.Sprintf("%s %d. %s%s\n", prefix, task.ID, desc, extra))
	}

	// Tool card
	if m.toolCard != "" {
		sb.WriteString("\n")
		sb.WriteString(lipgloss.NewStyle().
			Border(lipgloss.RoundedBorder()).
			BorderForeground(m.theme.Border).
			Padding(0, 1).
			Render(m.toolCard))
	}

	// Keys
	sb.WriteString("\n\n")
	sb.WriteString(lipgloss.NewStyle().
		Foreground(m.theme.TextSecondary).
		Render("P=Pause  R=Resume  S=Skip  ↑↓=Navigate"))

	// Show completion summary when all tasks are done (before transitioning to Verify)
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
