package tui

import (
	"fmt"
	"strings"

	"github.com/charmbracelet/bubbles/textarea"
	tea "github.com/charmbracelet/bubbletea"
	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/tui/theme"
)

// GoalInputModel is a full-screen textarea for entering workflow goals.
type GoalInputModel struct {
	theme       theme.Theme
	textarea    textarea.Model
	recentGoals []string
	showRecent  bool
	recentIdx   int
	width       int
	height      int
}

// NewGoalInputModel creates a GoalInputModel.
func NewGoalInputModel(t theme.Theme, recentGoals []string) *GoalInputModel {
	ta := textarea.New()
	ta.Placeholder = "Describe the goal for this coding session..."
	ta.Focus()
	ta.SetWidth(80)
	ta.SetHeight(6)
	ta.CharLimit = 2000

	return &GoalInputModel{
		theme:       t,
		textarea:    ta,
		recentGoals: recentGoals,
	}
}

// SetTheme updates the theme.
func (gi *GoalInputModel) SetTheme(t theme.Theme) {
	gi.theme = t
}

// SetDimensions updates the goal input dimensions.
func (gi *GoalInputModel) SetDimensions(w, h int) {
	gi.width = w
	gi.height = h
	taH := h - 10
	if taH < 3 {
		taH = 3
	}
	gi.textarea.SetWidth(w - 4)
	gi.textarea.SetHeight(taH)
}

// Init implements tea.Model.
func (gi *GoalInputModel) Init() tea.Cmd {
	return textarea.Blink
}

// Update implements tea.Model.
func (gi *GoalInputModel) Update(msg tea.Msg) (tea.Model, tea.Cmd) {
	switch msg := msg.(type) {
	case tea.WindowSizeMsg:
		gi.SetDimensions(msg.Width, msg.Height)
		return gi, nil
	case tea.KeyMsg:
		switch msg.String() {
		case "esc":
			// Cancel → go back to REPL
			return gi, func() tea.Msg {
				return AppMsg{Screen: ScreenREPL}
			}
		case "ctrl+enter":
			// Submit
			goal := strings.TrimSpace(gi.textarea.Value())
			if goal == "" {
				return gi, nil
			}
			return gi, func() tea.Msg {
				return GoalSubmittedMsg{Goal: goal}
			}
		case "ctrl+r":
			// Toggle recent goals panel
			if len(gi.recentGoals) > 0 {
				gi.showRecent = !gi.showRecent
			}
			return gi, nil
		case "up":
			if gi.showRecent && gi.recentIdx > 0 {
				gi.recentIdx--
				return gi, nil
			}
		case "down":
			if gi.showRecent && gi.recentIdx < len(gi.recentGoals)-1 {
				gi.recentIdx++
				return gi, nil
			}
		case "enter":
			if gi.showRecent && len(gi.recentGoals) > 0 {
				gi.textarea.SetValue(gi.recentGoals[gi.recentIdx])
				gi.showRecent = false
				gi.textarea.Focus()
				return gi, nil
			}
		}
	}

	var cmd tea.Cmd
	gi.textarea, cmd = gi.textarea.Update(msg)
	return gi, cmd
}

// View implements tea.Model.
func (gi *GoalInputModel) View() string {
	t := gi.theme
	w := gi.width
	if w < 40 {
		w = 80
	}

	// Title — simple text in brand, no decorative symbols
	title := lipgloss.NewStyle().Foreground(t.Brand).Bold(true).PaddingLeft(2).
		Render("What should M31A do?")

	// Description in muted
	desc := lipgloss.NewStyle().Foreground(t.TextMuted).PaddingLeft(2).
		Render("Describe the goal for this coding session.")

	// Textarea — no border, inherits terminal background
	inputContent := gi.textarea.View()
	textareaBox := lipgloss.NewStyle().
		PaddingLeft(2).
		Width(w - 2).
		Render(inputContent)

	// Footer
	hintsLeft := "ctrl+↵ submit  esc cancel"
	if len(gi.recentGoals) > 0 {
		hintsLeft += "  ctrl+r recent"
	}
	footer := lipgloss.NewStyle().Foreground(t.TextMuted).PaddingLeft(2).
		Render(hintsLeft)

	// Divider
	divider := lipgloss.NewStyle().Foreground(t.TextMuted).
		Render(strings.Repeat("─", w))

	parts := []string{"", title, "", desc, "", textareaBox, ""}

	if gi.showRecent && len(gi.recentGoals) > 0 {
		parts = append(parts, gi.renderRecent())
	}

	parts = append(parts, divider, footer)
	return lipgloss.JoinVertical(lipgloss.Left, parts...)
}

// renderRecent renders the recent goals picker as a numbered list.
func (gi *GoalInputModel) renderRecent() string {
	t := gi.theme
	title := lipgloss.NewStyle().Foreground(t.TextMuted).PaddingLeft(2).
		Render("Recent goals:")
	var rows []string
	for i, g := range gi.recentGoals {
		prefix := "  "
		style := lipgloss.NewStyle().Foreground(t.Text)
		if i == gi.recentIdx {
			prefix = lipgloss.NewStyle().Foreground(t.Brand).Render("▸ ")
			style = style.Foreground(t.Brand).Bold(true)
		}
		rows = append(rows, fmt.Sprintf("%s%s. %s", prefix, lipgloss.NewStyle().Foreground(t.TextMuted).Render(fmt.Sprintf("%d", i+1)), style.Render(TruncateWithEllipsis(g, 60))))
	}
	return lipgloss.JoinVertical(lipgloss.Left,
		"  "+title, strings.Join(rows, "\n"))
}
