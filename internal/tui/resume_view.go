package tui

import (
	"fmt"
	"strings"
	"time"

	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/tui/theme"
	"github.com/eshanized/M31A/internal/session"
)

// renderResume renders the session browser screen content.
// Header, footer, and chrome are handled by the unified PageLayout.
func (rm *ResumeModel) renderResume() string {
	t := rm.theme
	w := rm.width
	if w <= 0 {
		w = 80 // only when uninitialized
	}

	// Search input bar
	searchBar := ""
	if rm.searching {
		searchBar = lipgloss.NewStyle().Foreground(t.Brand).PaddingLeft(2).
			Render("🔍 " + rm.searchInput.View())
	}

	if len(rm.sessions) == 0 {
		emptyMsg := "No sessions found. Press  n  to start a new one."
		if rm.searching && rm.searchInput.Value() != "" {
			emptyMsg = "No sessions match your search."
		}
		empty := lipgloss.NewStyle().Foreground(t.TextMuted).PaddingLeft(2).Render(emptyMsg)
		parts := []string{empty}
		if searchBar != "" {
			parts = append([]string{searchBar, ""}, parts...)
		}
		return lipgloss.JoinVertical(lipgloss.Left, parts...)
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

	countInfo := fmt.Sprintf("%d/%d", rm.cursor+1, len(rm.sessions))
	if rm.searching || rm.searchInput.Value() != "" {
		countInfo += fmt.Sprintf(" (filtered from %d)", len(rm.allSessions))
	} else if rm.totalCount > len(rm.sessions) {
		countInfo += fmt.Sprintf(" (showing %d of %d total)", len(rm.sessions), rm.totalCount)
	}
	scrollInfo := lipgloss.NewStyle().Foreground(t.TextMuted).PaddingLeft(2).Render(countInfo)

	hints := lipgloss.NewStyle().Foreground(t.TextMuted).PaddingLeft(2).
		Render("↑↓ navigate · enter resume · / search · r rename · e export · n new · q back")

	parts := []string{
		strings.Join(rows, "\n"),
		"",
		scrollInfo,
		hints,
	}
	if searchBar != "" {
		parts = append([]string{searchBar, ""}, parts...)
	}

	return lipgloss.JoinVertical(lipgloss.Left, parts...)
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

	label := ""
	if info.Label != "" {
		label = " " + lipgloss.NewStyle().Foreground(t.TextSecondary).Render(info.Label)
	}

	row := prefix + idStyle.Render(shortID) + label + "  " + meta
	if w > 0 && lipgloss.Width(row) > w {
		row = lipgloss.NewStyle().MaxWidth(w).Render(row)
	}
	return row
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
