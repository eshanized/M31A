package tui

import (
	"fmt"
	"strings"

	"github.com/charmbracelet/bubbles/viewport"
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
	summary       ShipSummary
	theme         theme.Theme
	sessionID     string
	width         int
	height        int
	demonstration string
	demoViewport  viewport.Model
	showDemo      bool
}

// NewShipModel creates a ShipModel.
func NewShipModel(summary ShipSummary, t theme.Theme, w, h int) *ShipModel {
	sm := &ShipModel{
		summary: summary,
		theme:   t,
		width:   w,
		height:  h,
	}
	sm.demoViewport = viewport.New(w-4, h-8)
	return sm
}

// Init implements Screenable.
func (sm *ShipModel) Init() tea.Cmd { return nil }

// SetDimensions implements Screenable.
func (sm *ShipModel) SetDimensions(w, h int) {
	sm.width = w
	sm.height = h
	sm.demoViewport = viewport.New(w-4, h-8)
	if sm.demonstration != "" {
		sm.demoViewport.SetContent(sm.demonstration)
	}
}

// SetTheme implements Screenable.
func (sm *ShipModel) SetTheme(t theme.Theme) {
	sm.theme = t
}

// SetDemonstration sets the demonstration walkthrough content.
func (sm *ShipModel) SetDemonstration(content string) {
	sm.demonstration = content
	sm.demoViewport.SetContent(content)
}

// Update implements Screenable.
func (sm *ShipModel) Update(msg tea.Msg) (tea.Model, tea.Cmd) {
	switch msg := msg.(type) {
	case tea.WindowSizeMsg:
		sm.width = msg.Width
		sm.height = msg.Height
		sm.demoViewport = viewport.New(sm.width-4, sm.height-8)
		if sm.demonstration != "" {
			sm.demoViewport.SetContent(sm.demonstration)
		}
	case tea.MouseMsg:
		if msg.Action == tea.MouseActionPress {
			switch msg.Button {
			case tea.MouseButtonWheelUp:
				sm.demoViewport.LineUp(3)
			case tea.MouseButtonWheelDown:
				sm.demoViewport.LineDown(3)
			}
		}
	case tea.KeyMsg:
		if sm.showDemo {
			switch msg.String() {
			case "j", "down":
				sm.demoViewport.LineDown(1)
			case "k", "up":
				sm.demoViewport.LineUp(1)
			case "d", "esc", "q":
				sm.showDemo = false
			case "enter", "n":
				return sm, func() tea.Msg {
					return PopScreenMsg{}
				}
			}
			return sm, nil
		}
		switch msg.String() {
		case "enter", "n":
			return sm, func() tea.Msg {
				return PopScreenMsg{}
			}
		case "esc", "q":
			return sm, func() tea.Msg {
				return PopScreenMsg{}
			}
		case "d":
			if sm.demonstration != "" {
				sm.showDemo = true
			} else {
				return sm, func() tea.Msg {
					return AppMsg{Action: "view_ship_diff"}
				}
			}
		}
	}
	return sm, nil
}

// View renders the ship summary screen content.
// Header, footer, and chrome are handled by the unified PageLayout.
func (sm *ShipModel) View() string {
	if sm.showDemo && sm.demonstration != "" {
		header := lipgloss.NewStyle().Foreground(sm.theme.Brand).Bold(true).Render("Walkthrough") +
			lipgloss.NewStyle().Foreground(sm.theme.TextMuted).Render("  [d] Stats  [Esc] Close")
		return lipgloss.JoinVertical(lipgloss.Left, header, sm.demoViewport.View())
	}
	t := sm.theme
	// ── Stats consolidated to 2 lines ─────────────────────────────────────
	taskIcon := "✓"
	taskColor := t.Success
	if sm.summary.TaskFailed > 0 {
		taskIcon = "✗"
		taskColor = t.Error
	}

	// Line 1: Tasks + Files
	line1 := lipgloss.JoinHorizontal(lipgloss.Left,
		lipgloss.NewStyle().Foreground(t.Text).Render("Tasks: ")+
			lipgloss.NewStyle().Foreground(taskColor).Render(
				fmt.Sprintf("%d/%d %s", sm.summary.TaskDone, sm.summary.TaskTotal, taskIcon)),
		"  ",
		lipgloss.NewStyle().Foreground(t.TextMuted).Render("│"),
		"  ",
		lipgloss.NewStyle().Foreground(t.Text).Render("Files: ")+
			lipgloss.NewStyle().Foreground(t.Text).Render(
				fmt.Sprintf("+%d ~%d -%d", sm.summary.FilesAdded, sm.summary.FilesModified, sm.summary.FilesDeleted)),
	)

	// Line 2: Duration + Cost
	var line2Parts []string
	if sm.summary.Duration != "" {
		line2Parts = append(line2Parts,
			lipgloss.NewStyle().Foreground(t.Text).Render("Duration: ")+
				lipgloss.NewStyle().Foreground(t.Text).Render(sm.summary.Duration))
	}
	if sm.summary.TotalCost > 0 {
		line2Parts = append(line2Parts,
			lipgloss.NewStyle().Foreground(t.Text).Render("Cost: ")+
				lipgloss.NewStyle().Foreground(t.Warning).Render(fmt.Sprintf("$%.4f", sm.summary.TotalCost)))
	}
	if sm.summary.TotalTokens > 0 {
		line2Parts = append(line2Parts,
			lipgloss.NewStyle().Foreground(t.Text).Render("Tokens: ")+
				lipgloss.NewStyle().Foreground(t.Text).Render(formatSI(sm.summary.TotalTokens)))
	}
	line2 := strings.Join(line2Parts, "  "+lipgloss.NewStyle().Foreground(t.TextMuted).Render("│")+"  ")

	// ── Commit log as secondary content ────────────────────────────────────
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
			lipgloss.NewStyle().Foreground(t.TextSecondary).Render(c.Message),
		))
	}

	var commitBlock string
	if len(commitLines) > 0 {
		commitHeader := lipgloss.NewStyle().Foreground(t.TextMuted).Render("Commits:")
		commitBlock = commitHeader + "\n" + strings.Join(commitLines, "\n")
	}

	// ── Assemble (no card wrapper) ────────────────────────────────────────
	var parts []string
	parts = append(parts, "")
	parts = append(parts, line1)
	parts = append(parts, line2)
	if commitBlock != "" {
		parts = append(parts, "")
		parts = append(parts, commitBlock)
	}
	parts = append(parts, "")

	return lipgloss.JoinVertical(lipgloss.Left, parts...)
}
