package tui

import (
	"github.com/charmbracelet/bubbles/textarea"
	tea "github.com/charmbracelet/bubbletea"
	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/tui/theme"
)

// PlanRefineModel provides a text area for the user to enter plan refinement feedback.
type PlanRefineModel struct {
	textarea textarea.Model
	theme    theme.Theme
}

// NewPlanRefineModel creates a PlanRefineModel with a configured textarea.
func NewPlanRefineModel(t theme.Theme, width int) *PlanRefineModel {
	ta := textarea.New()
	ta.Placeholder = "Describe what you'd like to change about the plan..."
	ta.SetWidth(width - 4)
	ta.SetHeight(8)
	ta.ShowLineNumbers = false
	ta.CharLimit = 2000
	ta.Focus()
	return &PlanRefineModel{
		textarea: ta,
		theme:    t,
	}
}

// Update handles refine input key events.
func (pm *PlanRefineModel) Update(msg tea.Msg) (*PlanRefineModel, tea.Cmd) {
	switch msg := msg.(type) {
	case tea.KeyMsg:
		switch msg.String() {
		case "esc", "ctrl+enter":
			return pm, nil
		}
	}
	newTA, cmd := pm.textarea.Update(msg)
	pm.textarea = newTA
	return pm, cmd
}

// View renders the refine input area.
func (pm *PlanRefineModel) View() string {
	t := pm.theme
	header := lipgloss.NewStyle().Foreground(t.Brand).Bold(true).Render("Refine Plan") +
		lipgloss.NewStyle().Foreground(t.TextMuted).Render("  [Ctrl+Enter] Submit  [Esc] Cancel")
	return lipgloss.JoinVertical(lipgloss.Left,
		header,
		"",
		pm.textarea.View(),
	)
}

// Value returns the current textarea content.
func (pm *PlanRefineModel) Value() string {
	return pm.textarea.Value()
}
