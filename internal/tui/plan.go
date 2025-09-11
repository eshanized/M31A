package tui

import (
	"fmt"
	"math"
	"strings"

	"github.com/charmbracelet/bubbles/spinner"
	tea "github.com/charmbracelet/bubbletea"
	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/tui/theme"
	"github.com/eshanized/M31A/internal/types"
	"github.com/eshanized/M31A/pkg/arbitrage"
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

// ApplyArbitrage applies model cost optimization recommendations to the plan.
// BUG-05 fix: implements the handler for OptimizedMsg.
func (m *PlanModel) ApplyArbitrage(recs []arbitrage.ArbitrageRecommendation) {
	for _, rec := range recs {
		if rec.Savings > 0 {
			m.estCost -= rec.Savings
			if m.estCost < 0 {
				m.estCost = 0
			}
		}
	}
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

	// Blueprint header bar: ╭─ Blueprint ── N tasks ── Est. $X.XX ── model-name ─╮
	sb.WriteString(m.renderHeaderBar())

	// Key hints row
	sb.WriteString(m.renderKeyHints())
	sb.WriteString("\n")

	// Separator
	sb.WriteString(strings.Repeat("─", m.width-2))
	sb.WriteString("\n")

	if m.showGraph {
		sb.WriteString(m.renderHorizontalDependencyGraph())
	} else if m.showDiff {
		sb.WriteString(m.renderDiffPreview())
	} else {
		// Selected task detail box (double-border)
		if len(m.tasks) > 0 && m.selected >= 0 && m.selected < len(m.tasks) {
			sb.WriteString(m.renderDetailBox(m.tasks[m.selected]))
			sb.WriteString("\n")
		}

		// Compact task list
		sb.WriteString(m.renderTaskList())
		sb.WriteString("\n")

		// File impact section for selected task
		if len(m.tasks) > 0 && m.selected >= 0 && m.selected < len(m.tasks) {
			task := m.tasks[m.selected]
			if len(task.Files) > 0 {
				sb.WriteString(m.renderFileImpact(task))
				sb.WriteString("\n")
			}
		}
	}

	// Cost/Time panel — show human-readable model name with raw ID as tooltip
	modelDisplay := m.modelName
	if modelDisplay == "" {
		modelDisplay = m.modelID
	}
	sb.WriteString(lipgloss.NewStyle().
		Border(lipgloss.RoundedBorder()).
		BorderForeground(m.theme.Border).
		Padding(1, 2).
		Render(fmt.Sprintf("Model: %s\nEst cost: $%.2f\nEst time: %s\nProvider: %s",
			modelDisplay, m.estCost, m.estTime, m.provider)))

	return sb.String()
}

// renderHeaderBar renders the Blueprint header with border.
//
//	╭─ Blueprint ── N tasks ── Est. $X.XX ── model-name ─╮
func (m *PlanModel) renderHeaderBar() string {
	modelDisplay := m.modelName
	if modelDisplay == "" {
		modelDisplay = m.modelID
	}

	content := fmt.Sprintf(" Plan (Blueprint) ── %d tasks ── Est. $%.2f ── %s ", len(m.tasks), m.estCost, modelDisplay)

	// Pad or truncate to fit width
	innerWidth := m.width - 4 // account for ╭ and ╮
	if innerWidth < 20 {
		innerWidth = 20
	}
	runes := []rune(content)
	if len(runes) > innerWidth {
		content = string(runes[:innerWidth-1]) + "…"
	} else if len(runes) < innerWidth {
		content += strings.Repeat("─", innerWidth-len(runes))
	}

	borderStyle := lipgloss.NewStyle().
		Foreground(m.theme.Brand).
		Bold(true)

	return borderStyle.Render("╭" + content + "╮") + "\n"
}

// renderKeyHints renders the keyboard shortcut hints row.
//
//	[A]ccept  [R]etry  [D]iff  [O]ptimize  Tab=Graph  Esc=Back
func (m *PlanModel) renderKeyHints() string {
	hintStyle := lipgloss.NewStyle().
		Foreground(m.theme.TextSecondary)

	keyStyle := lipgloss.NewStyle().
		Foreground(m.theme.Brand).
		Bold(true)

	var sb strings.Builder
	sb.WriteString("│ ")
	sb.WriteString(keyStyle.Render("[A]ccept"))
	sb.WriteString("  ")
	sb.WriteString(keyStyle.Render("[R]etry"))
	sb.WriteString("  ")
	sb.WriteString(keyStyle.Render("[D]iff"))
	sb.WriteString("  ")
	sb.WriteString(keyStyle.Render("[O]ptimize"))
	sb.WriteString("  ")
	sb.WriteString(keyStyle.Render("Tab=Graph"))
	sb.WriteString("  ")
	sb.WriteString(keyStyle.Render("Esc=Back"))
	sb.WriteString(strings.Repeat(" ", max(0, m.width-lipgloss.Width(sb.String())-1)))
	sb.WriteString(hintStyle.Render("│"))

	return sb.String()
}

// renderDetailBox renders the selected task in a double-border box.
//
//	╔════════════════════════════════════════════════════════╗
//	║  TASK #3 (selected)                                   ║
//	║  Update webhook signature verification                ║
//	║  Depends on: #1, #2  ·  Blocks: #5, #6  ·  Files: 2  ║
//	╚════════════════════════════════════════════════════════╝
func (m *PlanModel) renderDetailBox(task types.Task) string {
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

	titleStyle := lipgloss.NewStyle().
		Foreground(m.theme.Brand).
		Bold(true)

	descStyle := lipgloss.NewStyle().
		Foreground(m.theme.TextPrimary)

	detailStyle := lipgloss.NewStyle().
		Foreground(m.theme.TextSecondary)

	// Build blocks list
	var blocks []int
	for _, other := range m.tasks {
		for _, dep := range other.Dependencies {
			if dep == task.ID {
				blocks = append(blocks, other.ID)
			}
		}
	}

	// Build detail line
	var details []string
	if len(task.Dependencies) > 0 {
		ds := make([]string, len(task.Dependencies))
		for i, d := range task.Dependencies {
			ds[i] = fmt.Sprintf("#%d", d)
		}
		details = append(details, fmt.Sprintf("Depends on: %s", strings.Join(ds, ", ")))
	}
	if len(blocks) > 0 {
		bs := make([]string, len(blocks))
		for i, b := range blocks {
			bs[i] = fmt.Sprintf("#%d", b)
		}
		details = append(details, fmt.Sprintf("Blocks: %s", strings.Join(bs, ", ")))
	}
	details = append(details, fmt.Sprintf("Files: %d", len(task.Files)))

	detailLine := strings.Join(details, "  ·  ")

	content := lipgloss.JoinVertical(lipgloss.Left,
		titleStyle.Render(fmt.Sprintf("  TASK #%d (selected)", task.ID)),
		descStyle.Render("  "+task.Description),
		detailStyle.Render("  "+detailLine),
	)

	boxWidth := m.width - 2
	if boxWidth < 30 {
		boxWidth = 30
	}

	box := lipgloss.NewStyle().
		Border(doubleBorder).
		BorderForeground(m.theme.CardBorderActive.GetForeground()).
		Width(boxWidth).
		Padding(0, 1).
		Render(content)

	return box
}

// renderTaskList renders a compact task list with status icons and action badges.
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
		// Truncate description to leave room for action badge
		badgeWidth := 8 // " NEW ✦" or " MOD ~"
		maxDescWidth := m.width - badgeWidth - 10
		if maxDescWidth > 0 {
			desc = TruncateWithEllipsis(desc, maxDescWidth)
		}

		// Action badge
		badge := m.actionBadge(task.Action)

		sb.WriteString(fmt.Sprintf("%s %d. %s%s\n", prefixStyle.Render(prefix), task.ID, desc, badge))

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

// actionBadge returns a styled action badge for a task.
func (m *PlanModel) actionBadge(action string) string {
	badgeStyle := lipgloss.NewStyle().Foreground(m.theme.TextSecondary)

	switch action {
	case "Create", "create":
		return badgeStyle.Foreground(m.theme.Success).Render("  ✦ NEW")
	case "Modify", "modify":
		return badgeStyle.Foreground(m.theme.Warning).Render("  ~ MOD")
	case "Delete", "delete":
		return badgeStyle.Foreground(m.theme.Error).Render("  ✗ DEL")
	default:
		// Default to MOD for unspecified actions
		return badgeStyle.Foreground(m.theme.Warning).Render("  ~ MOD")
	}
}

// renderFileImpact renders the file impact section for the selected task.
func (m *PlanModel) renderFileImpact(task types.Task) string {
	var sb strings.Builder

	// Section header
	headerStyle := lipgloss.NewStyle().
		Foreground(m.theme.TextSecondary).
		Bold(true)
	sb.WriteString(headerStyle.Render("── File Impact "))
	sb.WriteString(strings.Repeat("─", max(0, m.width-lipgloss.Width(headerStyle.Render("── File Impact "))-2)))
	sb.WriteString("\n")

	// File list with action badges
	fileStyle := lipgloss.NewStyle().Foreground(m.theme.TextMuted)

	for _, f := range task.Files {
		action := m.actionBadge(task.Action)

		// Estimate lines changed (heuristic: ~30 lines per file for display)
		estLines := 30
		barWidth := 16
		filled := int(math.Min(float64(barWidth), float64(estLines)/100.0*float64(barWidth)))
		empty := barWidth - filled

		bar := lipgloss.NewStyle().Foreground(m.theme.Brand).Render(strings.Repeat("█", filled)) +
			lipgloss.NewStyle().Foreground(m.theme.Border).Render(strings.Repeat("░", empty))

		sb.WriteString(fmt.Sprintf("  %s  %s  %s  ~%d lines changed\n",
			fileStyle.Render(f),
			action,
			bar,
			estLines,
		))
	}

	return sb.String()
}

// renderHorizontalDependencyGraph renders a left-to-right dependency graph.
//
//	●─── #1: Install stripe-node v14
//	│
//	●─── #2: Update TypeScript types
//	│
//	└──● #3: Update webhook sig ─────────────●─── #5: Add idempotency
func (m *PlanModel) renderHorizontalDependencyGraph() string {
	var sb strings.Builder
	sb.WriteString("Dependency Graph:\n\n")

	// Build adjacency: parent → children
	children := make(map[int][]int)
	roots := []int{}

	for _, task := range m.tasks {
		if len(task.Dependencies) == 0 {
			roots = append(roots, task.ID)
		} else {
			for _, dep := range task.Dependencies {
				children[dep] = append(children[dep], task.ID)
			}
		}
	}

	// Task lookup
	byID := make(map[int]types.Task)
	for _, task := range m.tasks {
		byID[task.ID] = task
	}

	// Render tree recursively
	var renderNode func(id int, depth int, prefix string, isLast bool)
	renderNode = func(id int, depth int, prefix string, isLast bool) {
		task, ok := byID[id]
		if !ok {
			return
		}
		connector := "├── "
		if isLast {
			connector = "└── "
		}
		if depth == 0 {
			connector = "●─── "
		}

		nodeStyle := lipgloss.NewStyle().Foreground(m.theme.Brand)
		descStyle := lipgloss.NewStyle().Foreground(m.theme.TextSecondary)

		sb.WriteString(prefix)
		sb.WriteString(connector)
		sb.WriteString(nodeStyle.Render(fmt.Sprintf("#%d", task.ID)))
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

	// Render root nodes
	rendered := make(map[int]bool)
	for i, rootID := range roots {
		renderNode(rootID, 0, "", i == len(roots)-1)
		rendered[rootID] = true
	}

	// Render any nodes not reached (circular protection)
	for _, task := range m.tasks {
		if !rendered[task.ID] {
			nodeStyle := lipgloss.NewStyle().Foreground(m.theme.Brand)
			descStyle := lipgloss.NewStyle().Foreground(m.theme.TextSecondary)
			sb.WriteString(fmt.Sprintf("  %s %s\n",
				nodeStyle.Render(fmt.Sprintf("#%d", task.ID)),
				descStyle.Render(task.Description)))
			rendered[task.ID] = true
		}
	}

	// Legend
	sb.WriteString("\n")
	legendStyle := lipgloss.NewStyle().Foreground(m.theme.TextSecondary)
	sb.WriteString(legendStyle.Render("  ●  root task  ──  dependency edge  [Esc] back to task list"))
	sb.WriteString("\n")

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
