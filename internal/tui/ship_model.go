package tui

import (
	"fmt"
	"strings"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/git"
	"github.com/eshanized/M31A/internal/tui/components"
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
		case "d":
			return sm, func() tea.Msg {
				return AppMsg{Action: "view_ship_diff"}
			}
		}
	}
	return sm, nil
}

// View renders the ship summary screen inside a branded ThinBorder card.
func (sm *ShipModel) View() string {
	t := sm.theme
	w := sm.width
	if w < 50 {
		w = 80
	}

	// ── Stats Grid (2-column layout) ────────────────────────────────────────

	taskIcon := "✓"
	taskColor := t.Success
	if sm.summary.TaskFailed > 0 {
		taskIcon = "✗"
		taskColor = t.Error
	}

	taskStr := lipgloss.NewStyle().Foreground(t.Text).Render(
		fmt.Sprintf("Tasks:")) + " " +
		lipgloss.NewStyle().Foreground(taskColor).Render(
			fmt.Sprintf("%d/%d %s", sm.summary.TaskDone, sm.summary.TaskTotal, taskIcon))

	filesStr := lipgloss.NewStyle().Foreground(t.Text).Render("Files:") + " " +
		lipgloss.NewStyle().Foreground(t.Text).Render(
			fmt.Sprintf("+%d ~%d -%d", sm.summary.FilesAdded, sm.summary.FilesModified, sm.summary.FilesDeleted))

	tokensStr := ""
	if sm.summary.TotalTokens > 0 {
		tokensStr = lipgloss.NewStyle().Foreground(t.Text).Render("Tokens:") + " " +
			lipgloss.NewStyle().Foreground(t.Text).Render(formatSI(sm.summary.TotalTokens))
	}
	costStr := ""
	if sm.summary.TotalCost > 0 {
		costStr = lipgloss.NewStyle().Foreground(t.Text).Render("Cost:") + " " +
			lipgloss.NewStyle().Foreground(t.Warning).Render(fmt.Sprintf("$%.4f", sm.summary.TotalCost))
	}
	durationStr := ""
	if sm.summary.Duration != "" {
		durationStr = lipgloss.NewStyle().Foreground(t.Text).Render("Duration:") + " " +
			lipgloss.NewStyle().Foreground(t.Text).Render(sm.summary.Duration)
	}
	commitsStr := lipgloss.NewStyle().Foreground(t.Text).Render("Commits:") + " " +
		lipgloss.NewStyle().Foreground(t.Text).Render(fmt.Sprintf("%d", len(sm.summary.Commits)))

	colWidth := (w - 8) / 2 // account for border padding
	var rows [][2]string
	if tokensStr != "" {
		rows = [][2]string{
			{taskStr, tokensStr},
			{filesStr, costStr},
			{"", durationStr},
			{"", commitsStr},
		}
	} else {
		rows = [][2]string{
			{taskStr, durationStr},
			{filesStr, commitsStr},
		}
	}
	grid := renderShipStatsGrid(t, rows, colWidth)

	// ── Commit Log ──────────────────────────────────────────────────────────
	var commitLines []string
	for i, c := range sm.summary.Commits {
		if i >= 5 {
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

	var commitBlock string
	if len(commitLines) > 0 {
		commitBlock = strings.Join(commitLines, "\n")
	}

	// ── Card Content ────────────────────────────────────────────────────────
	var cardContent []string
	cardContent = append(cardContent, "")
	cardContent = append(cardContent, grid)
	cardContent = append(cardContent, "")
	if commitBlock != "" {
		cardContent = append(cardContent, commitBlock)
		cardContent = append(cardContent, "")
	}
	cardContent = append(cardContent, lipgloss.NewStyle().Foreground(t.TextMuted).Render("↵ new session  d view diff  q back"))
	cardContent = append(cardContent, "")

	// ── Wrapping Card ───────────────────────────────────────────────────────
	content := strings.Join(cardContent, "\n")
	card := components.Card{
		Title:   "Session Summary",
		Content: content,
		Width:   w - 4,
		Border:  theme.ThinBorder,
		Style:   components.CardBrand,
		Theme:   t,
	}.Render()

	return "  " + card
}
