package tui

import (
	"fmt"
	"strings"
	"time"

	"github.com/charmbracelet/bubbles/list"
	"github.com/charmbracelet/bubbles/textinput"
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
func (i sessionItem) FilterValue() string { return i.title + " " + i.desc }

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

// SessionPreview holds the data shown in the preview pane.
type SessionPreview struct {
	ID            string
	Model         string
	Provider      string
	StartedAt     time.Time
	MessageCount  int
	FirstMessage  string
	WorkflowPhase string
	ProjectGoal   string
	Corrupted     bool
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

	// Search
	searchInput textinput.Model
	searchQuery string
	allSessions []session.SessionInfo // full unfiltered list

	// Preview
	preview *SessionPreview
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

	// Search input
	ti := textinput.New()
	ti.Placeholder = "Search sessions..."
	ti.PromptStyle = lipgloss.NewStyle().Foreground(t.Brand)
	ti.TextStyle = lipgloss.NewStyle().Foreground(t.TextPrimary)
	ti.Cursor.Style = lipgloss.NewStyle().Foreground(t.Brand)
	ti.CharLimit = 80
	ti.Width = 40

	rm := &ResumeModel{
		list:        l,
		manager:     mgr,
		theme:       t,
		searchInput: ti,
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
	m.allSessions = sessions
	m.applySearch()
}

// applySearch filters allSessions based on searchQuery and updates the list.
func (m *ResumeModel) applySearch() {
	query := strings.ToLower(strings.TrimSpace(m.searchQuery))
	if query == "" {
		m.list.SetItems(sessionInfoToItems(m.allSessions))
		return
	}

	var filtered []session.SessionInfo
	for _, s := range m.allSessions {
		haystack := strings.ToLower(s.ID + " " + s.Model + " " + s.Provider)
		if strings.Contains(haystack, query) {
			filtered = append(filtered, s)
		}
	}
	m.list.SetItems(sessionInfoToItems(filtered))
}

// loadPreview loads a session's first message for the preview pane.
func (m *ResumeModel) loadPreview(sessionID string) {
	sess, err := m.manager.LoadSession(sessionID)
	if err != nil {
		m.preview = &SessionPreview{
			ID:        sessionID,
			Corrupted: true,
		}
		return
	}

	firstMsg := ""
	if len(sess.Messages) > 0 {
		content := sess.Messages[0].Content
		if len(content) > 200 {
			content = content[:200] + "..."
		}
		firstMsg = content
	}

	goal := ""
	if sess.Project != nil {
		goal = sess.Project.Goal
	}

	m.preview = &SessionPreview{
		ID:            sess.ID,
		Model:         sess.Model,
		Provider:      sess.Provider,
		StartedAt:     sess.StartedAt,
		MessageCount:  sess.MessageCount,
		FirstMessage:  firstMsg,
		WorkflowPhase: string(sess.WorkflowPhase),
		ProjectGoal:   goal,
	}
}

// Refresh re-populates the session list from the manager.
func (m *ResumeModel) Refresh() error {
	m.populateList()
	m.preview = nil
	return nil
}

// Update handles messages for the resume screen.
func (m *ResumeModel) Update(msg tea.Msg) ([]tea.Cmd, *AppMsg) {
	switch msg := msg.(type) {
	case tea.WindowSizeMsg:
		m.width = msg.Width
		m.height = msg.Height
		h, v := 4, 6 // margins (extra for search bar)
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
				m.preview = nil
				return nil, nil
			case "n", "N", "esc":
				m.confirmDel = false
				m.delTarget = ""
				return nil, nil
			}
			return nil, nil
		}

		// If search input is focused
		if m.searchInput.Focused() {
			switch msg.String() {
			case "enter":
				// Deactivate search, focus list
				m.searchInput.Blur()
				return nil, nil
			case "esc":
				m.searchQuery = ""
				m.searchInput.SetValue("")
				m.applySearch()
				m.searchInput.Blur()
				return nil, nil
			}
			var cmd tea.Cmd
			m.searchInput, cmd = m.searchInput.Update(msg)
			m.searchQuery = m.searchInput.Value()
			m.applySearch()
			return []tea.Cmd{cmd}, nil
		}

		// Normal browsing mode (list focused)
		switch msg.String() {
		case "/":
			// Focus search
			m.searchInput.Focus()
			return nil, nil
		case "enter":
			item := m.list.SelectedItem()
			if item == nil {
				return nil, nil
			}
			si, ok := item.(sessionItem)
			if !ok {
				return nil, nil
			}
			return nil, &AppMsg{Screen: ScreenREPL, SessionID: si.id}
		case "n", "N":
			return nil, &AppMsg{Screen: ScreenFirstRun}
		case "d", "D":
			item := m.list.SelectedItem()
			if item == nil {
				return nil, nil
			}
			si, ok := item.(sessionItem)
			if !ok {
				return nil, nil
			}
			m.confirmDel = true
			m.delTarget = si.id
			return nil, nil
		case "esc":
			return nil, &AppMsg{Screen: ScreenREPL}
		case "up", "down", "k", "j":
			// Update list, then update preview
			var cmd tea.Cmd
			m.list, cmd = m.list.Update(msg)
			// Update preview for selected item
			if sel := m.list.SelectedItem(); sel != nil {
				si, ok := sel.(sessionItem)
				if !ok {
					return []tea.Cmd{cmd}, nil
				}
				if m.preview == nil || m.preview.ID != si.id {
					m.loadPreview(si.id)
				}
			}
			return []tea.Cmd{cmd}, nil
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

	return m.renderBrowser()
}

// renderBrowser renders the search bar + list + preview layout.
func (m *ResumeModel) renderBrowser() string {
	var parts []string

	// Search bar
	searchBar := m.renderSearchBar()
	parts = append(parts, searchBar)

	// Main area: list on left, preview on right (if width allows)
	listView := m.list.View()

	if m.width > 100 && m.preview != nil && !m.preview.Corrupted {
		previewView := m.renderPreview(m.width/2 - 4)
		listWidth := m.width/2 - 2
		previewWidth := m.width/2 - 2

		listStyled := lipgloss.NewStyle().
			Width(listWidth).
			Render(listView)
		previewStyled := lipgloss.NewStyle().
			Width(previewWidth).
			Render(previewView)

		mainContent := lipgloss.JoinHorizontal(lipgloss.Top, listStyled, previewStyled)
		parts = append(parts, mainContent)
	} else {
		parts = append(parts, listView)
	}

	// Error message
	if m.errMsg != "" {
		errStyle := lipgloss.NewStyle().Foreground(lipgloss.Color(m.theme.Error))
		parts = append(parts, "", errStyle.Render(m.errMsg))
	}

	// Footer hints
	footerStyle := lipgloss.NewStyle().Foreground(lipgloss.Color(m.theme.TextSecondary))
	footer := footerStyle.Render("Enter: resume  |  /: search  |  N: new  |  D: delete  |  Esc: back")
	parts = append(parts, "", footer)

	return strings.Join(parts, "\n")
}

// renderSearchBar renders the search input with a label.
func (m *ResumeModel) renderSearchBar() string {
	label := lipgloss.NewStyle().
		Foreground(m.theme.Brand).
		Bold(true).
		Render("> ")

	searchRow := lipgloss.JoinHorizontal(lipgloss.Center, label, m.searchInput.View())

	style := lipgloss.NewStyle().
		Foreground(m.theme.TextSecondary).
		Padding(0, 1).
		Width(m.width - 2)

	return style.Render(searchRow)
}

// renderPreview renders the session preview pane.
func (m *ResumeModel) renderPreview(width int) string {
	if m.preview == nil || m.preview.Corrupted {
		style := lipgloss.NewStyle().
			Foreground(m.theme.TextSecondary).
			Italic(true).
			Width(width).
			Padding(1, 1)
		return style.Render("Select a session to preview")
	}

	p := m.preview
	var lines []string

	// Header
	lines = append(lines, lipgloss.NewStyle().
		Foreground(m.theme.Brand).
		Bold(true).
		Render(p.ID))

	// Model info
	lines = append(lines, lipgloss.NewStyle().
		Foreground(m.theme.TextSecondary).
		Render(fmt.Sprintf("%s · %s", p.Model, p.Provider)))

	// Stats
	lines = append(lines, lipgloss.NewStyle().
		Foreground(m.theme.TextSecondary).
		Render(fmt.Sprintf("%d messages · Started %s", p.MessageCount, p.StartedAt.Format("Jan 02 15:04"))))

	if p.WorkflowPhase != "" {
		lines = append(lines, lipgloss.NewStyle().
			Foreground(m.theme.Brand).
			Render("Phase: "+p.WorkflowPhase))
	}

	if p.ProjectGoal != "" {
		goal := p.ProjectGoal
		if len(goal) > 100 {
			goal = goal[:100] + "..."
		}
		lines = append(lines, "")
		lines = append(lines, lipgloss.NewStyle().
			Foreground(m.theme.TextPrimary).
			Bold(true).
			Render("Goal:"))
		lines = append(lines, lipgloss.NewStyle().
			Foreground(m.theme.TextSecondary).
			Render(goal))
	}

	if p.FirstMessage != "" {
		lines = append(lines, "")
		lines = append(lines, lipgloss.NewStyle().
			Foreground(m.theme.TextPrimary).
			Bold(true).
			Render("First message:"))
		lines = append(lines, lipgloss.NewStyle().
			Foreground(m.theme.TextSecondary).
			Italic(true).
			Render(p.FirstMessage))
	}

	// Border box
	content := strings.Join(lines, "\n")
	boxStyle := lipgloss.NewStyle().
		Border(lipgloss.RoundedBorder()).
		BorderForeground(m.theme.Border).
		Padding(0, 1).
		Width(width).
		Height(16)

	return boxStyle.Render(content)
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
