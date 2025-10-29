package tui

import (
	"fmt"
	"strings"
	"time"

	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/tui/theme"
	"github.com/eshanized/M31A/pkg/session"
)

// renderResume renders the session browser screen.
func (rm *ResumeModel) renderResume() string {
	t := rm.theme
	w := rm.width
	if w < 30 {
		w = 80
	}

	title := lipgloss.NewStyle().Foreground(t.Brand).Bold(true).PaddingLeft(1).
		Render("  Resume Session")
	divider := lipgloss.NewStyle().Foreground(t.Border).Render(strings.Repeat("─", w))

	if len(rm.sessions) == 0 {
		empty := lipgloss.NewStyle().Foreground(t.TextMuted).PaddingLeft(2).
			Render("No sessions found. Press  n  to start a new one.")
		footer := lipgloss.NewStyle().Foreground(t.TextMuted).PaddingLeft(2).
			Render("n new session  esc back")
		return lipgloss.JoinVertical(lipgloss.Left,
			title, divider, "", empty, "", divider, footer)
	}

	listH := rm.visibleRows()
	end := rm.offset + listH
	if end > len(rm.sessions) {
		end = len(rm.sessions)
	}
	visible := rm.sessions[rm.offset:end]

	var rows []string
	for i, info := range visible {
		globalIdx := rm.offset + i
		rows = append(rows, renderSessionInfoRow(info, globalIdx == rm.cursor, w, t))
	}

	scrollInfo := lipgloss.NewStyle().Foreground(t.TextMuted).PaddingLeft(2).
		Render(fmt.Sprintf("%d/%d", rm.cursor+1, len(rm.sessions)))

	footer := lipgloss.NewStyle().Foreground(t.TextMuted).PaddingLeft(2).
		Render("↵ resume  n new  ↑↓/jk navigate  esc back")

	return lipgloss.JoinVertical(lipgloss.Left,
		title,
		divider,
		strings.Join(rows, "\n"),
		"",
		scrollInfo,
		divider,
		footer,
	)
}

// renderSessionInfoRow renders a single session row from a SessionInfo.
func renderSessionInfoRow(info session.SessionInfo, selected bool, w int, t theme.Theme) string {
	prefix := "  "
	idStyle := lipgloss.NewStyle().Foreground(t.Text)
	if selected {
		prefix = lipgloss.NewStyle().Foreground(t.Brand).Render("▶ ")
		idStyle = lipgloss.NewStyle().Foreground(t.Brand).Bold(true)
	}

	age := sessionAge(info.StartedAt)
	phase := string(info.WorkflowPhase)
	if phase == "" {
		phase = "idle"
	}
	provider := info.Provider
	if provider == "" {
		provider = "—"
	}

	meta := lipgloss.NewStyle().Foreground(t.TextMuted).
		Render(fmt.Sprintf("%s · %s · %d msgs · %s",
			ProviderShortName(provider), phase, info.MessageCount, age))

	shortID := info.ID
	if len(shortID) > 12 {
		shortID = shortID[:12]
	}

	return prefix + idStyle.Render(shortID) + "  " + meta
}

// sessionAge returns a human-readable age string.
func sessionAge(t time.Time) string {
	since := time.Since(t)
	switch {
	case since < time.Minute:
		return "just now"
	case since < time.Hour:
		return fmt.Sprintf("%dm ago", int(since.Minutes()))
	case since < 24*time.Hour:
		return fmt.Sprintf("%dh ago", int(since.Hours()))
	default:
		return fmt.Sprintf("%dd ago", int(since.Hours()/24))
	}
}
