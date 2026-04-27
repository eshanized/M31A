package tui

import (
	"strings"

	"github.com/charmbracelet/bubbles/textinput"
	tea "github.com/charmbracelet/bubbletea"
	"github.com/eshanized/M31A/internal/tui/theme"
	"github.com/eshanized/M31A/pkg/session"
)

// ResumeModel shows the session browser so the user can resume a past session.
type ResumeModel struct {
	theme       theme.Theme
	sessions    []session.SessionInfo
	allSessions []session.SessionInfo // unfiltered list for search reset
	cursor      int
	offset      int
	width       int
	height      int
	searchInput textinput.Model
	searching   bool
}

// NewResumeModel creates a ResumeModel.
func NewResumeModel(sessions []session.SessionInfo, t theme.Theme) *ResumeModel {
	ti := textinput.New()
	ti.Placeholder = "Search sessions..."
	ti.CharLimit = 100

	return &ResumeModel{
		theme:       t,
		sessions:    sessions,
		allSessions: sessions,
		searchInput: ti,
	}
}

// SetTheme updates the theme.
func (rm *ResumeModel) SetTheme(t theme.Theme) {
	rm.theme = t
}

// SetDimensions updates the resume model dimensions.
func (rm *ResumeModel) SetDimensions(w, h int) {
	rm.width = w
	rm.height = h
}

// Refresh replaces the session list (called after re-fetching).
func (rm *ResumeModel) Refresh(sessions []session.SessionInfo) {
	rm.allSessions = sessions
	if rm.searching {
		rm.filterSessions()
	} else {
		rm.sessions = sessions
	}
	if rm.cursor >= len(rm.sessions) {
		rm.cursor = max(0, len(rm.sessions)-1)
	}
}

// filterSessions applies the current search text to filter the session list.
func (rm *ResumeModel) filterSessions() {
	query := strings.ToLower(rm.searchInput.Value())
	if query == "" {
		rm.sessions = rm.allSessions
		return
	}
	var filtered []session.SessionInfo
	for _, s := range rm.allSessions {
		if strings.Contains(strings.ToLower(s.ID), query) ||
			strings.Contains(strings.ToLower(s.Label), query) {
			filtered = append(filtered, s)
		}
	}
	rm.sessions = filtered
	rm.cursor = 0
	rm.offset = 0
}

// Init implements tea.Model.
func (rm *ResumeModel) Init() tea.Cmd {
	return nil
}

// Update implements tea.Model.
func (rm *ResumeModel) Update(msg tea.Msg) (tea.Model, tea.Cmd) {
	switch msg := msg.(type) {
	case tea.WindowSizeMsg:
		rm.width = msg.Width
		rm.height = msg.Height
		return rm, nil
	case tea.KeyMsg:
		if rm.searching {
			switch msg.String() {
			case "esc":
				rm.searching = false
				rm.searchInput.SetValue("")
				rm.sessions = rm.allSessions
				rm.cursor = 0
				rm.offset = 0
				return rm, nil
			case "enter":
				rm.searching = false
				return rm, nil
			default:
				var cmd tea.Cmd
				rm.searchInput, cmd = rm.searchInput.Update(msg)
				rm.filterSessions()
				return rm, cmd
			}
		}
		switch msg.String() {
		case "esc", "q":
			return rm, func() tea.Msg {
				return PopScreenMsg{}
			}
		case "/":
			rm.searching = true
			rm.searchInput.Focus()
			return rm, textinput.Blink
		case "up", "k":
			if rm.cursor > 0 {
				rm.cursor--
				rm.clampScroll()
			}
		case "down", "j":
			if rm.cursor < len(rm.sessions)-1 {
				rm.cursor++
				rm.clampScroll()
			}
		case "enter", " ":
			if len(rm.sessions) > 0 {
				id := rm.sessions[rm.cursor].ID
				return rm, func() tea.Msg {
					return AppMsg{
						Screen:    ScreenREPL,
						SessionID: id,
					}
				}
			}
		case "n":
			return rm, func() tea.Msg {
				return AppMsg{
					Screen: ScreenREPL,
					Action: "new_session",
				}
			}
		case "r":
			// Rename selected session (emitted to AppState for handling)
			if len(rm.sessions) > 0 {
				id := rm.sessions[rm.cursor].ID
				return rm, func() tea.Msg {
					return SessionRenameMsg{SessionID: id}
				}
			}
		case "e":
			// Export selected session
			if len(rm.sessions) > 0 {
				id := rm.sessions[rm.cursor].ID
				return rm, func() tea.Msg {
					return SessionExportMsg{SessionID: id}
				}
			}
		case "d":
			// Show session detail preview
			if len(rm.sessions) > 0 {
				id := rm.sessions[rm.cursor].ID
				return rm, func() tea.Msg {
					return AppMsg{
						Screen:    ScreenSessionDetail,
						SessionID: id,
					}
				}
			}
		}
	}
	return rm, nil
}

// View implements tea.Model — delegates to renderResume.
func (rm *ResumeModel) View() string {
	return rm.renderResume()
}

// clampScroll ensures the cursor stays in the visible viewport.
func (rm *ResumeModel) clampScroll() {
	listH := rm.visibleRows()
	if rm.cursor < rm.offset {
		rm.offset = rm.cursor
	}
	if rm.cursor >= rm.offset+listH {
		rm.offset = rm.cursor - listH + 1
	}
}

// visibleRows returns the number of rows that fit in the visible area.
func (rm *ResumeModel) visibleRows() int {
	h := rm.height - 6
	if h < 3 {
		return 3
	}
	return h
}
