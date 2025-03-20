package tui

import (
	"fmt"
	"strings"

	"github.com/charmbracelet/bubbles/list"
	tea "github.com/charmbracelet/bubbletea"
	"github.com/charmbracelet/lipgloss"
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

// sessionInfoToItem converts a SessionInfo to a sessionItem for list display.
func sessionInfoToItem(info session.SessionInfo) sessionItem {
	item := sessionItem{
		id:        info.ID,
		corrupted: info.Corrupted,
	}

	if info.Corrupted {
		item.title = fmt.Sprintf("%s [!]", info.ID)
		item.desc = "Corrupted session data"
	} else {
		item.title = fmt.Sprintf("%s — %s", info.ID, info.Model)
		provider := info.Provider
		if provider == "" {
			provider = "unknown"
		}
		msgCount := info.MessageCount
		started := info.StartedAt.Format("2006-01-02 15:04")
		item.desc = fmt.Sprintf("%s · %d messages · Started: %s", provider, msgCount, started)
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
	delItemID  string // session ID being deleted
	errMsg     string
}

// NewResumeModel creates a ResumeModel with the given theme and session manager.
func NewResumeModel(t theme.Theme, mgr *session.Manager) *ResumeModel {
	delegate := list.NewDefaultDelegate()

	// Style the delegate for our theme
	delegate.Styles.SelectedTitle = delegate.Styles.SelectedTitle.
		Background(lipgloss.Color(t.Brand)).
		Foreground(lipgloss.Color("#FFFFFF"))
	delegate.Styles.SelectedDesc = delegate.Styles.SelectedDesc.
		Background(lipgloss.Color(t.Brand)).
		Foreground(lipgloss.Color("#FFFFFF"))
	delegate.Styles.NormalTitle = delegate.Styles.NormalTitle.
		Background(lipgloss.Color(t.Surface)).
		Foreground(lipgloss.Color(t.TextPrimary))
	delegate.Styles.NormalDesc = delegate.Styles.NormalDesc.
		Background(lipgloss.Color(t.Surface)).
		Foreground(lipgloss.Color(t.TextSecondary))

	items := []list.Item{}
	l := list.New(items, delegate, 0, 0)
	l.Title = "Sessions"
	l.SetShowStatusBar(false)
	l.SetFilteringEnabled(false)

	// Style the title
	l.Styles.Title = lipgloss.NewStyle().
		Foreground(lipgloss.Color(t.Brand)).
		Bold(true).
		Padding(0, 1)

	rm := &ResumeModel{
		list:    l,
		manager: mgr,
		theme:   t,
	}

	// Initial population
	rm.populateList()

	return rm
}

// populateList loads sessions from the manager into the list model.
func (m *ResumeModel) populateList() {
	sessions, err := m.manager.ListSessions()
	if err != nil {
		m.errMsg = fmt.Sprintf("Cannot load sessions: %v", err)
		return
	}
	m.errMsg = ""
	m.list.SetItems(sessionInfoToItems(sessions))
}

// Refresh re-populates the session list from the manager.
func (m *ResumeModel) Refresh() error {
	sessions, err := m.manager.ListSessions()
	if err != nil {
		return err
	}
	m.list.SetItems(sessionInfoToItems(sessions))
	m.errMsg = ""
	return nil
}

// Update handles messages for the resume screen.
func (m *ResumeModel) Update(msg tea.Msg) ([]tea.Cmd, *AppMsg) {
	switch msg := msg.(type) {
	case tea.WindowSizeMsg:
		m.width = msg.Width
		m.height = msg.Height
		h, v := 4, 4 // margins
		m.list.SetSize(msg.Width-h, msg.Height-v)

	case tea.KeyMsg:
		if m.confirmDel {
			// Delete confirmation mode
			switch msg.String() {
			case "y", "Y", "enter":
				if err := m.manager.DeleteSession(m.delTarget); err != nil {
					m.errMsg = fmt.Sprintf("Delete failed: %v", err)
				}
				m.confirmDel = false
				m.delTarget = ""
				m.populateList()
				return nil, nil
			case "n", "N", "esc":
				m.confirmDel = false
				m.delTarget = ""
				return nil, nil
			}
			return nil, nil
		}

		// Normal browsing mode
		switch msg.String() {
		case "enter":
			item := m.list.SelectedItem()
			if item == nil {
				return nil, nil
			}
			si := item.(sessionItem)
			return nil, &AppMsg{Screen: ScreenREPL, SessionID: si.id}
		case "n", "N":
			return nil, &AppMsg{Screen: ScreenFirstRun}
		case "d", "D":
			item := m.list.SelectedItem()
			if item == nil {
				return nil, nil
			}
			si := item.(sessionItem)
			m.confirmDel = true
			m.delTarget = si.id
			return nil, nil
		case "esc":
			return nil, &AppMsg{Screen: ScreenREPL}
		}
	}

	var cmd tea.Cmd
	m.list, cmd = m.list.Update(msg)
	return []tea.Cmd{cmd}, nil
}

// View renders the resume screen.
func (m *ResumeModel) View() string {
	if m.width == 0 || m.height == 0 {
		return "Loading..."
	}

	if m.confirmDel {
		return m.renderDeleteConfirmation()
	}

	return m.renderList()
}

// renderList renders the session list with footer hints.
func (m *ResumeModel) renderList() string {
	var parts []string

	listView := m.list.View()
	parts = append(parts, listView)

	// Error message
	if m.errMsg != "" {
		errStyle := lipgloss.NewStyle().Foreground(lipgloss.Color(m.theme.Error))
		parts = append(parts, "", errStyle.Render(m.errMsg))
	}

	// Footer hints
	footerStyle := lipgloss.NewStyle().Foreground(lipgloss.Color(m.theme.TextSecondary))
	footer := footerStyle.Render("Enter: resume  |  N: new session  |  D: delete  |  Esc: back")
	parts = append(parts, "", footer)

	return strings.Join(parts, "\n")
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
