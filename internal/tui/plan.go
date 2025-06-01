package tui

import (
	"fmt"
	"strings"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/tui/theme"
	"github.com/eshanized/M31A/internal/types"
)

// PlanModel displays the task plan for user review.
type PlanModel struct {
	theme    theme.Theme
	tasks    []types.Task
	selected int
	width    int
	height   int
	modelID  string
	provider string
	estCost  float64
	estTime  string
	showDiff bool
	showGraph bool
}

// NewPlanModel creates a Plan screen model.
func NewPlanModel(tasks []types.Task, t theme.Theme, modelID string, providerName string, estCost float64, estTime string) *PlanModel {
	return &PlanModel{
		theme:    t,
		tasks:    tasks,
		modelID:  modelID,
		provider: providerName,
		estCost:  estCost,
		estTime:  estTime,
	}
}

func (m *PlanModel) Update(msg tea.Msg) ([]tea.Cmd, *AppMsg) {
	switch msg := msg.(type) {
	case tea.WindowSizeMsg:
		m.width = msg.Width
		m.height = msg.Height
		return nil, nil

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
		case "e", "E":
			// V1: placeholder for edit
		}
	}
	return nil, nil
}

func (m *PlanModel) View() string {
	if m.width == 0 {
		return "Loading plan..."
	}

	var sb strings.Builder

	// Header
	sb.WriteString(lipgloss.NewStyle().
		Foreground(m.theme.Brand).
		Bold(true).
		Render(" Plan "))
	sb.WriteString("\n")
	sb.WriteString(lipgloss.NewStyle().
		Foreground(m.theme.TextSecondary).
		Render("[A]ccept  [E]dit  [R]etry  [D]iff  Tab=Graph"))
	sb.WriteString("\n\n")

	if m.showGraph {
		sb.WriteString(m.renderDependencyGraph())
	} else if m.showDiff {
		sb.WriteString(m.renderDiffPreview())
	} else {
		sb.WriteString(m.renderTaskList())
	}

	// Cost/Time panel
	sb.WriteString("\n")
	sb.WriteString(lipgloss.NewStyle().
		Border(lipgloss.RoundedBorder()).
		BorderForeground(m.theme.Border).
		Padding(1, 2).
		Render(fmt.Sprintf("Model: %s\nEst cost: $%.2f\nEst time: %s\nProvider: %s",
			m.modelID, m.estCost, m.estTime, m.provider)))

	return sb.String()
}

func (m *PlanModel) renderTaskList() string {
	var sb strings.Builder

	for i, task := range m.tasks {
		prefix := "[ ]"
		if i == m.selected {
			prefix = "[>]"
		}
		if task.Status == types.StatusDone {
			prefix = "[x]"
		}

		sb.WriteString(fmt.Sprintf("%s %d. %s\n", prefix, task.ID, task.Description))

		deps := "-"
		if len(task.Dependencies) > 0 {
			ds := make([]string, len(task.Dependencies))
			for j, d := range task.Dependencies {
				ds[j] = fmt.Sprintf("%d", d)
			}
			deps = strings.Join(ds, ", ")
		}
		sb.WriteString(lipgloss.NewStyle().
			Foreground(m.theme.TextSecondary).
			Render(fmt.Sprintf("     deps: %s", deps)))
		sb.WriteString("\n")
	}

	return sb.String()
}

func (m *PlanModel) renderDependencyGraph() string {
	var sb strings.Builder
	sb.WriteString("Dependency Graph:\n\n")

	for _, task := range m.tasks {
		if len(task.Dependencies) == 0 {
			sb.WriteString(fmt.Sprintf("  [%d] %s\n", task.ID, task.Description))
		} else {
			for _, dep := range task.Dependencies {
				sb.WriteString(fmt.Sprintf("  [%d] -> [%d] %s\n", dep, task.ID, task.Description))
			}
		}
	}

	return sb.String()
}

func (m *PlanModel) renderDiffPreview() string {
	var sb strings.Builder
	sb.WriteString("Predicted File Changes:\n\n")

	for _, task := range m.tasks {
		for _, f := range task.Files {
			action := "+"
			if task.Action == "Modify" {
				action = "~"
			} else if task.Action == "Delete" {
				action = "-"
			}
			sb.WriteString(fmt.Sprintf("  %s %s\n", action, f))
		}
	}

	return sb.String()
}
