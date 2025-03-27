package tui

import (
	"fmt"
	"strings"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/tui/theme"
	"github.com/eshanized/M31A/internal/types"
)

// ExecuteModel displays task execution progress.
type ExecuteModel struct {
	theme    theme.Theme
	tasks    []types.Task
	current  int
	width    int
	height   int
	paused   bool
	toolCard string
}

// NewExecuteModel creates an Execute screen model.
func NewExecuteModel(tasks []types.Task, t theme.Theme) *ExecuteModel {
	return &ExecuteModel{
		theme: t,
		tasks: tasks,
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
		case "p", "P":
			m.paused = true
		case "r", "R":
			m.paused = false
		case "s", "S":
			// Skip current task
			if m.current < len(m.tasks) {
				m.tasks[m.current].Status = types.StatusSkipped
			}
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

	// Header
	sb.WriteString(lipgloss.NewStyle().
		Foreground(m.theme.Brand).
		Bold(true).
		Render(" Execute "))
	sb.WriteString("\n\n")

	// Progress bar
	completed := 0
	for _, t := range m.tasks {
		if t.Status == types.StatusDone || t.Status == types.StatusSkipped {
			completed++
		}
	}
	total := len(m.tasks)
	pct := 0
	if total > 0 {
		pct = completed * 100 / total
	}
	sb.WriteString(fmt.Sprintf("%d/%d complete  %d%%  %s\n",
		completed, total, pct, m.renderProgressBar(pct)))
	sb.WriteString("\n")

	// Task list
	for i, task := range m.tasks {
		statusIcon := "[ ]"
		extra := ""
		switch task.Status {
		case types.StatusDone:
			statusIcon = "[x]"
		case types.StatusRunning:
			statusIcon = "[>]"
			extra = "  ← running"
		case types.StatusSkipped:
			statusIcon = "[-]"
			extra = "  ← skipped"
		case types.StatusFailed:
			statusIcon = "[!]"
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
				extra = fmt.Sprintf("  ← blocked (dep: %d)", task.Dependencies[0])
			}
		}

		prefix := statusIcon
		if i == m.current {
			prefix = lipgloss.NewStyle().
				Foreground(m.theme.Brand).
				Render(statusIcon)
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
		Render("P=Pause  R=Resume  S=Skip"))

	return sb.String()
}

func (m *ExecuteModel) renderProgressBar(pct int) string {
	width := 20
	filled := pct * width / 100

	bar := strings.Repeat("█", filled) + strings.Repeat("░", width-filled)
	return lipgloss.NewStyle().
		Foreground(m.theme.Brand).
		Render("[" + bar + "]")
}
