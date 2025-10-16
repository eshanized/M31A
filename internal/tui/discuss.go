package tui

import (
	"fmt"
	"strings"
	"time"

	"github.com/charmbracelet/bubbles/textarea"
	tea "github.com/charmbracelet/bubbletea"
	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/tui/theme"
	"github.com/eshanized/M31A/internal/types"
)

// DiscussModel provides a dedicated Q&A flow for the discuss phase.
type DiscussModel struct {
	theme         theme.Theme
	width         int
	height        int
	questions     []string
	currentIndex  int
	answers       map[int]string
	textInput     textarea.Model
	timer         int // seconds remaining (DefaultPermissionTimeout = 5 min)
	timerActive   bool
	questionCount int
}

// NewDiscussModel creates a DiscussModel with the given questions.
func NewDiscussModel(t theme.Theme, questions []string, width, height int) *DiscussModel {
	ta := textarea.New()
	ta.Placeholder = "Type your answer..."
	ta.Focus()
	ta.CharLimit = 500
	ta.SetWidth(width/2 + width/4)
	ta.SetHeight(4)
	ta.ShowLineNumbers = false
	ta.FocusedStyle.CursorLine = lipgloss.NewStyle().Background(lipgloss.Color(t.Surface))
	ta.FocusedStyle.Text = lipgloss.NewStyle().Foreground(lipgloss.Color(t.TextPrimary))
	ta.FocusedStyle.Placeholder = lipgloss.NewStyle().Foreground(lipgloss.Color(t.TextMuted))

	return &DiscussModel{
		theme:         t,
		questions:     questions,
		questionCount: len(questions),
		currentIndex:  0,
		answers:       make(map[int]string),
		textInput:     ta,
		timer:         types.DefaultPermissionTimeout,
		timerActive:   true,
		width:         width,
		height:        height,
	}
}

// Init returns the textarea blink command.
func (m *DiscussModel) Init() tea.Cmd {
	return textarea.Blink
}

// Update handles messages for the discuss screen.
func (m *DiscussModel) Update(msg tea.Msg) ([]tea.Cmd, *AppMsg) {
	switch msg := msg.(type) {
	case tea.WindowSizeMsg:
		m.width = msg.Width
		m.height = msg.Height
		m.textInput.SetWidth(m.width/2 + m.width/4)
		return nil, nil

	case tea.KeyMsg:
		switch msg.String() {
		case "enter":
			if m.textInput.Focused() {
				answer := strings.TrimSpace(m.textInput.Value())
				if answer != "" {
					m.answers[m.currentIndex] = answer
				}
				m.currentIndex++
				m.textInput.SetValue("")
				m.timer = types.DefaultPermissionTimeout
				if m.currentIndex >= m.questionCount {
					return nil, &AppMsg{Screen: ScreenREPL, Action: "discuss_complete"}
				}
				return []tea.Cmd{textarea.Blink}, nil
			}
		case "tab":
			if m.textInput.Focused() {
				m.textInput.Blur()
			} else {
				m.textInput.Focus()
				return []tea.Cmd{textarea.Blink}, nil
			}
			return nil, nil
		case "esc":
			// Skip all remaining questions
			return nil, &AppMsg{Screen: ScreenREPL, Action: "discuss_complete"}
		case "ctrl+c":
			return nil, &AppMsg{Screen: ScreenREPL, Action: "discuss_cancelled"}
		}

	case timerTickMsg:
		if m.timerActive && m.timer > 0 {
			m.timer--
			if m.timer == 0 {
				// Auto-skip on timeout
				m.currentIndex++
				m.textInput.SetValue("")
				m.timer = types.DefaultPermissionTimeout
				if m.currentIndex >= m.questionCount {
					return nil, &AppMsg{Screen: ScreenREPL, Action: "discuss_complete"}
				}
				return []tea.Cmd{textarea.Blink}, nil
			}
			return []tea.Cmd{m.tickCmd()}, nil
		}
	}

	// Update text input
	if m.textInput.Focused() {
		var cmd tea.Cmd
		m.textInput, cmd = m.textInput.Update(msg)
		return []tea.Cmd{cmd}, nil
	}

	return nil, nil
}

// timerTickMsg is a custom message for the timer countdown.
type timerTickMsg struct{}

// tickCmd returns a tea.Cmd that emits a timerTickMsg after 1 second.
func (m *DiscussModel) tickCmd() tea.Cmd {
	return tea.Tick(time.Second, func(t time.Time) tea.Msg {
		return timerTickMsg{}
	})
}

// View renders the discuss screen.
func (m *DiscussModel) View() string {
	if m.currentIndex >= m.questionCount {
		return centerScreen("All questions answered. Advancing to plan phase...", m.width, m.height)
	}

	var parts []string

	// Header
	headerStyle := lipgloss.NewStyle().
		Foreground(m.theme.Brand).
		Bold(true).
		Padding(0, 1)
	parts = append(parts, headerStyle.Render(fmt.Sprintf("Discuss — Question %d/%d", m.currentIndex+1, m.questionCount)))

	// Progress bar
	parts = append(parts, "")
	parts = append(parts, m.renderProgressBar())
	parts = append(parts, "")

	// Timer
	timerColor := m.theme.TextSecondary
	if m.timer < 60 {
		timerColor = m.theme.Error
	}
	timerStr := fmt.Sprintf("⏱ %02d:%02d remaining", m.timer/60, m.timer%60)
	parts = append(parts, lipgloss.NewStyle().Foreground(lipgloss.Color(timerColor)).Render(timerStr))
	parts = append(parts, "")

	// Question
	question := m.questions[m.currentIndex]
	questionStyle := lipgloss.NewStyle().
		Foreground(lipgloss.Color(m.theme.Brand)).
		Bold(true).
		Padding(0, 2).
		Width(m.width - 4)
	parts = append(parts, questionStyle.Render(question))
	parts = append(parts, "")

	// Text input
	inputStyle := lipgloss.NewStyle().
		Border(lipgloss.NormalBorder()).
		BorderForeground(m.theme.Border).
		Padding(0, 1).
		Width(m.width/2 + m.width/4 + 4)
	parts = append(parts, inputStyle.Render(m.textInput.View()))
	parts = append(parts, "")

	// Footer
	footerStyle := lipgloss.NewStyle().Foreground(m.theme.TextSecondary)
	footer := footerStyle.Render("[Enter] Submit · [Tab] Skip question · [Esc] Skip all")
	parts = append(parts, footer)

	return centerScreen(strings.Join(parts, "\n"), m.width, m.height)
}

// renderProgressBar renders a progress indicator.
func (m *DiscussModel) renderProgressBar() string {
	total := 30 // max width of progress bar
	filled := 0
	if m.questionCount > 0 {
		filled = (m.currentIndex * total) / m.questionCount
	}
	empty := total - filled

	bar := lipgloss.NewStyle().Foreground(m.theme.Brand).Render(strings.Repeat("█", filled)) +
		lipgloss.NewStyle().Foreground(m.theme.TextMuted).Render(strings.Repeat("░", empty))

	return fmt.Sprintf("  %s  %d/%d", bar, m.currentIndex, m.questionCount)
}

// GetAnswers returns the collected answers.
func (m *DiscussModel) GetAnswers() map[int]string {
	return m.answers
}

// Skipped returns whether the user skipped remaining questions.
func (m *DiscussModel) Skipped() bool {
	return m.currentIndex > 0 && m.currentIndex < m.questionCount
}
