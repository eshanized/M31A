package components

import (
	"fmt"
	"strings"

	"github.com/charmbracelet/bubbles/textarea"
	tea "github.com/charmbracelet/bubbletea"
	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/core/types"
	"github.com/eshanized/M31A/internal/ui/tui/theme"
)

type QuestionModel struct {
	question    string
	header      string
	options     []string
	allowCustom bool
	textarea    textarea.Model
	selected    int
	theme       theme.Theme
	width       int
}

func NewQuestionModel(req types.QuestionRequest, t theme.Theme, width int) QuestionModel {
	ta := textarea.New()
	ta.Placeholder = "Type your answer..."
	ta.Focus()
	ta.CharLimit = 1024
	ta.SetWidth(width - 4)
	ta.SetHeight(3)

	return QuestionModel{
		question:    req.Question,
		header:      req.Header,
		options:     req.Options,
		allowCustom: req.AllowCustom,
		textarea:    ta,
		selected:    -1,
		theme:       t,
		width:       width,
	}
}

func (m *QuestionModel) Init() tea.Cmd {
	return textarea.Blink
}

func (m *QuestionModel) Update(msg tea.Msg) (tea.Model, tea.Cmd) {
	var cmds []tea.Cmd

	switch msg := msg.(type) {
	case tea.KeyMsg:
		switch msg.Type {
		case tea.KeyEnter:
			if m.selected >= 0 && m.selected < len(m.options) {
				// Option selected
				return m, m.submitCmd(m.options[m.selected])
			}
			if m.textarea.Value() != "" {
				return m, m.submitCmd(m.textarea.Value())
			}
		case tea.KeyUp, tea.KeyDown:
			if len(m.options) > 0 {
				if msg.Type == tea.KeyUp {
					m.selected--
					if m.selected < 0 {
						m.selected = len(m.options) - 1
					}
				} else {
					m.selected++
					if m.selected >= len(m.options) {
						m.selected = 0
					}
				}
			}
		case tea.KeyTab:
			if m.selected >= 0 {
				m.textarea.SetValue(m.options[m.selected])
			}
		}
	}

	var cmd tea.Cmd
	m.textarea, cmd = m.textarea.Update(msg)
	cmds = append(cmds, cmd)

	return m, tea.Batch(cmds...)
}

func (m *QuestionModel) View() string {
	var b strings.Builder

	// Header
	if m.header != "" {
		b.WriteString(lipgloss.NewStyle().
			Foreground(m.theme.TextSecondary).
			Bold(true).
			Render(m.header) + "\n\n")
	}

	// Question
	b.WriteString(lipgloss.NewStyle().
		Foreground(m.theme.TextPrimary).
		Bold(true).
		Render(m.question) + "\n\n")

	// Options
	if len(m.options) > 0 {
		for i, opt := range m.options {
			prefix := "  "
			if i == m.selected {
				prefix = "> "
			}
			b.WriteString(prefix + opt + "\n")
		}
		b.WriteString("\n")
		if m.allowCustom {
			b.WriteString(lipgloss.NewStyle().
				Foreground(m.theme.TextSecondary).
				Italic(true).
				Render("Press Tab to fill selected option, or type a custom answer") + "\n\n")
		}
	}

	// Textarea
	b.WriteString(m.textarea.View())

	b.WriteString("\n\n")
	b.WriteString(lipgloss.NewStyle().
		Foreground(m.theme.TextSecondary).
		Italic(true).
		Render("Press Enter to submit"))

	return b.String()
}

func (m *QuestionModel) submitCmd(answer string) tea.Cmd {
	return func() tea.Msg {
		return types.QuestionResponse{Answer: answer}
	}
}

func (m *QuestionModel) Width() int {
	return m.width
}

func (m *QuestionModel) SetWidth(width int) {
	m.width = width
	m.textarea.SetWidth(width - 4)
}

// FormatQuestion formats a question for display in a compact form (e.g., in the REPL).
func FormatQuestion(question, header string, options []string, width int, t theme.Theme, timeoutSecs ...int) string {
	var b strings.Builder

	if header != "" {
		b.WriteString(lipgloss.NewStyle().
			Foreground(t.TextSecondary).
			Bold(true).
			Render(header) + "\n")
	}

	b.WriteString(lipgloss.NewStyle().
		Foreground(t.TextPrimary).
		Bold(true).
		Render(question) + "\n")

	if len(options) > 0 {
		b.WriteString("\n")
		for i, opt := range options {
			fmt.Fprintf(&b, "  %d. %s\n", i+1, opt)
		}
	}

	if len(timeoutSecs) > 0 && timeoutSecs[0] > 0 {
		fmt.Fprintf(&b, "\n  Timeout: %ds — no response uses default", timeoutSecs[0])
	}

	return lipgloss.NewStyle().
		Border(lipgloss.RoundedBorder()).
		BorderForeground(t.Border).
		Padding(1, 2).
		Width(width - 4).
		Render(b.String())
}
