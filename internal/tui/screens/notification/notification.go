package notification

import (
	"github.com/eshanized/M31A/internal/tui/tuitypes"
	"strings"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/eshanized/M31A/internal/tui/components"
	"github.com/eshanized/M31A/internal/tui/theme"
)

// NotificationModel shows the notification history.
type NotificationModel struct {
	theme  theme.Theme
	list   components.NotificationList
	width  int
	height int
}

// NewNotificationModel creates a NotificationModel.
func NewNotificationModel(t theme.Theme, w, h int) *NotificationModel {
	return &NotificationModel{
		theme: t,
		list: components.NotificationList{
			Theme:  t,
			Width:  w - 4,
			Height: h - 6,
		},
		width:  w,
		height: h,
	}
}

// AddNotification adds a notification to the history.
func (nm *NotificationModel) AddNotification(text, ntype string) {
	nm.list.Add(text, ntype)
}

// SetTheme updates the theme.
func (nm *NotificationModel) SetTheme(t theme.Theme) {
	nm.theme = t
	nm.list.Theme = t
}

// SetDimensions updates dimensions.
func (nm *NotificationModel) SetDimensions(w, h int) {
	nm.width = w
	nm.height = h
	nm.list.Width = w - 4
	nm.list.Height = h - 6
}

// Init implements tea.Model.
func (nm *NotificationModel) Init() tea.Cmd { return nil }

// Update implements tuitypes.Screenable.
func (nm *NotificationModel) Update(msg tea.Msg) (tea.Model, tea.Cmd) {
	switch msg := msg.(type) {
	case tea.WindowSizeMsg:
		nm.SetDimensions(msg.Width, msg.Height)
	case tea.KeyMsg:
		switch msg.String() {
		case "esc", "q":
			return nm, func() tea.Msg { return tuitypes.PopScreenMsg{} }
		case "up", "k":
			nm.list.MoveCursor(-1)
		case "down", "j":
			nm.list.MoveCursor(1)
		}
	}
	return nm, nil
}

// View implements tea.Model.
func (nm *NotificationModel) View() string {
	t := nm.theme
	title := components.ScreenTitle{Text: "Notification History", Theme: t}.Render()
	footer := components.HintBar{
		Hints: []string{"j/k Navigate", "esc Back"},
		Theme: t,
	}.Render()
	return strings.Join([]string{"", title, "", nm.list.View(), "", footer}, "\n")
}
