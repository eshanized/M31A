package tui

import (
	"fmt"
	"strings"
	"time"

	"github.com/charmbracelet/bubbles/list"
	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/pkg/session"
)

// sessionInfoToItem converts a SessionInfo to a sessionItem for list display.
func sessionInfoToItem(info session.SessionInfo) sessionItem {
	item := sessionItem{
		id:        info.ID,
		corrupted: info.Corrupted,
		phase:     string(info.WorkflowPhase),
		model:     info.Model,
		provider:  info.Provider,
		startedAt: info.StartedAt,
		msgCount:  info.MessageCount,
	}

	if info.Corrupted {
		item.title = fmt.Sprintf("%s [!]", info.ID)
		item.desc = "Corrupted session data"
	} else {
		item.title = info.ID
		provider := info.Provider
		if provider == "" {
			provider = "unknown"
		}
		item.desc = fmt.Sprintf("%s · %d messages", provider, info.MessageCount)
	}

	return item
}

// sessionInfoToItems converts a slice of SessionInfo to a slice of list.Item.
func sessionInfoToItems(infos []session.SessionInfo) []list.Item {
	items := make([]list.Item, len(infos))
	for i, info := range infos {
		items[i] = sessionInfoToItem(info)
	}
	return items
}

// SessionPreview holds the data shown in the preview pane.
type SessionPreview struct {
	ID            string
	Model         string
	Provider      string
	StartedAt     time.Time
	MessageCount  int
	FirstMessage  string
	WorkflowPhase string
	ProjectGoal   string
	Corrupted     bool
}

// View renders the resume screen.
func (m *ResumeModel) View() string {
	if m.width == 0 || m.height == 0 {
		return m.spinner.View() + " Loading..."
	}

	if m.confirmDel {
		return m.renderDeleteConfirmation()
	}

	return m.renderBrowser()
}

// renderBrowser renders the search bar + filter chips + timeline view layout.
func (m *ResumeModel) renderBrowser() string {
	var parts []string

	// Search bar
	searchBar := m.renderSearchBar()
	parts = append(parts, searchBar)

	// Filter chips row (above the list)
	chipsRow := m.filterChips.Render()
	if m.chipsFocused {
		// Visual hint: underline when focused
		chipsRow = lipgloss.NewStyle().
			Border(lipgloss.NormalBorder(), false, false, true, false).
			BorderForeground(m.theme.Brand).
			Render(chipsRow)
	}
	labelStyle := lipgloss.NewStyle().Foreground(m.theme.TextSecondary)
	parts = append(parts, labelStyle.Render("Filter: ")+" "+chipsRow)

	// Main area: timeline view
	timelineView := m.renderTimeline()

	if m.width > 100 && m.preview != nil && !m.preview.Corrupted {
		previewView := m.renderPreview(m.width/2 - 4)
		listWidth := m.width/2 - 2
		previewWidth := m.width/2 - 2

		listStyled := lipgloss.NewStyle().
			Width(listWidth).
			Render(timelineView)
		previewStyled := lipgloss.NewStyle().
			Width(previewWidth).
			Render(previewView)

		mainContent := lipgloss.JoinHorizontal(lipgloss.Top, listStyled, previewStyled)
		parts = append(parts, mainContent)
	} else {
		parts = append(parts, timelineView)
	}

	// Error message
	if m.errMsg != "" {
		errStyle := lipgloss.NewStyle().Foreground(lipgloss.Color(m.theme.Error))
		parts = append(parts, "", errStyle.Render(m.errMsg))
	}

	// Footer hints
	footerStyle := lipgloss.NewStyle().Foreground(lipgloss.Color(m.theme.TextSecondary))
	footer := footerStyle.Render("Enter: resume  |  /: search  |  F: filter  |  N: new  |  D: delete  |  Esc: back")
	parts = append(parts, "", footer)

	return strings.Join(parts, "\n")
}

// renderTimeline renders sessions grouped by date with session cards.
func (m *ResumeModel) renderTimeline() string {
	items := m.list.Items()
	if len(items) == 0 {
		emptyStyle := lipgloss.NewStyle().
			Foreground(m.theme.TextSecondary).
			Italic(true).
			Padding(2, 4)
		return emptyStyle.Render("No sessions found")
	}

	// Group sessions by date
	type dateGroup struct {
		label    string
		sessions []sessionItem
	}

	groups := make(map[string]*dateGroup)
	var groupOrder []string

	now := time.Now()
	today := now.Format("2006-01-02")
	yesterday := now.AddDate(0, 0, -1).Format("2006-01-02")

	for _, item := range items {
		si, ok := item.(sessionItem)
		if !ok {
			continue
		}

		var dateKey, label string
		if si.corrupted {
			dateKey = "unknown"
			label = "UNKNOWN"
		} else {
			dateStr := si.startedAt.Format("2006-01-02")
			switch dateStr {
			case today:
				dateKey = today
				label = "TODAY"
			case yesterday:
				dateKey = yesterday
				label = "YESTERDAY"
			default:
				dateKey = dateStr
				label = si.startedAt.Format("Jan 02")
			}
		}

		if _, exists := groups[dateKey]; !exists {
			groups[dateKey] = &dateGroup{label: label}
			groupOrder = append(groupOrder, dateKey)
		}
		groups[dateKey].sessions = append(groups[dateKey].sessions, si)
	}

	// Render groups
	var lines []string
	for _, dateKey := range groupOrder {
		group := groups[dateKey]

		// Date header
		headerStyle := lipgloss.NewStyle().
			Foreground(m.theme.Brand).
			Bold(true)
		lines = append(lines, headerStyle.Render(group.label))
		lines = append(lines, "")

		// Session cards
		for _, si := range group.sessions {
			card := m.renderSessionCard(si)
			lines = append(lines, card)
			lines = append(lines, "")
		}
	}

	return strings.Join(lines, "\n")
}

// renderSessionCard renders a single session as a rounded-border card.
func (m *ResumeModel) renderSessionCard(si sessionItem) string {
	var lines []string

	// Line 1: ▶  ID  ●  model   time  PHASE
	var line1 strings.Builder

	// Selection indicator
	if si.id == m.currentSelectedID() {
		line1.WriteString(lipgloss.NewStyle().Foreground(m.theme.Brand).Render("▶ "))
	} else {
		line1.WriteString("  ")
	}

	// Session ID
	line1.WriteString(lipgloss.NewStyle().Foreground(m.theme.TextPrimary).Render(si.id))

	// Active dot
	line1.WriteString("  ")
	if si.isActive {
		line1.WriteString(lipgloss.NewStyle().Foreground(m.theme.Success).Render("●"))
	} else {
		line1.WriteString(lipgloss.NewStyle().Foreground(m.theme.TextMuted).Render("○"))
	}
	line1.WriteString("  ")

	// Model
	model := si.model
	if model == "" {
		model = "unknown"
	}
	line1.WriteString(lipgloss.NewStyle().Foreground(m.theme.TextPrimary).Render(model))

	// Time
	if !si.startedAt.IsZero() {
		timeStr := si.startedAt.Format("15:04")
		padding := m.width - 8 - lipgloss.Width(line1.String()) - len(timeStr) - 10
		if padding > 0 {
			line1.WriteString(strings.Repeat(" ", padding))
		}
		line1.WriteString(lipgloss.NewStyle().Foreground(m.theme.TextSecondary).Render(timeStr))
	}

	// Phase badge
	phase := si.phase
	if phase == "" {
		phase = "IDLE"
	}
	line1.WriteString("  ")
	line1.WriteString(m.renderPhaseBadge(phase))

	lines = append(lines, line1.String())

	// Line 2: Goal text
	if si.goal != "" {
		goal := si.goal
		if len(goal) > 60 {
			goal = goal[:57] + "..."
		}
		lines = append(lines, "   "+lipgloss.NewStyle().
			Foreground(m.theme.TextSecondary).
			Render("\""+goal+"\""))
	}

	// Line 3: Stats
	provider := si.provider
	if provider == "" {
		provider = "unknown"
	}
	stats := fmt.Sprintf("%d messages · %s", si.msgCount, provider)
	if si.duration > 0 {
		stats += fmt.Sprintf(" · %s", formatDuration(si.duration))
	}
	lines = append(lines, "   "+lipgloss.NewStyle().
		Foreground(m.theme.TextMuted).
		Render(stats))

	// Build card with rounded border
	content := strings.Join(lines, "\n")
	cardStyle := lipgloss.NewStyle().
		Border(lipgloss.RoundedBorder()).
		BorderForeground(m.theme.Border).
		Padding(0, 1).
		Width(m.width - 4)

	return cardStyle.Render(content)
}

// renderPhaseBadge renders a phase badge with progress indicator.
func (m *ResumeModel) renderPhaseBadge(phase string) string {
	var progress string
	switch strings.ToUpper(phase) {
	case "IDLE":
		progress = "░░░░░░░░"
	case "DISCUSS":
		progress = "██░░░░░░"
	case "PLAN":
		progress = "████░░░░"
	case "EXECUTE":
		progress = "██████░░"
	case "VERIFY":
		progress = "███████░"
	case "SHIP":
		progress = "████████"
	default:
		progress = "░░░░░░░░"
	}

	filledStyle := lipgloss.NewStyle().Foreground(m.theme.Brand)
	emptyStyle := lipgloss.NewStyle().Foreground(m.theme.TextMuted)

	// Count filled/empty blocks
	filled := 0
	for _, ch := range progress {
		if ch == '█' {
			filled++
		}
	}
	empty := 8 - filled

	bar := filledStyle.Render(strings.Repeat("█", filled)) +
		emptyStyle.Render(strings.Repeat("░", empty))

	return fmt.Sprintf("%s %s", bar, lipgloss.NewStyle().
		Foreground(m.theme.TextSecondary).
		Render(strings.ToUpper(phase)))
}

// renderSearchBar renders the search input with a label.
func (m *ResumeModel) renderSearchBar() string {
	label := lipgloss.NewStyle().
		Foreground(m.theme.Brand).
		Bold(true).
		Render("> ")

	searchRow := lipgloss.JoinHorizontal(lipgloss.Center, label, m.searchInput.View())

	style := lipgloss.NewStyle().
		Foreground(m.theme.TextSecondary).
		Padding(0, 1).
		Width(m.width - 2)

	return style.Render(searchRow)
}

// renderPreview renders the session preview pane.
func (m *ResumeModel) renderPreview(width int) string {
	if m.preview == nil || m.preview.Corrupted {
		style := lipgloss.NewStyle().
			Foreground(m.theme.TextSecondary).
			Italic(true).
			Width(width).
			Padding(1, 1)
		return style.Render("Select a session to preview")
	}

	p := m.preview
	var lines []string

	// Header
	lines = append(lines, lipgloss.NewStyle().
		Foreground(m.theme.Brand).
		Bold(true).
		Render(p.ID))

	// Model info
	lines = append(lines, lipgloss.NewStyle().
		Foreground(m.theme.TextSecondary).
		Render(fmt.Sprintf("%s · %s", p.Model, p.Provider)))

	// Stats
	lines = append(lines, lipgloss.NewStyle().
		Foreground(m.theme.TextSecondary).
		Render(fmt.Sprintf("%d messages · Started %s", p.MessageCount, p.StartedAt.Format("Jan 02 15:04"))))

	if p.WorkflowPhase != "" {
		lines = append(lines, lipgloss.NewStyle().
			Foreground(m.theme.Brand).
			Render("Phase: "+p.WorkflowPhase))
	}

	if p.ProjectGoal != "" {
		goal := p.ProjectGoal
		if len(goal) > 100 {
			goal = goal[:100] + "..."
		}
		lines = append(lines, "")
		lines = append(lines, lipgloss.NewStyle().
			Foreground(m.theme.TextPrimary).
			Bold(true).
			Render("Goal:"))
		lines = append(lines, lipgloss.NewStyle().
			Foreground(m.theme.TextSecondary).
			Render(goal))
	}

	if p.FirstMessage != "" {
		lines = append(lines, "")
		lines = append(lines, lipgloss.NewStyle().
			Foreground(m.theme.TextPrimary).
			Bold(true).
			Render("First message:"))
		lines = append(lines, lipgloss.NewStyle().
			Foreground(m.theme.TextSecondary).
			Italic(true).
			Render(p.FirstMessage))
	}

	// Border box
	content := strings.Join(lines, "\n")
	boxStyle := lipgloss.NewStyle().
		Border(lipgloss.RoundedBorder()).
		BorderForeground(m.theme.Border).
		Padding(0, 1).
		Width(width).
		Height(16)

	return boxStyle.Render(content)
}

// renderDeleteConfirmation renders the delete confirmation overlay.
func (m *ResumeModel) renderDeleteConfirmation() string {
	prompt := fmt.Sprintf("Delete session %s? (Y/N)", m.delTarget)

	confirmStyle := lipgloss.NewStyle().
		Background(lipgloss.Color(m.theme.SurfaceElevated)).
		Foreground(lipgloss.Color(m.theme.TextPrimary)).
		Padding(1, 2).
		Border(lipgloss.DoubleBorder()).
		BorderForeground(lipgloss.Color(m.theme.Warning))

	rendered := confirmStyle.Render(prompt)
	return lipgloss.Place(m.width, m.height, lipgloss.Center, lipgloss.Center, rendered)
}

// formatDuration formats a duration as a human-readable string.
func formatDuration(d time.Duration) string {
	if d < time.Minute {
		return fmt.Sprintf("%ds", int(d.Seconds()))
	}
	if d < time.Hour {
		return fmt.Sprintf("%dm", int(d.Minutes()))
	}
	h := int(d.Hours())
	m := int(d.Minutes()) % 60
	return fmt.Sprintf("%dh %dm", h, m)
}
