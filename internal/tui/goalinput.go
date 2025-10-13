package tui

import (
	"fmt"
	"strings"

	"github.com/charmbracelet/bubbles/textarea"
	"github.com/charmbracelet/bubbles/textinput"
	tea "github.com/charmbracelet/bubbletea"
	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/tui/theme"
)

// GoalInputModel is the full-screen goal entry view for /workflow start.
// It shows a large centered textarea with brand border + recent goals quick-select.
type GoalInputModel struct {
	theme        theme.Theme
	width        int
	height       int
	textarea     textarea.Model
	searchInput  textinput.Model
	recentGoals  []string // last N goals from session history
	selectedGoal int      // -1 = none selected
	focused      string   // "textarea" | "recent"
}

// NewGoalInputModel creates a GoalInputModel.
func NewGoalInputModel(t theme.Theme, recentGoals []string) *GoalInputModel {
	ta := textarea.New()
	ta.Placeholder = "Describe your goal… e.g. \"Refactor the payment service to use Stripe v14\""
	ta.CharLimit = 2000
	ta.ShowLineNumbers = false
	ta.SetWidth(70)
	ta.SetHeight(6)
	ta.Focus()

	ti := textinput.New()
	ti.Placeholder = "Filter recent goals..."
	ti.CharLimit = 80

	m := &GoalInputModel{
		theme:        t,
		textarea:     ta,
		searchInput:  ti,
		recentGoals:  recentGoals,
		selectedGoal: -1,
		focused:      "textarea",
	}
	return m
}

func (m *GoalInputModel) Init() tea.Cmd {
	return textarea.Blink
}

// Goal returns the current text in the textarea.
func (m *GoalInputModel) Goal() string {
	return strings.TrimSpace(m.textarea.Value())
}

func (m *GoalInputModel) Update(msg tea.Msg) ([]tea.Cmd, *AppMsg) {
	switch msg := msg.(type) {
	case tea.WindowSizeMsg:
		m.width = msg.Width
		m.height = msg.Height
		// Resize textarea to 60% of terminal width, up to 80 cols
		tw := m.width*6/10 - 4
		if tw > 80 {
			tw = 80
		}
		if tw < 40 {
			tw = 40
		}
		m.textarea.SetWidth(tw)
		m.textarea.SetHeight(6)

	case tea.KeyMsg:
		switch msg.String() {
		case "esc":
			// Cancel — go back to REPL
			return nil, &AppMsg{Screen: ScreenREPL}

		case "ctrl+enter", "alt+enter":
			// Submit the goal — route through PhaseInitialize before Discuss
			goal := m.Goal()
			if goal == "" {
				return nil, nil
			}
			return nil, &AppMsg{
				Action: "goal_submitted",
			}

		case "tab":
			// Toggle focus between textarea and recent goals list
			if len(m.recentGoals) > 0 {
				if m.focused == "textarea" {
					m.focused = "recent"
					m.textarea.Blur()
					m.selectedGoal = 0
				} else {
					m.focused = "textarea"
					m.textarea.Focus()
					m.selectedGoal = -1
				}
			}
			return nil, nil

		case "enter":
			// In recent goals mode: inject selected goal into textarea
			if m.focused == "recent" && m.selectedGoal >= 0 && m.selectedGoal < len(m.recentGoals) {
				m.textarea.SetValue(m.recentGoals[m.selectedGoal])
				m.focused = "textarea"
				m.textarea.Focus()
				m.selectedGoal = -1
				return nil, nil
			}

		case "up", "k":
			if m.focused == "recent" && m.selectedGoal > 0 {
				m.selectedGoal--
			}
			return nil, nil

		case "down", "j":
			if m.focused == "recent" && m.selectedGoal < len(m.recentGoals)-1 {
				m.selectedGoal++
			}
			return nil, nil
		}
	}

	// Forward to textarea
	if m.focused == "textarea" {
		var cmd tea.Cmd
		m.textarea, cmd = m.textarea.Update(msg)
		return []tea.Cmd{cmd}, nil
	}
	return nil, nil
}

func (m *GoalInputModel) View() string {
	if m.width == 0 {
		return "Loading goal input..."
	}

	t := m.theme

	// Title
	title := lipgloss.JoinHorizontal(lipgloss.Top,
		lipgloss.NewStyle().Foreground(t.Brand).Bold(true).Render("▓▓▓ M31A ▓"),
		lipgloss.NewStyle().Foreground(t.TextMuted).Render(" › "),
		lipgloss.NewStyle().Foreground(t.TextPrimary).Bold(true).Render("New Workflow Goal"),
	)

	subtitle := lipgloss.NewStyle().Foreground(t.TextSecondary).
		Render("Describe what you want M31A to build or fix. Be specific.")

	// Textarea with brand border
	taStyle := lipgloss.NewStyle().
		Border(lipgloss.RoundedBorder()).
		BorderForeground(t.Brand).
		Background(t.Surface).
		Padding(0, 1)

	inputBox := taStyle.Render(m.textarea.View())

	// Keyboard hints
	hints := lipgloss.JoinHorizontal(lipgloss.Left,
		lipgloss.NewStyle().Foreground(t.Brand).Bold(true).Render("Ctrl+Enter"),
		lipgloss.NewStyle().Foreground(t.TextMuted).Render(" submit  ·  "),
		lipgloss.NewStyle().Foreground(t.Brand).Bold(true).Render("Esc"),
		lipgloss.NewStyle().Foreground(t.TextMuted).Render(" cancel  ·  "),
		lipgloss.NewStyle().Foreground(t.Brand).Bold(true).Render("Tab"),
		lipgloss.NewStyle().Foreground(t.TextMuted).Render(" recent goals"),
	)

	// Assemble main content
	contentParts := []string{
		title,
		"",
		subtitle,
		"",
		inputBox,
		"",
		hints,
	}

	// Recent goals list (shown below when available)
	if len(m.recentGoals) > 0 {
		contentParts = append(contentParts, "")
		contentParts = append(contentParts,
			lipgloss.NewStyle().Foreground(t.TextSecondary).Bold(true).Render("Recent goals:"))

		limit := len(m.recentGoals)
		if limit > 5 {
			limit = 5
		}
		for i := 0; i < limit; i++ {
			goal := m.recentGoals[i]
			if len(goal) > 70 {
				goal = goal[:67] + "..."
			}
			var line string
			if m.focused == "recent" && i == m.selectedGoal {
				line = lipgloss.NewStyle().
					Foreground(t.Brand).
					Bold(true).
					Render(fmt.Sprintf("  ▶ %s", goal))
			} else {
				line = lipgloss.NewStyle().
					Foreground(t.TextMuted).
					Render(fmt.Sprintf("    %s", goal))
			}
			contentParts = append(contentParts, line)
		}
	}

	content := lipgloss.JoinVertical(lipgloss.Center, contentParts...)
	return lipgloss.Place(m.width, m.height, lipgloss.Center, lipgloss.Center, content)
}
