package goal_input

import (
	"fmt"
	"strings"

	"github.com/charmbracelet/bubbles/textarea"
	tea "github.com/charmbracelet/bubbletea"
	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/ui/tui/theme"
	"github.com/eshanized/M31A/internal/ui/tui/tuitypes"
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
	// Width/height are set by SetDimensions on first WindowSizeMsg
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

// Update implements tuitypes.Screenable interface.
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
				return tuitypes.AppMsg{Screen: tuitypes.ScreenREPL}
			}
		case "ctrl+enter":
			// Submit
			goal := strings.TrimSpace(gi.textarea.Value())
			if goal == "" {
				return gi, nil
			}
			return gi, func() tea.Msg {
				return tuitypes.GoalSubmittedMsg{Goal: goal}
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

// View implements tea.Model — content only, chrome handled by PageLayout.
func (gi *GoalInputModel) View() string {
	w := gi.width
	if w <= 0 {
		w = 80 // only when uninitialized (not when genuinely narrow)
	}

	// Textarea — no border, inherits terminal background
	inputContent := gi.textarea.View()
	textareaBox := lipgloss.NewStyle().
		PaddingLeft(2).
		Width(w - 2).
		Render(inputContent)

	parts := []string{"", textareaBox}

	if gi.showRecent && len(gi.recentGoals) > 0 {
		parts = append(parts, gi.renderRecent())
	}

	return lipgloss.JoinVertical(lipgloss.Left, parts...)
}

// renderRecent renders the recent goals picker as a numbered list.
func (gi *GoalInputModel) renderRecent() string {
	t := gi.theme
	title := lipgloss.NewStyle().Foreground(t.TextMuted).
		Render("Recent goals:")
	// Truncate goal text relative to actual terminal width
	maxGoalLen := gi.width - 12
	if maxGoalLen < 20 {
		maxGoalLen = 20
	}
	var rows []string
	for i, g := range gi.recentGoals {
		prefix := "  "
		style := lipgloss.NewStyle().Foreground(t.Text)
		if i == gi.recentIdx {
			prefix = lipgloss.NewStyle().Foreground(t.Brand).Render("▸ ")
			style = style.Foreground(t.Brand).Bold(true)
		}
		rows = append(rows, fmt.Sprintf("%s%s. %s", prefix, lipgloss.NewStyle().Foreground(t.TextMuted).Render(fmt.Sprintf("%d", i+1)), style.Render(tuitypes.TruncateWithEllipsis(g, maxGoalLen))))
	}
	return lipgloss.JoinVertical(lipgloss.Left,
		"  "+title, strings.Join(rows, "\n"))
}
