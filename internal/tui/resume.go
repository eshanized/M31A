package tui

import (
	"github.com/charmbracelet/bubbles/list"
	tea "github.com/charmbracelet/bubbletea"
	"github.com/eshanized/M31A/internal/tui/theme"
	"github.com/eshanized/M31A/pkg/session"
)

// sessionItem implements list.DefaultItem for session display.
type sessionItem struct {
	title, desc string
	id          string
	corrupted   bool
}

func (i sessionItem) Title() string       { return i.title }
func (i sessionItem) Description() string { return i.desc }
func (i sessionItem) FilterValue() string { return i.title }

// ResumeModel provides a session browser using bubbles/list.
// Initialized in NewApp and active when screen == ScreenResume.
type ResumeModel struct {
	list       list.Model
	manager    *session.Manager
	theme      theme.Theme
	width      int
	height     int
	confirmDel bool   // awaiting delete confirmation
	delTarget  string // session ID to delete
}

// NewResumeModel creates a ResumeModel with the given theme and session manager.
func NewResumeModel(t theme.Theme, mgr *session.Manager) *ResumeModel {
	items := []list.Item{}
	l := list.New(items, list.NewDefaultDelegate(), 0, 0)
	l.Title = "Sessions"
	l.SetShowStatusBar(false)

	return &ResumeModel{
		list:    l,
		manager: mgr,
		theme:   t,
	}
}

// Refresh re-populates the session list from the manager.
func (m *ResumeModel) Refresh() error {
	return nil
}

// Update handles messages for the resume screen.
func (m *ResumeModel) Update(msg tea.Msg) ([]tea.Cmd, *AppMsg) {
	return nil, nil
}

// View renders the resume screen.
func (m *ResumeModel) View() string {
	return "Resume"
}
