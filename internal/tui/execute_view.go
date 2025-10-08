package tui

import (
	"fmt"
	"math"
	"strings"
	"time"

	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/tui/components"
	"github.com/eshanized/M31A/internal/types"
)

func (m *ExecuteModel) View() string {
	if m.width == 0 {
		return m.spinner.View() + " Loading execute..."
	}

	var sb strings.Builder

	sb.WriteString(m.renderHeader())
	sb.WriteString("\n")
	sb.WriteString(m.renderLiveMetrics())
	sb.WriteString("\n")
	sb.WriteString(strings.Repeat("─", m.width-2))
	sb.WriteString("\n")
	sb.WriteString(m.renderProgressBar())
	sb.WriteString("\n")

	runningTask := m.findRunningTask()
	if runningTask != nil {
		sb.WriteString(m.renderRunningPanel(runningTask))
		sb.WriteString("\n")
	}

	sb.WriteString(m.renderCompactTaskList())

	if m.toolCard != "" && runningTask == nil {
		sb.WriteString("\n")
		sb.WriteString(lipgloss.NewStyle().
			Border(lipgloss.RoundedBorder()).
			BorderForeground(m.theme.Border).
			Padding(0, 1).
			Render(m.toolCard))
	}

	sb.WriteString("\n")
	sb.WriteString(lipgloss.NewStyle().
		Foreground(m.theme.TextSecondary).
		Render("P=Pause  R=Resume  S=Skip  ↑↓=Navigate"))

	allDone := true
	hasFailures := false
	for _, task := range m.tasks {
		switch task.Status {
		case types.StatusDone:
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

		desc := task.Description
		maxDescWidth := m.width - 30
		if maxDescWidth > 0 {
			desc = TruncateWithEllipsis(desc, maxDescWidth)
		}

		icon := iconStyle.Render(statusIcon)
		taskID := lipgloss.NewStyle().Foreground(m.theme.TextSecondary).Render(fmt.Sprintf("%d", task.ID))

		sb.WriteString(fmt.Sprintf("%s  %s  %s%s\n", icon, taskID, desc, extra))
	}

	if m.toolCalls > 0 {
		sb.WriteString(lipgloss.NewStyle().
			Foreground(m.theme.TextSecondary).
			Render(fmt.Sprintf("  %d✦ total", m.toolCalls)))
		sb.WriteString("\n")
	}

	return sb.String()
}

func (m *ExecuteModel) renderMetricsHeader() string {
	elapsed := time.Since(m.startedAt)
	elapsedSec := int(elapsed.Seconds())

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
