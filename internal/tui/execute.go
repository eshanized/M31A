package tui

import (
	"fmt"
	"strings"
	"time"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/tui/components"
	"github.com/eshanized/M31A/internal/tui/theme"
	"github.com/eshanized/M31A/internal/types"
)

// ExecuteModel displays task execution progress.
type ExecuteModel struct {
	theme       theme.Theme
	tasks       []types.Task
	current     int
	width       int
	height      int
	toolCard    string
	startedAt   time.Time
	taskStarted time.Time
	totalTokens int
	totalCost   float64
	toolCalls   int
	paused      bool
}

// NewExecuteModel creates an Execute screen model. width/height are
// required non-zero dimensions so the screen renders immediately
// on creation without waiting for a separate WindowSizeMsg (D-03 fix).
func NewExecuteModel(tasks []types.Task, t theme.Theme, width, height int) *ExecuteModel {
	return &ExecuteModel{
		theme:     t,
		tasks:     tasks,
		width:     width,
		height:    height,
		startedAt: time.Now(),
	}
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

	case tea.KeyMsg:
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
		}

		// Check if all done
		allDone := true
		for _, t := range m.tasks {
			if t.Status == types.StatusPending || t.Status == types.StatusRunning {
				allDone = false
				break
			}
		}
		if allDone {
			return nil, &AppMsg{Screen: ScreenVerify}
		}
	}
	return nil, nil
}

func (m *ExecuteModel) View() string {
	if m.width == 0 {
		return "Loading execute..."
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
	t := theme.Default()
	segBar := components.SegmentedBar{
		Segments: []components.Segment{
			{Count: completed, Color: t.Success, Label: "done"},
			{Count: skipped, Color: t.Warning, Label: "skipped"},
			{Count: failed, Color: t.Error, Label: "failed"},
		},
		Width: m.width - 4,
	}
	sb.WriteString(fmt.Sprintf("%d/%d tasks  %s\n\n",
		completed+skipped, total, segBar.Render()))

	// Task list
	for i, task := range m.tasks {
		statusIcon := "[ ]"
		extra := ""
		switch task.Status {
		case types.StatusDone:
			statusIcon = lipgloss.NewStyle().Foreground(t.Success).Render("[✓]")
		case types.StatusRunning:
			statusIcon = lipgloss.NewStyle().Foreground(m.theme.Brand).Render("[▶]")
			extra = "  ← running"
		case types.StatusSkipped:
			statusIcon = lipgloss.NewStyle().Foreground(t.Warning).Render("[-]")
			extra = "  ← skipped"
		case types.StatusFailed:
			statusIcon = lipgloss.NewStyle().Foreground(t.Error).Render("[✗]")
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
				extra = lipgloss.NewStyle().Foreground(t.TextMuted).Render(
					fmt.Sprintf("  ← blocked (dep: %d)", task.Dependencies[0]))
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

		sb.WriteString(fmt.Sprintf("%s %d. %s%s\n", prefix, task.ID, task.Description, extra))
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

	return sb.String()
}

// renderMetricsHeader renders the execution metrics dashboard.
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
		{Value: components.FormatDuration(elapsedSec), Label: "Elapsed"},
		{Value: fmt.Sprintf("%d/%d", done, len(m.tasks)), Label: "Complete"},
		{Value: fmt.Sprintf("%d", m.toolCalls), Label: "Tool Calls"},
	}

	if m.totalTokens > 0 {
		metrics = append(metrics, components.MetricCard{
			Value: components.FormatMetric(m.totalTokens),
			Label: "Tokens",
		})
	}

	if m.totalCost > 0 {
		metrics = append(metrics, components.MetricCard{
			Value: components.FormatCost(m.totalCost),
			Label: "Cost",
		})
	}

	return components.MetricRow(metrics, m.width-4)
}
