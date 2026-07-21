package tui

import (
	"fmt"
	"math"
	"strings"
	"time"

	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/integrations/git"
	"github.com/eshanized/M31A/internal/tui/components"
	"github.com/eshanized/M31A/internal/tui/theme"
)

func (s *SidebarModel) View() string {
	if !s.visible {
		return ""
	}
	t := s.theme
	w := s.width
	contentW := w - 1

	// Route to mode-specific renderer
	var lines []string
	switch s.mode {
	case SidebarModeIdle:
		lines = s.renderIdle(contentW)
	case SidebarModeActive:
		lines = s.renderActive(contentW)
	case SidebarModeNarrative:
		lines = RenderNarrativeSidebar(s.narrativeState, t, contentW)
		if len(lines) == 0 {
			lines = s.renderIdle(contentW)
		}
	default:
		// SidebarModeFiles and SidebarModeTodo use full rendering
		lines = append(lines, s.renderHeader(contentW)...)
		lines = append(lines, s.renderGitStatus(contentW)...)
		lines = append(lines, s.renderTokenUsage(contentW)...)
		lines = append(lines, s.renderPhasePipeline(contentW)...)
		lines = append(lines, s.renderToolTimeline(contentW)...)
		lines = append(lines, s.renderSpeedMetrics(contentW)...)
		lines = append(lines, s.renderSubAgentBadge(contentW)...)
		lines = append(lines, s.renderPendingPermBadge(contentW)...)
		lines = append(lines, s.renderFileTreeOrTodo(contentW)...)
		lines = append(lines, s.renderSession(contentW)...)
		lines = append(lines, s.renderHints(contentW)...)
	}

	// Pad each line to exactly contentW characters for consistent border alignment.
	var paddedLines []string
	for _, line := range lines {
		lineW := lipgloss.Width(line)
		if lineW < contentW {
			line += strings.Repeat(" ", contentW-lineW)
		}
		paddedLines = append(paddedLines, line)
	}
	panel := lipgloss.NewStyle().Render(strings.Join(paddedLines, "\n"))

	// Calculate line count for borders
	lineCount := len(lines)

	// Build left border accent when focused
	var leftBorder string
	if s.focused {
		var leftBorderParts []string
		for i := 0; i < lineCount; i++ {
			leftBorderParts = append(leftBorderParts,
				lipgloss.NewStyle().Foreground(t.Brand).Render("│"))
		}
		leftBorder = lipgloss.NewStyle().Render(strings.Join(leftBorderParts, "\n"))
	}

	// Build right border — use gradient style when focused
	var rightBorderStr string
	if s.focused {
		gradientColors := []lipgloss.Color{t.Brand, t.Accent, t.Brand}
		var rightBorderParts []string
		for i := 0; i < lineCount; i++ {
			colorIdx := i % len(gradientColors)
			rightBorderParts = append(rightBorderParts,
				lipgloss.NewStyle().Foreground(gradientColors[colorIdx]).Render("│"))
		}
		rightBorderStr = strings.Join(rightBorderParts, "\n")
	} else {
		var rightBorderParts []string
		for i := 0; i < lineCount; i++ {
			rightBorderParts = append(rightBorderParts,
				lipgloss.NewStyle().Foreground(t.TextMuted).Render("│"))
		}
		rightBorderStr = strings.Join(rightBorderParts, "\n")
	}
	rightBorder := lipgloss.NewStyle().Render(rightBorderStr)

	// Join with left border (when focused), panel, and right border
	if s.focused {
		return lipgloss.JoinHorizontal(lipgloss.Top, leftBorder, panel, rightBorder)
	}
	return lipgloss.JoinHorizontal(lipgloss.Top, panel, rightBorder)
}

// ─── Sidebar rendering sections ────────────────────────────────────────────────

// renderHeader renders the brand name and version badge.
func (s *SidebarModel) renderHeader(contentW int) []string {
	t := s.theme
	version := s.version
	if version == "" {
		version = "dev"
	}
	title := lipgloss.NewStyle().
		Foreground(t.Brand).
		Bold(true).
		PaddingLeft(1).
		Render("M31A")
	versionBadge := lipgloss.NewStyle().
		Foreground(t.TextMuted).
		Render(" " + version)
	return []string{title + versionBadge, components.SectionDivider{Width: contentW, Theme: t}.Render()}
}

// renderIdle renders the minimal sidebar mode (2-3 lines): branch, cost, task.
func (s *SidebarModel) renderIdle(contentW int) []string {
	t := s.theme
	var lines []string

	// Line 1: branch
	if s.branch != "" {
		branchLine := lipgloss.NewStyle().
			Foreground(t.TextSecondary).
			PaddingLeft(1).
			Width(contentW).
			Render("⎇ " + s.branch)
		lines = append(lines, branchLine)
	}

	// Line 2: cost + elapsed time (if available)
	if s.showCost && s.cost > 0 {
		var costStr string
		if s.cost < 0.01 {
			costStr = "<$0.01"
		} else {
			costStr = fmt.Sprintf("$%.2f", s.cost)
		}
		// Add elapsed time if available
		if !s.taskProgress.StartedAt.IsZero() {
			elapsed := time.Since(s.taskProgress.StartedAt)
			costStr += fmt.Sprintf(" in %s", formatDuration(elapsed))
		}
		costLine := lipgloss.NewStyle().
			Foreground(t.TextMuted).
			PaddingLeft(1).
			Width(contentW).
			Render(costStr)
		lines = append(lines, costLine)
	}

	// Line 3: current task/phase
	if s.currentPhase != "" {
		phaseLine := lipgloss.NewStyle().
			Foreground(t.Brand).
			PaddingLeft(1).
			Width(contentW).
			Render("▸ " + s.currentPhase)
		lines = append(lines, phaseLine)
	}

	return lines
}

// renderActive renders the active sidebar mode (5-7 lines): branch, staged files, in-progress items.
func (s *SidebarModel) renderActive(contentW int) []string {
	t := s.theme
	var lines []string

	// Line 1: branch
	if s.branch != "" {
		branchLine := lipgloss.NewStyle().
			Foreground(t.TextSecondary).
			PaddingLeft(1).
			Width(contentW).
			Render("⎇ " + s.branch)
		lines = append(lines, branchLine)
	}

	// Line 2: file status summary
	modCount, addCount, delCount, untracked := countFileStatuses(s.files)
	if modCount+addCount+delCount+untracked > 0 {
		var pills []string
		if modCount > 0 {
			pills = append(pills, lipgloss.NewStyle().Foreground(t.Warning).Render(fmt.Sprintf("●%d", modCount)))
		}
		if addCount > 0 {
			pills = append(pills, lipgloss.NewStyle().Foreground(t.Success).Render(fmt.Sprintf("+%d", addCount)))
		}
		if delCount > 0 {
			pills = append(pills, lipgloss.NewStyle().Foreground(t.Error).Render(fmt.Sprintf("-%d", delCount)))
		}
		if untracked > 0 {
			pills = append(pills, lipgloss.NewStyle().Foreground(t.TextMuted).Render(fmt.Sprintf("?%d", untracked)))
		}
		lines = append(lines, lipgloss.NewStyle().PaddingLeft(1).Render(strings.Join(pills, " ")))
	}

	// Line 3: current phase
	if s.currentPhase != "" {
		phaseLine := lipgloss.NewStyle().
			Foreground(t.Brand).
			PaddingLeft(1).
			Width(contentW).
			Render("▸ " + s.currentPhase)
		lines = append(lines, phaseLine)
	}

	// Lines 4-5: in-progress tasks (top 2)
	inProgressCount := 0
	for _, item := range s.todoItems {
		if item.Status == "in_progress" && inProgressCount < 2 {
			taskLine := lipgloss.NewStyle().
				Foreground(t.TextMuted).
				PaddingLeft(1).
				Width(contentW).
				Render("  " + truncateStr(item.Content, contentW-4))
			lines = append(lines, taskLine)
			inProgressCount++
		}
	}

	// Line 6: cost + elapsed time (if available)
	if s.showCost && s.cost > 0 {
		var costStr string
		if s.cost < 0.01 {
			costStr = "<$0.01"
		} else {
			costStr = fmt.Sprintf("$%.2f", s.cost)
		}
		// Add elapsed time if available
		if !s.taskProgress.StartedAt.IsZero() {
			elapsed := time.Since(s.taskProgress.StartedAt)
			costStr += fmt.Sprintf(" in %s", formatDuration(elapsed))
		}
		costLine := lipgloss.NewStyle().
			Foreground(t.TextMuted).
			PaddingLeft(1).
			Width(contentW).
			Render(costStr)
		lines = append(lines, costLine)
	}

	// Line 7: pending permissions (if any)
	if s.pendingPermCount > 0 {
		permLine := lipgloss.NewStyle().
			Foreground(t.Warning).
			PaddingLeft(1).
			Width(contentW).
			Render(fmt.Sprintf("! %d pending", s.pendingPermCount))
		lines = append(lines, permLine)
	}

	return lines
}

// truncateStr truncates a string to maxLen, adding "…" if truncated.
func truncateStr(s string, maxLen int) string {
	if maxLen < 1 {
		return ""
	}
	runes := []rune(s)
	if len(runes) <= maxLen {
		return s
	}
	if maxLen == 1 {
		return "…"
	}
	return string(runes[:maxLen-1]) + "…"
}

// formatDuration formats a duration in a human-readable way.
func formatDuration(d time.Duration) string {
	if d < time.Minute {
		return fmt.Sprintf("%ds", int(d.Seconds()))
	}
	if d < time.Hour {
		minutes := int(d.Minutes())
		seconds := int(d.Seconds()) % 60
		if seconds == 0 {
			return fmt.Sprintf("%dm", minutes)
		}
		return fmt.Sprintf("%dm%ds", minutes, seconds)
	}
	hours := int(d.Hours())
	minutes := int(d.Minutes()) % 60
	if minutes == 0 {
		return fmt.Sprintf("%dh", hours)
	}
	return fmt.Sprintf("%dh%dm", hours, minutes)
}

// renderGitStatus renders the git branch and file status pills.
func (s *SidebarModel) renderGitStatus(contentW int) []string {
	t := s.theme
	var lines []string
	if s.branch == "" {
		return lines
	}
	branchLine := lipgloss.NewStyle().
		Foreground(t.TextSecondary).
		PaddingLeft(1).
		Width(contentW).
		Render("⎇ " + s.branch)
	lines = append(lines, branchLine)

	modCount, addCount, delCount, untracked := countFileStatuses(s.files)
	if modCount+addCount+delCount+untracked > 0 {
		var pills []string
		if modCount > 0 {
			pills = append(pills, lipgloss.NewStyle().Foreground(t.Warning).Render(fmt.Sprintf("●%d", modCount)))
		}
		if addCount > 0 {
			pills = append(pills, lipgloss.NewStyle().Foreground(t.Success).Render(fmt.Sprintf("+%d", addCount)))
		}
		if delCount > 0 {
			pills = append(pills, lipgloss.NewStyle().Foreground(t.Error).Render(fmt.Sprintf("-%d", delCount)))
		}
		if untracked > 0 {
			untrackedColor := t.TextMuted
			if untracked >= 5 {
				untrackedColor = t.Warning
			}
			pills = append(pills, lipgloss.NewStyle().Foreground(untrackedColor).Render(fmt.Sprintf("?%d", untracked)))
		}
		lines = append(lines, lipgloss.NewStyle().PaddingLeft(1).Render(strings.Join(pills, " ")))
	}
	return lines
}

// renderTokenUsage renders the token usage section with context gauge, burn rate, and cost.
func (s *SidebarModel) renderTokenUsage(contentW int) []string {
	t := s.theme
	var lines []string
	if s.totalTokens <= 0 {
		return lines
	}
	lines = append(lines, components.SectionDivider{Title: "USAGE", Width: contentW, Theme: t}.Render())

	if s.contextLen > 0 {
		pct := float64(s.totalTokens) / float64(s.contextLen)
		if pct > 1 {
			pct = 1
		}
		const barSegments = 8
		filled := int(pct * barSegments)
		var pressureIcon string
		switch {
		case pct >= 0.9:
			pressureIcon = " !!"
		case pct >= 0.7:
			pressureIcon = " !"
		}
		var bar string
		for i := 0; i < barSegments; i++ {
			var segColor lipgloss.Color
			switch {
			case i >= 7:
				segColor = t.Error
			case i >= 5:
				segColor = t.Warning
			case i >= 3:
				segColor = t.Brand
			default:
				segColor = t.Success
			}
			if i < filled {
				bar += lipgloss.NewStyle().Foreground(segColor).Render("█")
			} else {
				bar += lipgloss.NewStyle().Foreground(t.Border).Render("░")
			}
		}
		lines = append(lines, lipgloss.NewStyle().PaddingLeft(1).Render("["+bar+"] "+fmt.Sprintf("%d%%", int(pct*100))+pressureIcon))
	}

	lines = append(lines, lipgloss.NewStyle().Foreground(t.TextMuted).PaddingLeft(1).Render(formatTokenCountSidebar(s.totalTokens)))

	if s.tokenBurnRate > 0 {
		burnStr := fmt.Sprintf("%.0f tok/s", s.tokenBurnRate)
		if s.tokenBurnCostRate > 0 {
			burnStr += fmt.Sprintf(" · $%.3f/s", s.tokenBurnCostRate)
		}
		lines = append(lines, lipgloss.NewStyle().Foreground(t.Brand).PaddingLeft(1).Render(burnStr))
	}

	if s.showCost && s.cost > 0 {
		var costStr string
		if s.cost < 0.01 {
			costStr = "<$0.01"
		} else {
			costStr = fmt.Sprintf("$%.2f", s.cost)
		}
		if s.costTrend > 0.001 {
			costStr += lipgloss.NewStyle().Foreground(t.Error).Render(" ↑")
		} else if s.costTrend < -0.001 {
			costStr += lipgloss.NewStyle().Foreground(t.Success).Render(" ↓")
		}
		if s.tokenBurnCostRate > 0 {
			costStr += fmt.Sprintf(" ($%.3f/s)", s.tokenBurnCostRate)
		}
		lines = append(lines, lipgloss.NewStyle().Foreground(t.Warning).PaddingLeft(1).Render(costStr))
	}

	if s.modelName != "" {
		modelDisplay := s.modelName
		if len(modelDisplay) > contentW-2 {
			modelDisplay = modelDisplay[:contentW-5] + "…"
		}
		lines = append(lines, lipgloss.NewStyle().Foreground(t.TextMuted).PaddingLeft(1).Render(modelDisplay))
	}
	return lines
}

// renderPhasePipeline renders the workflow phase pipeline indicator.
func (s *SidebarModel) renderPhasePipeline(contentW int) []string {
	t := s.theme
	var lines []string
	if s.mode != SidebarModeTodo || s.currentPhase == "" {
		return lines
	}
	lines = append(lines, components.SectionDivider{Title: "PHASE", Width: contentW, Theme: t}.Render())

	phases := s.GetPhasePipeline()
	var phaseParts []string
	for _, phase := range phases {
		label := strings.ToUpper(phase[:1])
		var styledPhase string
		switch {
		case s.IsPhaseCompleted(phase):
			styledPhase = lipgloss.NewStyle().Foreground(t.Success).Render("✓" + label)
		case phase == s.currentPhase:
			styledPhase = lipgloss.NewStyle().Foreground(t.Brand).Bold(true).Render("●" + label)
		default:
			styledPhase = lipgloss.NewStyle().Foreground(t.TextMuted).Render("○" + label)
		}
		phaseParts = append(phaseParts, styledPhase)
	}
	lines = append(lines, lipgloss.NewStyle().PaddingLeft(1).Render(strings.Join(phaseParts, " ")))

	if !s.phaseStartedAt.IsZero() {
		phaseElapsed := time.Since(s.phaseStartedAt)
		lines = append(lines, lipgloss.NewStyle().Foreground(t.TextMuted).PaddingLeft(1).Render(fmt.Sprintf("%ds in %s", int(phaseElapsed.Seconds()), s.currentPhase)))
	}
	return lines
}

// renderToolTimeline renders the active tool call timeline.
func (s *SidebarModel) renderToolTimeline(contentW int) []string {
	t := s.theme
	var lines []string
	if s.mode != SidebarModeTodo || len(s.toolCalls) == 0 {
		return lines
	}
	lines = append(lines, components.SectionDivider{Title: "TOOLS", Width: contentW, Theme: t}.Render())

	for _, tc := range s.toolCalls {
		var icon string
		var durationStr string
		var durationColor lipgloss.Color
		if tc.Active {
			icon = lipgloss.NewStyle().Foreground(t.Brand).Render("●")
			durationStr = "…"
			durationColor = t.Brand
		} else if tc.Success {
			icon = lipgloss.NewStyle().Foreground(t.Success).Render("✓")
			durationStr = fmt.Sprintf("%dms", tc.Duration.Milliseconds())
			switch {
			case tc.Duration < time.Second:
				durationColor = t.Success
			case tc.Duration < 5*time.Second:
				durationColor = t.Warning
			default:
				durationColor = t.Error
			}
		} else {
			icon = lipgloss.NewStyle().Foreground(t.Error).Render("✗")
			durationStr = fmt.Sprintf("%dms", tc.Duration.Milliseconds())
			durationColor = t.Error
		}
		descDisplay := tc.Description
		if descDisplay == "" {
			descDisplay = tc.Name
		}
		maxDescLen := contentW - 12
		if maxDescLen < 8 {
			maxDescLen = 8
		}
		if len(descDisplay) > maxDescLen {
			descDisplay = descDisplay[:maxDescLen-3] + "…"
		}
		toolLine := lipgloss.NewStyle().Foreground(t.Text).PaddingLeft(1).Render(fmt.Sprintf("%s %-16s ", icon, descDisplay)) +
			lipgloss.NewStyle().Foreground(durationColor).Render(durationStr)
		lines = append(lines, toolLine)
	}
	return lines
}

// renderSpeedMetrics renders execution speed metrics (tasks/min, avg, ETA).
func (s *SidebarModel) renderSpeedMetrics(contentW int) []string {
	t := s.theme
	var lines []string
	if s.mode != SidebarModeTodo || s.tasksPerMinute <= 0 {
		return lines
	}
	lines = append(lines, components.SectionDivider{Title: "SPEED", Width: contentW, Theme: t}.Render())
	lines = append(lines, lipgloss.NewStyle().Foreground(t.TextMuted).PaddingLeft(1).Render(fmt.Sprintf("%.1f tasks/min", s.tasksPerMinute)))
	if s.avgTaskDuration > 0 {
		lines = append(lines, lipgloss.NewStyle().Foreground(t.TextMuted).PaddingLeft(1).Render(fmt.Sprintf("avg %ds/task", int(s.avgTaskDuration.Seconds()))))
	}
	if s.estimatedTimeRem > 0 {
		lines = append(lines, lipgloss.NewStyle().Foreground(t.TextMuted).PaddingLeft(1).Render(fmt.Sprintf("ETA %ds", int(s.estimatedTimeRem.Seconds()))))
	}
	return lines
}

// renderSubAgentBadge renders the sub-agent status badge.
func (s *SidebarModel) renderSubAgentBadge(_ int) []string {
	t := s.theme
	if s.subAgentCount <= 0 {
		return nil
	}
	var agentText string
	if s.subAgentActive > 0 {
		agentText = fmt.Sprintf("⬡ %d/%d agents", s.subAgentActive, s.subAgentCount)
	} else {
		agentText = fmt.Sprintf("⬡ %d agents", s.subAgentCount)
	}
	return []string{lipgloss.NewStyle().Foreground(t.TextMuted).PaddingLeft(1).Render(agentText)}
}

// renderPendingPermBadge shows pending permission request count.
func (s *SidebarModel) renderPendingPermBadge(_ int) []string {
	if s.pendingPermCount <= 0 {
		return nil
	}
	text := fmt.Sprintf("⏱ %d pending approval(s)", s.pendingPermCount)
	return []string{lipgloss.NewStyle().Foreground(s.theme.Warning).PaddingLeft(1).Render(text)}
}

// renderFileTreeOrTodo renders either the file tree or the todo list depending on mode.
func (s *SidebarModel) renderFileTreeOrTodo(contentW int) []string {
	t := s.theme
	var lines []string

	if s.mode == SidebarModeTodo {
		lines = append(lines, components.SectionDivider{Title: "PROGRESS", Width: contentW, Theme: t}.Render())

		pTotal, pDone, pFailed, _ := s.computeProgress()
		if pTotal > 0 || s.taskProgress.Total > 0 {
			total := pTotal
			done := pDone
			failed := pFailed
			if total == 0 {
				total = s.taskProgress.Total
				done = s.taskProgress.Done
				failed = s.taskProgress.Failed
			}
			pct := float64(done+failed) / float64(total)
			if pct > 1 {
				pct = 1
			}
			barW := contentW - 4
			if barW < 8 {
				barW = 8
			}
			filled := int(math.Round(pct * float64(barW)))
			empty := barW - filled

			var barColor lipgloss.Color
			switch {
			case pct >= 1.0 && failed == 0:
				barColor = t.Success
			case failed > 0:
				barColor = t.Error
			default:
				barColor = t.Brand
			}

			bar := lipgloss.NewStyle().Foreground(barColor).Render(strings.Repeat("█", filled)) +
				lipgloss.NewStyle().Foreground(t.Border).Render(strings.Repeat("░", empty))
			pctStr := fmt.Sprintf("%d%%", int(math.Round(pct*100)))
			summary := fmt.Sprintf("%d/%d", done+failed, total)
			if failed > 0 {
				summary += lipgloss.NewStyle().Foreground(t.Error).Render(fmt.Sprintf(" (%d failed)", failed))
			} else if s.taskProgress.Running > 0 && done+failed < total {
				summary += lipgloss.NewStyle().Foreground(t.Warning).Render(fmt.Sprintf(" (%d pending)", total-done-failed))
			}
			lines = append(lines, lipgloss.NewStyle().PaddingLeft(1).Render(bar+" "+pctStr))
			lines = append(lines, lipgloss.NewStyle().PaddingLeft(1).Foreground(t.TextMuted).Render(summary))
		} else {
			lines = append(lines, lipgloss.NewStyle().Foreground(t.TextMuted).PaddingLeft(2).Render("waiting…"))
		}

		if !s.taskProgress.StartedAt.IsZero() {
			elapsed := time.Since(s.taskProgress.StartedAt)
			lines = append(lines, lipgloss.NewStyle().Foreground(t.TextMuted).PaddingLeft(1).Render(fmt.Sprintf("%ds elapsed", int(elapsed.Seconds()))))
		}

		lines = append(lines, components.SectionDivider{Title: "TODO", Width: contentW, Theme: t}.Render())

		if len(s.todoItems) == 0 {
			lines = append(lines, lipgloss.NewStyle().Foreground(t.TextMuted).PaddingLeft(2).Render("no items yet"))
		} else {
			items := s.todoItems
			maxTodoItems := s.height - sidebarFixedOverhead - 6
			if maxTodoItems < 3 {
				maxTodoItems = 3
			}
			if len(items) > maxTodoItems {
				items = items[len(items)-maxTodoItems:]
			}
			for idx, item := range items {
				icon := todoItemIcon(item.Status, t)
				desc := item.Content
				num := fmt.Sprintf("%d.", idx+1)
				var sourceBadge string
				if item.Source != "" && item.Source != "agent" {
					sourceBadge = lipgloss.NewStyle().Foreground(t.TextMuted).Render(" " + item.Source)
				}
				maxDescW := contentW - 10
				if maxDescW < 8 {
					maxDescW = 8
				}
				if lipgloss.Width(desc) > maxDescW {
					desc = desc[:maxDescW-3] + "…"
				}
				descColor := t.Text
				if item.Status == "completed" {
					descColor = t.TextMuted
				}
				lines = append(lines, lipgloss.NewStyle().Foreground(descColor).PaddingLeft(1).Render(fmt.Sprintf("%s %s %s", num, icon, desc)+sourceBadge))
			}
		}
	} else {
		filesLabel := components.SectionDivider{Title: "FILES", Width: contentW, Theme: t}.Render()
		if s.focused {
			filesLabel += lipgloss.NewStyle().Foreground(t.TextMuted).Render(" ↑↓")
		}
		lines = append(lines, filesLabel)

		if len(s.files) == 0 {
			lines = append(lines, lipgloss.NewStyle().Foreground(t.Success).PaddingLeft(2).Width(contentW).Render("✓ clean"))
		} else if s.tree != nil {
			s.tree.Width = contentW
			lines = append(lines, strings.Split(s.tree.View(), "\n")...)
		}
	}
	return lines
}

// renderSession renders the session ID display.
func (s *SidebarModel) renderSession(contentW int) []string {
	t := s.theme
	if s.sessionID == "" {
		return nil
	}
	sessDisplay := s.sessionID
	if len(sessDisplay) > contentW-2 {
		sessDisplay = sessDisplay[:contentW-5] + "…"
	}
	return []string{"", lipgloss.NewStyle().Foreground(t.TextMuted).PaddingLeft(1).Render(sessDisplay)}
}

// renderHints renders keyboard shortcut hints for the current screen.
func (s *SidebarModel) renderHints(_ int) []string {
	t := s.theme
	var hints []string
	switch s.currentScreen {
	case "execute":
		hints = []string{"p:pause", "j/k:scroll"}
	case "plan":
		hints = []string{"a:approve", "r:refine"}
	case "repl":
		hints = []string{"ctrl+b:sidebar", "ctrl+g:focus"}
	case "verify":
		hints = []string{"y:yes", "n:no"}
	case "runtime":
		hints = []string{"enter:continue", "j/k:scroll"}
	case "ship":
		hints = []string{"ctrl+b:sidebar"}
	case "settings":
		hints = []string{"tab:tabs", "e:edit"}
	case "config":
		hints = []string{"tab:tabs", "/:search"}
	case "diff":
		hints = []string{"j/k:scroll", "q:close"}
	case "rollback":
		hints = []string{"enter:revert", "j/k:nav"}
	case "chathistory":
		hints = []string{"j/k:scroll", "enter:view"}
	case "dashboard":
		hints = []string{"enter:select"}
	default:
		hints = []string{"ctrl+b:sidebar"}
	}
	if s.focused {
		hints = append(hints, "esc:unfocus")
	}
	return []string{"", lipgloss.NewStyle().Foreground(t.TextMuted).PaddingLeft(1).Render(strings.Join(hints, " "))}
}
func countFileStatuses(files []git.FileStatus) (mod, add, del, untracked int) {
	for _, f := range files {
		switch {
		case strings.Contains(f.Status, "M"):
			mod++
		case strings.Contains(f.Status, "A"):
			add++
		case strings.Contains(f.Status, "D"):
			del++
		case strings.Contains(f.Status, "?"):
			untracked++
		}
	}
	return
}

// formatTokenCountSidebar formats token count for sidebar display.
func formatTokenCountSidebar(n int) string {
	if n >= 1000 {
		return fmt.Sprintf("%.1fK tokens", float64(n)/1000)
	}
	return fmt.Sprintf("%d tokens", n)
}

// toolCallAction returns a human-readable action description for a tool name.
func toolCallAction(name string) string {
	switch name {
	case "Bash":
		return "Running command"
	case "FileRead":
		return "Reading file"
	case "FileWrite":
		return "Writing file"
	case "Edit":
		return "Editing file"
	case "Glob":
		return "Finding files"
	case "Grep":
		return "Searching code"
	case "WebFetch":
		return "Fetching URL"
	case "WebSearch":
		return "Searching web"
	case "CodeMap":
		return "Mapping code"
	case "FileDelete":
		return "Deleting file"
	case "FileMove":
		return "Moving file"
	case "FileList":
		return "Listing files"
	case "TodoWrite":
		return "Updating tasks"
	case "AskUserQuestion":
		return "Asking question"
	case "Agent":
		return "Spawning agent"
	default:
		return name
	}
}

func todoItemIcon(status string, t theme.Theme) string {
	switch status {
	case "completed":
		return lipgloss.NewStyle().Foreground(t.Success).Render("✓")
	case "in_progress":
		return lipgloss.NewStyle().Foreground(t.Brand).Render("●")
	case "failed":
		return lipgloss.NewStyle().Foreground(t.Error).Render("✗")
	case "cancelled":
		return lipgloss.NewStyle().Foreground(t.TextMuted).Render("—")
	default:
		return lipgloss.NewStyle().Foreground(t.TextMuted).Render("○")
	}
}
