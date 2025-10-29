package tui

import (
	tea "github.com/charmbracelet/bubbletea"
	"github.com/eshanized/M31A/internal/tui/theme"
	"github.com/eshanized/M31A/pkg/session"
)

// ResumeModel shows the session browser so the user can resume a past session.
type ResumeModel struct {
	theme    theme.Theme
	sessions []session.SessionInfo
	cursor   int
	offset   int
	width    int
	height   int
}

// NewResumeModel creates a ResumeModel.
func NewResumeModel(sessions []session.SessionInfo, t theme.Theme) *ResumeModel {
	return &ResumeModel{
		theme:    t,
		sessions: sessions,
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
	rm.sessions = sessions
	if rm.cursor >= len(sessions) {
		rm.cursor = max(0, len(sessions)-1)
	}
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
		switch msg.String() {
		case "esc", "q":
			return rm, func() tea.Msg {
				return AppMsg{Screen: ScreenREPL}
			}
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
			// New session
			return rm, func() tea.Msg {
				return AppMsg{
					Screen: ScreenREPL,
					Action: "new_session",
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

