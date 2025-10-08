package tui

import (
	"github.com/charmbracelet/bubbles/spinner"
	tea "github.com/charmbracelet/bubbletea"
	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/git"
	"github.com/eshanized/M31A/internal/tui/theme"
)

// ShipSummary holds session completion data.
type ShipSummary struct {
	TaskDone      int
	TaskTotal     int
	TaskFailed    int
	TaskSkipped   int
	Commits       []git.CommitInfo
	Duration      string
	SessionID     string
	FilesAdded    int
	FilesModified int
	FilesDeleted  int
	Insertions    int
	Deletions     int
	TotalTokens   int
	TotalCost     float64
	Model         string
	Provider      string
}

// ShipModel displays the session completion summary.
type ShipModel struct {
	theme            theme.Theme
	summary          ShipSummary
	width            int
	height           int
	spinner          spinner.Model
	confirmNewSession bool
	sessionID        string
}

// NewShipModel creates a Ship screen model.
func NewShipModel(summary ShipSummary, t theme.Theme, width, height int) *ShipModel {
	sp := spinner.New()
	sp.Spinner = spinner.Dot
	sp.Style = lipgloss.NewStyle().Foreground(t.Brand)
	return &ShipModel{
		theme:   t,
		summary: summary,
		width:   width,
		height:  height,
		spinner: sp,
	}
}

func (m *ShipModel) Init() tea.Cmd {
	return m.spinner.Tick
}

func (m *ShipModel) Update(msg tea.Msg) ([]tea.Cmd, *AppMsg) {
	switch msg := msg.(type) {
	case tea.WindowSizeMsg:
		m.width = msg.Width
		m.height = msg.Height
		return nil, nil

	case spinner.TickMsg:
		var cmd tea.Cmd
		m.spinner, cmd = m.spinner.Update(msg)
		return []tea.Cmd{cmd}, nil

	case tea.KeyMsg:
		switch msg.String() {
		case "n", "N":
			if m.confirmNewSession {
				return nil, &AppMsg{Screen: ScreenREPL, Action: "new_session"}
			}
			m.confirmNewSession = true
			return nil, nil
		case "y", "Y":
			if m.confirmNewSession {
				return nil, &AppMsg{Screen: ScreenREPL, Action: "new_session"}
			}
		case "esc":
			if m.confirmNewSession {
				m.confirmNewSession = false
				return nil, nil
			}
			return nil, &AppMsg{Screen: ScreenREPL}
		case "r", "R":
			return nil, &AppMsg{Screen: ScreenREPL}
		}
	}
	return nil, nil
}
