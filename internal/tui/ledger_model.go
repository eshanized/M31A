package tui

import (
	"fmt"
	"strings"

	"github.com/charmbracelet/bubbles/viewport"
	tea "github.com/charmbracelet/bubbletea"
	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/tui/components"
	"github.com/eshanized/M31A/internal/tui/theme"
	"github.com/eshanized/M31A/internal/ledger"
)

// LedgerModel displays the cross-session learning ledger.
type LedgerModel struct {
	theme    theme.Theme
	ledger   *ledger.Ledger
	entries  []ledger.LedgerEntry
	viewport viewport.Model
	loaded   bool
	width    int
	height   int
}

// NewLedgerModel creates a LedgerModel backed by the given ledger.
func NewLedgerModel(t theme.Theme, l *ledger.Ledger) *LedgerModel {
	return &LedgerModel{
		theme:  t,
		ledger: l,
	}
}

// SetTheme updates the theme.
func (lm *LedgerModel) SetTheme(t theme.Theme) {
	lm.theme = t
}

// SetDimensions updates the ledger model dimensions.
func (lm *LedgerModel) SetDimensions(w, h int) {
	lm.width = w
	lm.height = h
	vpH := h - 6
	if vpH < 3 {
		vpH = 3
	}
	lm.viewport = viewport.New(w, vpH)
	if lm.loaded {
		lm.viewport.SetContent(lm.renderEntries())
	}
}

// LoadEntries reloads the entries from the ledger.
func (lm *LedgerModel) LoadEntries() {
	if lm.ledger == nil {
		lm.entries = nil
		lm.loaded = true
		return
	}
	lm.entries = lm.ledger.Entries()
	lm.loaded = true
	lm.viewport.SetContent(lm.renderEntries())
}

// Init implements tea.Model.
func (lm *LedgerModel) Init() tea.Cmd { return nil }

// Update implements Screenable.
func (lm *LedgerModel) Update(msg tea.Msg) (tea.Model, tea.Cmd) {
	switch msg := msg.(type) {
	case tea.WindowSizeMsg:
		lm.SetDimensions(msg.Width, msg.Height)
		return lm, nil
	case tea.MouseMsg:
		if msg.Action == tea.MouseActionPress {
			switch msg.Button {
			case tea.MouseButtonWheelUp:
				lm.viewport.LineUp(3)
			case tea.MouseButtonWheelDown:
				lm.viewport.LineDown(3)
			}
		}
	case tea.KeyMsg:
		switch msg.String() {
		case "esc", "q":
			return lm, func() tea.Msg {
				return PopScreenMsg{}
			}
		case "j", "down":
			lm.viewport.LineDown(1)
		case "k", "up":
			lm.viewport.LineUp(1)
		case "r":
			lm.LoadEntries()
		}
	}
	return lm, nil
}

// View implements tea.Model — content only, chrome handled by PageLayout.
func (lm *LedgerModel) View() string {
	t := lm.theme

	body := ""
	if !lm.loaded {
		body = components.LoadingIndicator{Label: "Loading ledger", Theme: t}.Render()
	} else if len(lm.entries) == 0 {
		body = components.InlineEmptyState{
			Message: "No ledger entries yet. Complete a workflow to record your first session.",
			Theme:   t,
		}.Render()
	} else {
		body = lm.viewport.View()
	}

	stats := ""
	if lm.ledger != nil {
		s := lm.ledger.Stats()
		stats = lipgloss.NewStyle().Foreground(t.TextMuted).PaddingLeft(2).
			Render(fmt.Sprintf("%d sessions · avg cost $%.4f · avg %d tasks",
				s.TotalSessions, s.AvgCost, int(s.AvgTaskCount)))
	}

	return lipgloss.JoinVertical(lipgloss.Left, body, "", stats)
}

// renderEntries converts ledger entries to a styled table.
func (lm *LedgerModel) renderEntries() string {
	t := lm.theme
	if len(lm.entries) == 0 {
		return ""
	}

	header := lipgloss.NewStyle().Foreground(t.TextMuted).Bold(true).PaddingLeft(2).
		Render(fmt.Sprintf("%-12s  %-20s  %-14s  %8s  %6s  %8s",
			"Session", "Timestamp", "Model", "Tasks", "Failed", "Cost"))
	divRow := lipgloss.NewStyle().Foreground(t.Border).PaddingLeft(2).
		Render(strings.Repeat("─", 80))

	var rows []string
	rows = append(rows, header, divRow)
	for _, e := range lm.entries {
		ts := e.Timestamp.Format("2006-01-02 15:04")
		modelShort := TruncateWithEllipsis(e.Model, 14)
		row := fmt.Sprintf("  %-12s  %-20s  %-14s  %8d  %6d  $%7.4f",
			TruncateWithEllipsis(e.SessionID, 12),
			ts,
			modelShort,
			e.TaskCount,
			e.FailedTasks,
			e.CostEstimate,
		)
		rows = append(rows, lipgloss.NewStyle().Foreground(t.Text).Render(row))
	}
	return strings.Join(rows, "\n")
}
