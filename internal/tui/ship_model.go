package tui

import (
	"fmt"
	"strings"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/git"
	"github.com/eshanized/M31A/internal/tui/theme"
)

// ShipSummary carries workflow completion statistics for the Ship screen.
type ShipSummary struct {
	SessionID     string
	Model         string
	Provider      string
	TaskDone      int
	TaskTotal     int
	TaskFailed    int
	TaskSkipped   int
	Commits       []git.CommitInfo
	FilesAdded    int
	FilesModified int
	FilesDeleted  int
	Insertions    int
	Deletions     int
	Duration      string
	TotalTokens   int
	TotalCost     float64
}

// ShipModel displays the workflow completion summary.
type ShipModel struct {
	summary   ShipSummary
	theme     theme.Theme
	sessionID string
	width     int
	height    int
}

// NewShipModel creates a ShipModel.
func NewShipModel(summary ShipSummary, t theme.Theme, w, h int) *ShipModel {
	return &ShipModel{
		summary: summary,
		theme:   t,
		width:   w,
		height:  h,
	}
}

// Update handles ship screen key events.
func (sm *ShipModel) Update(msg tea.Msg) (*ShipModel, tea.Cmd) {
	switch msg := msg.(type) {
	case tea.KeyMsg:
		switch msg.String() {
		case "enter", "n":
			return sm, func() tea.Msg {
				return AppMsg{Screen: ScreenREPL}
			}
		}
	}
	return sm, nil
}

// View renders the ship summary screen.
func (sm *ShipModel) View() string {
	t := sm.theme
	w := sm.width

	// Title
	title := lipgloss.NewStyle().Foreground(t.Success).Bold(true).PaddingLeft(2).
		Render("🚀 Shipped!")

	divider := lipgloss.NewStyle().Foreground(t.TextMuted).Render(strings.Repeat("─", w))

	// Task stats
	taskLine := fmt.Sprintf("  Tasks: %d/%d completed", sm.summary.TaskDone, sm.summary.TaskTotal)
	if sm.summary.TaskFailed > 0 {
		taskLine += fmt.Sprintf("  %d failed", sm.summary.TaskFailed)
	}
	if sm.summary.TaskSkipped > 0 {
		taskLine += fmt.Sprintf("  %d skipped", sm.summary.TaskSkipped)
	}

	// Git stats
	gitLine := ""
	if sm.summary.FilesAdded+sm.summary.FilesModified+sm.summary.FilesDeleted > 0 {
		gitLine = fmt.Sprintf("  Files: +%d ~%d -%d  Lines: +%d -%d",
			sm.summary.FilesAdded,
			sm.summary.FilesModified,
			sm.summary.FilesDeleted,
			sm.summary.Insertions,
			sm.summary.Deletions,
		)
	}

	// Cost/token stats
	costLine := ""
	if sm.summary.TotalTokens > 0 {
		costLine = fmt.Sprintf("  Tokens: %s", formatSI(sm.summary.TotalTokens))
		if sm.summary.TotalCost > 0 {
			costLine += fmt.Sprintf("  Cost: $%.4f", sm.summary.TotalCost)
		}
	}

	// Duration
	durationLine := ""
	if sm.summary.Duration != "" {
		durationLine = fmt.Sprintf("  Duration: %s", sm.summary.Duration)
	}

	// Commits
	var commitLines []string
	for i, c := range sm.summary.Commits {
		if i >= 5 {
			commitLines = append(commitLines, "  ... and more")
			break
		}
		hash := ""
		if len(c.Hash) >= 7 {
			hash = c.Hash[:7]
		}
		commitLines = append(commitLines, fmt.Sprintf("  %s  %s",
			lipgloss.NewStyle().Foreground(t.TextMuted).Render(hash),
			lipgloss.NewStyle().Foreground(t.Text).Render(c.Message),
		))
	}

	footer := lipgloss.NewStyle().Foreground(t.TextMuted).PaddingLeft(2).
		Render("↵ or n  start new session")

	var parts []string
	parts = append(parts, title, divider, "", taskLine)
	if gitLine != "" {
		parts = append(parts, gitLine)
	}
	if costLine != "" {
		parts = append(parts, costLine)
	}
	if durationLine != "" {
		parts = append(parts, durationLine)
	}
	if len(commitLines) > 0 {
		parts = append(parts, "", "  Commits:", strings.Join(commitLines, "\n"))
	}
	parts = append(parts, "", divider, footer)

	return lipgloss.JoinVertical(lipgloss.Left, parts...)
}
