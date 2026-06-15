package tui

import (
	tea "github.com/charmbracelet/bubbletea"
	"github.com/eshanized/M31A/internal/tui/components"
	"github.com/eshanized/M31A/internal/tui/theme"
)

// ConfirmQuitModel shows a confirmation dialog when quitting during active processing.
type ConfirmQuitModel struct {
	theme   theme.Theme
	message string
	width   int
	height  int
}

// NewConfirmQuitModel creates a ConfirmQuitModel.
func NewConfirmQuitModel(t theme.Theme, w, h int) *ConfirmQuitModel {
	return &ConfirmQuitModel{
		theme:   t,
		message: "An operation is in progress. Are you sure you want to quit?",
		width:   w,
		height:  h,
	}
}

// SetMessage sets the confirmation message.
func (cq *ConfirmQuitModel) SetMessage(msg string) {
	cq.message = msg
}

// SetTheme updates the theme.
func (cq *ConfirmQuitModel) SetTheme(t theme.Theme) { cq.theme = t }

// SetDimensions updates dimensions.
func (cq *ConfirmQuitModel) SetDimensions(w, h int) {
	cq.width = w
	cq.height = h
}

// Init implements tea.Model.
func (cq *ConfirmQuitModel) Init() tea.Cmd { return nil }

// Update implements tea.Model.
func (cq *ConfirmQuitModel) Update(msg tea.Msg) (tea.Model, tea.Cmd) {
	switch msg := msg.(type) {
	case tea.WindowSizeMsg:
		cq.SetDimensions(msg.Width, msg.Height)
	case tea.KeyMsg:
		switch msg.String() {
		case "y", "Y":
			return cq, tea.Quit
		case "n", "N", "esc":
			return cq, func() tea.Msg { return PopScreenMsg{} }
		}
	}
	return cq, nil
}

// View implements tea.Model.
func (cq *ConfirmQuitModel) View() string {
	w := cq.width
	if w <= 0 {
		w = 80
	}

	// Scale dialog width to terminal size: ideal 50, min 24, max 60
	dialogWidth := w - 4
	if dialogWidth > 60 {
		dialogWidth = 60
	}
	if dialogWidth < 24 {
		dialogWidth = 24
	}

	return components.ConfirmDialog{
		Title:       "Confirm Quit",
		Message:     cq.message,
		ConfirmText: "Yes, quit",
		CancelText:  "No, stay",
		Warning:     true,
		Theme:       cq.theme,
		Width:       dialogWidth,
	}.View(cq.width, cq.height)
}
