package tui

import (
	"fmt"
	"strings"
	"time"

	"github.com/charmbracelet/bubbles/list"
	"github.com/charmbracelet/bubbles/spinner"
	"github.com/charmbracelet/bubbles/textinput"
	tea "github.com/charmbracelet/bubbletea"
	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/tui/components"
	"github.com/eshanized/M31A/internal/tui/theme"
	"github.com/eshanized/M31A/internal/types"
	"github.com/eshanized/M31A/pkg/session"
)

// sessionItem implements list.DefaultItem for session display.
type sessionItem struct {
	title, desc string
	id          string
	corrupted   bool
	phase       string // workflow phase for badge
	goal        string // project goal for display
	model       string
	provider    string
	startedAt   time.Time
	msgCount    int
	duration    time.Duration
	isActive    bool // most recent session
}

func (i sessionItem) Title() string       { return i.title }
func (i sessionItem) Description() string { return i.desc }
func (i sessionItem) FilterValue() string { return i.title + " " + i.desc }

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
	spinner    spinner.Model

	// Search
	searchInput textinput.Model
	searchQuery string
	allSessions []session.SessionInfo // full unfiltered list

	// Filter chips — narrow the list to sessions in selected workflow phases
	filterChips    components.FilterChips
	chipsFocused   bool

	// Preview
	preview *SessionPreview
}

// NewResumeModel creates a ResumeModel with the given theme and session manager.
func NewResumeModel(t theme.Theme, mgr *session.Manager) *ResumeModel {
	delegate := list.NewDefaultDelegate()

	// Style the delegate for our theme
	delegate.Styles.SelectedTitle = delegate.Styles.SelectedTitle.
		Background(lipgloss.Color(t.Brand)).
		Foreground(t.BadgeTextLight)
	delegate.Styles.SelectedDesc = delegate.Styles.SelectedDesc.
		Background(lipgloss.Color(t.Brand)).
		Foreground(t.BadgeTextLight)
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

	sp := spinner.New()
	sp.Spinner = spinner.Dot
	sp.Style = lipgloss.NewStyle().Foreground(t.Brand)

	chips := components.FilterChips{
		Chips: []components.FilterChip{
			{Label: "all", Active: true},
			{Label: "idle", Active: false},
			{Label: "discuss", Active: false},
			{Label: "plan", Active: false},
			{Label: "execute", Active: false},
			{Label: "verify", Active: false},
			{Label: "ship", Active: false},
		},
		Theme: t,
	}

	rm := &ResumeModel{
		list:        l,
		manager:     mgr,
		theme:       t,
		searchInput: ti,
		spinner:     sp,
		filterChips: chips,
	}

	// Initial population
	rm.populateList()

	return rm
}

func (m *ResumeModel) Init() tea.Cmd {
	return m.spinner.Tick
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

// applySearch filters allSessions by search query and the active filter
// chips, then updates the list. "all" chip means no phase filter.
func (m *ResumeModel) applySearch() {
	query := strings.ToLower(strings.TrimSpace(m.searchQuery))

	// Determine active phase filters. If "all" is active or no specific
	// phase chip is active, skip phase filtering.
	activePhases := m.activePhaseFilters()

	var filtered []session.SessionInfo
	for _, s := range m.allSessions {
		// Phase filter
		if len(activePhases) > 0 && !activePhases[s.WorkflowPhase] {
			continue
		}
		// Text filter
		if query != "" {
			haystack := strings.ToLower(s.ID + " " + s.Model + " " + s.Provider)
			if !strings.Contains(haystack, query) {
				continue
			}
		}
		filtered = append(filtered, s)
	}
	m.list.SetItems(sessionInfoToItems(filtered))
}

// activePhaseFilters returns the set of workflow phases currently selected
// in the filter chips row. Returns nil when no specific phase chip is
// active (either "all" is on, or every chip is off).
func (m *ResumeModel) activePhaseFilters() map[types.WorkflowPhase]bool {
	active := make(map[types.WorkflowPhase]bool)
	for i, chip := range m.filterChips.Chips {
		if !chip.Active {
			continue
		}
		switch i {
		case 0: // "all"
			return nil
		case 1:
			active[types.PhaseIdle] = true
		case 2:
			active[types.PhaseDiscuss] = true
		case 3:
			active[types.PhasePlan] = true
		case 4:
			active[types.PhaseExecute] = true
		case 5:
			active[types.PhaseVerify] = true
		case 6:
			active[types.PhaseShip] = true
		}
	}
	if len(active) == 0 {
		return nil
	}
	return active
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

	case spinner.TickMsg:
		var cmd tea.Cmd
		m.spinner, cmd = m.spinner.Update(msg)
		return []tea.Cmd{cmd}, nil

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

		// If chips row is focused: navigate/toggle, then return.
		if m.chipsFocused {
			switch msg.String() {
			case "tab", "down":
				m.chipsFocused = false
				return nil, nil
			case "left", "h":
				if m.filterChips.Selected > 0 {
					m.filterChips.Selected--
				}
				return nil, nil
			case "right", "l":
				if m.filterChips.Selected < len(m.filterChips.Chips)-1 {
					m.filterChips.Selected++
				}
				return nil, nil
			case " ", "enter":
				idx := m.filterChips.Selected
				if idx == 0 {
					// "all" chip clears other selections
					for i := range m.filterChips.Chips {
						m.filterChips.Chips[i].Active = i == 0
					}
				} else {
					m.filterChips.ToggleChip(idx)
					m.filterChips.Chips[0].Active = false
					if m.filterChips.ActiveCount() == 0 {
						m.filterChips.Chips[0].Active = true
					}
				}
				m.applySearch()
				return nil, nil
			case "esc":
				m.chipsFocused = false
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
		case "f", "F":
			// Focus filter chips row
			m.chipsFocused = true
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

// currentSelectedID returns the ID of the currently selected session in the list.
func (m *ResumeModel) currentSelectedID() string {
	if sel := m.list.SelectedItem(); sel != nil {
		if si, ok := sel.(sessionItem); ok {
			return si.id
		}
	}
	return ""
}
