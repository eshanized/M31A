package tui

import (
	"fmt"
	"math"
	"strings"

	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/types"
)

func (m *PlanModel) View() string {
	if m.width == 0 {
		return m.spinner.View() + " Loading plan..."
	}

	var sb strings.Builder

	sb.WriteString(m.renderHeaderBar())
	sb.WriteString(m.renderKeyHints())
	sb.WriteString("\n")
	sb.WriteString(strings.Repeat("─", m.width-2))
	sb.WriteString("\n")

	if m.showGraph {
		sb.WriteString(m.renderHorizontalDependencyGraph())
	} else if m.showDiff {
		sb.WriteString(m.renderDiffPreview())
	} else {
		if len(m.tasks) > 0 && m.selected >= 0 && m.selected < len(m.tasks) {
			sb.WriteString(m.renderDetailBox(m.tasks[m.selected]))
			sb.WriteString("\n")
		}

		sb.WriteString(m.renderTaskList())
		sb.WriteString("\n")

		if len(m.tasks) > 0 && m.selected >= 0 && m.selected < len(m.tasks) {
			task := m.tasks[m.selected]
			if len(task.Files) > 0 {
				sb.WriteString(m.renderFileImpact(task))
				sb.WriteString("\n")
			}
		}
	}

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

func (m *PlanModel) renderHeaderBar() string {
	modelDisplay := m.modelName
	if modelDisplay == "" {
		modelDisplay = m.modelID
	}

	content := fmt.Sprintf(" Plan (Blueprint) ── %d tasks ── Est. $%.2f ── %s ", len(m.tasks), m.estCost, modelDisplay)

	innerWidth := m.width - 4
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

	var blocks []int
	for _, other := range m.tasks {
		for _, dep := range other.Dependencies {
			if dep == task.ID {
				blocks = append(blocks, other.ID)
			}
		}
	}

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

func (m *PlanModel) renderTaskList() string {
	var sb strings.Builder

	for i, task := range m.tasks {
		var prefix string
		var prefixStyle lipgloss.Style

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

		if i == m.selected {
			prefixStyle = lipgloss.NewStyle().
				Foreground(m.theme.Brand).
				Bold(true)
		}

		desc := task.Description
		badgeWidth := 8
		maxDescWidth := m.width - badgeWidth - 10
		if maxDescWidth > 0 {
			desc = TruncateWithEllipsis(desc, maxDescWidth)
		}

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

func (m *PlanModel) renderFileImpact(task types.Task) string {
	var sb strings.Builder

	headerStyle := lipgloss.NewStyle().
		Foreground(m.theme.TextSecondary).
		Bold(true)
	sb.WriteString(headerStyle.Render("── File Impact "))
	sb.WriteString(strings.Repeat("─", max(0, m.width-lipgloss.Width(headerStyle.Render("── File Impact "))-2)))
	sb.WriteString("\n")

	fileStyle := lipgloss.NewStyle().Foreground(m.theme.TextMuted)

	for _, f := range task.Files {
		action := m.actionBadge(task.Action)

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

func (m *PlanModel) renderHorizontalDependencyGraph() string {
	var sb strings.Builder
	sb.WriteString("Dependency Graph:\n\n")

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

	byID := make(map[int]types.Task)
	for _, task := range m.tasks {
		byID[task.ID] = task
	}

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

	rendered := make(map[int]bool)
	for i, rootID := range roots {
		renderNode(rootID, 0, "", i == len(roots)-1)
		rendered[rootID] = true
	}

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

		taskHeaderStyle := lipgloss.NewStyle().
			Foreground(m.theme.Brand).
			Bold(true)
		sb.WriteString(taskHeaderStyle.Render(fmt.Sprintf("Task %d: %s", task.ID, task.Description)))
		sb.WriteString(fmt.Sprintf(" (%d files)\n", len(task.Files)))

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
