package tui

import (
	"fmt"
	"strings"

	"github.com/charmbracelet/bubbles/spinner"
	tea "github.com/charmbracelet/bubbletea"
	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/tui/theme"
	"github.com/eshanized/M31A/internal/types"
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

// NewPlanModel creates a Plan screen model. width/height are required
// non-zero dimensions so the plan renders immediately on creation
// without waiting for a separate WindowSizeMsg (D-03 fix).
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
			// H-19: trigger arbitrage optimization via command registry
			return []tea.Cmd{func() tea.Msg {
				return SlashCommandMsg{Command: "/optimize"}
			}}, nil
		}
	}
	return nil, nil
}

func (m *PlanModel) View() string {
	if m.width == 0 {
		return m.spinner.View() + " Loading plan..."
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
		Render("[A]ccept  [R]etry  [D]iff  Tab=Graph  [Esc] Back  [↑/↓] Navigate  [Enter] Select"))
	sb.WriteString("\n\n")

	if m.showGraph {
		sb.WriteString(m.renderDependencyGraph())
	} else if m.showDiff {
		sb.WriteString(m.renderDiffPreview())
	} else {
		sb.WriteString(m.renderTaskList())
	}

	// Cost/Time panel — show human-readable model name with raw ID as tooltip
	modelDisplay := m.modelName
	if modelDisplay == "" {
		modelDisplay = m.modelID
	}
	sb.WriteString("\n")
	sb.WriteString(lipgloss.NewStyle().
		Border(lipgloss.RoundedBorder()).
		BorderForeground(m.theme.Border).
		Padding(1, 2).
		Render(fmt.Sprintf("Model: %s\nEst cost: $%.2f\nEst time: %s\nProvider: %s",
			modelDisplay, m.estCost, m.estTime, m.provider)))

	return sb.String()
}

func (m *PlanModel) renderTaskList() string {
	var sb strings.Builder

	for i, task := range m.tasks {
		var prefix string
		var prefixStyle lipgloss.Style

		// Determine status prefix and color
		switch task.Status {
		case types.StatusDone:
			prefix = "[x]"
			prefixStyle = lipgloss.NewStyle().Foreground(m.theme.Success)
		case types.StatusRunning:
			prefix = "[>]"
			prefixStyle = lipgloss.NewStyle().Foreground(m.theme.Brand)
		case types.StatusFailed:
			prefix = "[!]"
			prefixStyle = lipgloss.NewStyle().Foreground(m.theme.Error)
		case types.StatusSkipped:
			prefix = "[-]"
			prefixStyle = lipgloss.NewStyle().Foreground(m.theme.Warning)
		case types.StatusPending:
			if i == m.selected {
				prefix = "[>]"
				prefixStyle = lipgloss.NewStyle().Foreground(m.theme.Brand)
			} else {
				prefix = "[ ]"
				prefixStyle = lipgloss.NewStyle().Foreground(m.theme.TextMuted)
			}
		default:
			prefix = "[ ]"
			prefixStyle = lipgloss.NewStyle().Foreground(m.theme.TextMuted)
		}

		// Override style for selected item
		if i == m.selected {
			prefixStyle = lipgloss.NewStyle().
				Foreground(m.theme.Brand).
				Bold(true)
		}

		desc := task.Description
		if i != m.selected {
			maxDescWidth := m.width - 20
			if maxDescWidth > 0 {
				desc = TruncateWithEllipsis(desc, maxDescWidth)
			}
		}
		sb.WriteString(fmt.Sprintf("%s %d. %s\n", prefixStyle.Render(prefix), task.ID, desc))

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

	// Build adjacency: parent → children
	children := make(map[int][]int)
	roots := []int{}
	hasParent := make(map[int]bool)

	for _, task := range m.tasks {
		if len(task.Dependencies) == 0 {
			roots = append(roots, task.ID)
		} else {
			for _, dep := range task.Dependencies {
				children[dep] = append(children[dep], task.ID)
				hasParent[task.ID] = true
			}
		}
	}

	// Task lookup
	byID := make(map[int]types.Task)
	for _, task := range m.tasks {
		byID[task.ID] = task
	}

	// Render tree recursively with indentation
	var renderNode func(id int, depth int, prefix string, isLast bool)
	renderNode = func(id int, depth int, prefix string, isLast bool) {
		task := byID[id]
		connector := "├── "
		if isLast {
			connector = "└── "
		}
		if depth == 0 {
			connector = ""
		}

		nodeStyle := lipgloss.NewStyle().Foreground(m.theme.TextPrimary)
		descStyle := lipgloss.NewStyle().Foreground(m.theme.TextSecondary)

		sb.WriteString(prefix)
		sb.WriteString(connector)
		sb.WriteString(nodeStyle.Render(fmt.Sprintf("[%d]", task.ID)))
		sb.WriteString(" ")
		sb.WriteString(descStyle.Render(task.Description))
		sb.WriteString("\n")

		childList := children[id]
		for i, childID := range childList {
			newPrefix := prefix
			if depth > 0 {
				if isLast {
					newPrefix += "    "
				} else {
					newPrefix += "│   "
				}
			}
			renderNode(childID, depth+1, newPrefix, i == len(childList)-1)
		}
	}

	// Render root nodes and any orphaned nodes
	rendered := make(map[int]bool)
	for i, rootID := range roots {
		renderNode(rootID, 0, "", i == len(roots)-1)
		rendered[rootID] = true
	}

	// Render nodes with parents that weren't reached (circular protection)
	for _, task := range m.tasks {
		if !rendered[task.ID] {
			sb.WriteString(fmt.Sprintf("  [%d] %s\n", task.ID, task.Description))
			rendered[task.ID] = true
		}
	}

	return sb.String()
}

func (m *PlanModel) renderDiffPreview() string {
	var sb strings.Builder
	sb.WriteString("Predicted File Changes:\n\n")

	for _, task := range m.tasks {
		if len(task.Files) == 0 {
			continue
		}

		// Task header
		taskHeaderStyle := lipgloss.NewStyle().
			Foreground(m.theme.Brand).
			Bold(true)
		sb.WriteString(taskHeaderStyle.Render(fmt.Sprintf("Task %d: %s", task.ID, task.Description)))
		sb.WriteString(fmt.Sprintf(" (%d files)\n", len(task.Files)))

		// Files under this task
		for _, f := range task.Files {
			action := "+"
			actionStyle := lipgloss.NewStyle().Foreground(m.theme.Success)
			if task.Action == "Modify" {
				action = "~"
				actionStyle = lipgloss.NewStyle().Foreground(m.theme.Warning)
			} else if task.Action == "Delete" {
				action = "-"
				actionStyle = lipgloss.NewStyle().Foreground(m.theme.Error)
			}
			sb.WriteString(fmt.Sprintf("  %s %s\n", actionStyle.Render(action), f))
		}
		sb.WriteString("\n")
	}

	return sb.String()
}
