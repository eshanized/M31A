package tui

import (
	"fmt"
	"strings"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/session"
	"github.com/eshanized/M31A/internal/tui/components"
	"github.com/eshanized/M31A/internal/tui/theme"
)

// SessionDetailModel shows detailed info about a session before loading it.
type SessionDetailModel struct {
	theme  theme.Theme
	sess   *session.Session
	width  int
	height int
}

// NewSessionDetailModel creates a SessionDetailModel.
func NewSessionDetailModel(t theme.Theme, w, h int) *SessionDetailModel {
	return &SessionDetailModel{
		theme:  t,
		width:  w,
		height: h,
	}
}

// SetSession sets the session to display.
func (sd *SessionDetailModel) SetSession(s *session.Session) {
	sd.sess = s
}

// SetTheme updates the theme.
func (sd *SessionDetailModel) SetTheme(t theme.Theme) { sd.theme = t }

// SetDimensions updates dimensions.
func (sd *SessionDetailModel) SetDimensions(w, h int) {
	sd.width = w
	sd.height = h
}

// Init implements tea.Model.
func (sd *SessionDetailModel) Init() tea.Cmd { return nil }

// Update implements Screenable.
func (sd *SessionDetailModel) Update(msg tea.Msg) (tea.Model, tea.Cmd) {
	switch msg := msg.(type) {
	case tea.WindowSizeMsg:
		sd.SetDimensions(msg.Width, msg.Height)
	case tea.KeyMsg:
		switch msg.String() {
		case "esc", "q":
			return sd, func() tea.Msg { return PopScreenMsg{} }
		case "enter":
			if sd.sess != nil {
				return sd, func() tea.Msg {
					return AppMsg{Screen: ScreenREPL, SessionID: sd.sess.ID}
				}
			}
		}
	}
	return sd, nil
}

// View implements tea.Model.
func (sd *SessionDetailModel) View() string {
	t := sd.theme

	if sd.sess == nil {
		return lipgloss.NewStyle().Foreground(t.TextMuted).PaddingLeft(2).
			Render("No session selected.")
	}

	s := sd.sess
	var lines []string

	title := components.ScreenTitle{Text: "Session Detail", Theme: t}.Render()
	lines = append(lines, "", title, "")

	info := []struct{ label, value string }{
		{"ID", s.ID},
		{"Model", s.Model},
		{"Provider", s.Provider},
		{"Phase", string(s.WorkflowPhase)},
		{"Messages", fmt.Sprintf("%d", s.MessageCount)},
	}

	for _, item := range info {
		label := lipgloss.NewStyle().Foreground(t.TextSecondary).Width(12).Render(item.label + ":")
		value := lipgloss.NewStyle().Foreground(t.Text).Render(item.value)
		lines = append(lines, "  "+label+"  "+value)
	}

	// Preview messages
	if len(s.Messages) > 0 {
		lines = append(lines, "",
			lipgloss.NewStyle().Foreground(t.TextSecondary).Bold(true).PaddingLeft(2).
				Render("Message Preview:"))
		maxPreview := 3
		for i, msg := range s.Messages {
			if i >= maxPreview {
				break
			}
			role := lipgloss.NewStyle().Foreground(t.TextMuted).Width(10).Render(msg.Role + ":")
			content := msg.Content
			if len(content) > 60 {
				content = content[:57] + "..."
			}
			lines = append(lines, "  "+role+"  "+
				lipgloss.NewStyle().Foreground(t.Text).Render(content))
		}
	}

	footer := lipgloss.NewStyle().Foreground(t.TextMuted).PaddingLeft(2).
		Render("[enter] Resume   [esc] Back")
	lines = append(lines, "", footer)

	return strings.Join(lines, "\n")
}
