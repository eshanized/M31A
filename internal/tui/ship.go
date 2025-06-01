package tui

import (
	"fmt"
	"strings"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/git"
	"github.com/eshanized/M31A/internal/tui/theme"
)

// ShipSummary holds session completion data.
type ShipSummary struct {
	TaskDone    int
	TaskTotal   int
	TaskFailed  int
	TaskSkipped int
	Commits     []git.CommitInfo
	Duration    string
	SessionID   string
}

// ShipModel displays the session completion summary.
type ShipModel struct {
	theme   theme.Theme
	summary ShipSummary
	width   int
	height  int
}

// NewShipModel creates a Ship screen model.
func NewShipModel(summary ShipSummary, t theme.Theme) *ShipModel {
	return &ShipModel{
		theme:   t,
		summary: summary,
	}
}

func (m *ShipModel) Update(msg tea.Msg) ([]tea.Cmd, *AppMsg) {
	switch msg := msg.(type) {
	case tea.WindowSizeMsg:
		m.width = msg.Width
		m.height = msg.Height
		return nil, nil

	case tea.KeyMsg:
		switch msg.String() {
		case "n", "N":
			// New session
			return nil, &AppMsg{Screen: ScreenFirstRun}
		case "r", "R":
			return nil, &AppMsg{Screen: ScreenREPL}
		case "esc":
			return nil, &AppMsg{Screen: ScreenREPL}
		case "o", "O":
			// Open in browser (V1: placeholder)
		}
	}
	return nil, nil
}

func (m *ShipModel) View() string {
	if m.width == 0 {
		return "Loading ship..."
	}

	var sb strings.Builder

	// Header
	sb.WriteString(lipgloss.NewStyle().
		Foreground(m.theme.Success).
		Bold(true).
		Render(" ✓ Session complete!"))
	sb.WriteString("\n\n")

	// Session info
	sb.WriteString(fmt.Sprintf("Session: %s\n", m.summary.SessionID))
	sb.WriteString(fmt.Sprintf("Duration: %s\n", m.summary.Duration))
	sb.WriteString("\n")

	// Task summary
	sb.WriteString(fmt.Sprintf("Tasks: %d done, %d failed, %d skipped\n",
		m.summary.TaskDone, m.summary.TaskFailed, m.summary.TaskSkipped))
	sb.WriteString("\n")

	// Commits
	sb.WriteString(lipgloss.NewStyle().
		Bold(true).
		Render("Commits:"))
	sb.WriteString("\n")

	for _, c := range m.summary.Commits {
		sb.WriteString(fmt.Sprintf("  %s %s\n", c.ShortHash, c.Message))
	}

	// Keys
	sb.WriteString("\n")
	sb.WriteString(lipgloss.NewStyle().
		Foreground(m.theme.TextSecondary).
		Render("[O] Open in browser  [N] New session  [R] REPL  [Esc] Back"))

	return sb.String()
}
