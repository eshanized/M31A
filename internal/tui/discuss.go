package tui

import (
	"fmt"
	"strings"
	"time"

	"github.com/charmbracelet/bubbles/textinput"
	tea "github.com/charmbracelet/bubbletea"
	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/tui/theme"
)

// DiscussModel presents discuss Q&A questions one-by-one.
type DiscussModel struct {
	theme     theme.Theme
	questions []string
	current   int
	answers   []string
	input     textinput.Model
	timeout   int // seconds, 0 = no timeout
	deadline  time.Time
	hasDeadline bool
	width     int
	height    int
}

// NewDiscussModel creates a DiscussModel for the given questions.
func NewDiscussModel(t theme.Theme, questions []string, w, h int) *DiscussModel {
	ti := textinput.New()
	ti.Placeholder = "Type your answer..."
	ti.CharLimit = 500
	ti.Focus()

	answers := make([]string, len(questions))

	return &DiscussModel{
		theme:     t,
		questions: questions,
		answers:   answers,
		input:     ti,
		width:     w,
		height:    h,
	}
}

// SetTheme updates the theme.
func (dm *DiscussModel) SetTheme(t theme.Theme) {
	dm.theme = t
}

// SetDimensions updates the discuss model dimensions.
func (dm *DiscussModel) SetDimensions(w, h int) {
	dm.width = w
	dm.height = h
}

// SetTimeout configures the per-question timeout.
func (dm *DiscussModel) SetTimeout(secs int) {
	dm.timeout = secs
	if secs > 0 {
		dm.hasDeadline = true
		dm.deadline = time.Now().Add(time.Duration(secs) * time.Second)
	}
}

// Init implements tea.Model.
func (dm *DiscussModel) Init() tea.Cmd {
	return textinput.Blink
}

// Update implements tea.Model.
func (dm *DiscussModel) Update(msg tea.Msg) (tea.Model, tea.Cmd) {
	switch msg := msg.(type) {
	case tea.WindowSizeMsg:
		dm.width = msg.Width
		dm.height = msg.Height
		return dm, nil

	case DiscussAnswerTimeoutMsg:
		if msg.QuestionIndex == dm.current {
			return dm, dm.advanceQuestion("")
		}
		return dm, nil

	case tea.KeyMsg:
		switch msg.String() {
		case "esc":
			// Skip current question
			return dm, dm.advanceQuestion("")
		case "ctrl+s":
			// Skip all remaining
			return dm, func() tea.Msg {
				return AppMsg{Screen: ScreenREPL}
			}
		case "enter":
			ans := strings.TrimSpace(dm.input.Value())
			return dm, dm.advanceQuestion(ans)
		default:
			var cmd tea.Cmd
			dm.input, cmd = dm.input.Update(msg)
			return dm, cmd
		}
	}

	var cmd tea.Cmd
	dm.input, cmd = dm.input.Update(msg)
	return dm, cmd
}

// advanceQuestion records the answer and moves to next question or emits final msg.
func (dm *DiscussModel) advanceQuestion(answer string) tea.Cmd {
	if dm.current < len(dm.questions) {
		dm.answers[dm.current] = answer
	}

	qIdx := dm.current
	ans := answer

	// Emit answer message
	answerCmd := func() tea.Msg {
		return QuestionResponseMsg{Answer: ans}
	}
	_ = qIdx

	dm.current++
	dm.input.SetValue("")
	dm.input.Focus()

	if dm.hasDeadline {
		dm.deadline = time.Now().Add(time.Duration(dm.timeout) * time.Second)
	}

	if dm.current >= len(dm.questions) {
		// All done
		return func() tea.Msg {
			return AppMsg{Screen: ScreenREPL}
		}
	}
	return answerCmd
}

// View implements tea.Model.
func (dm *DiscussModel) View() string {
	t := dm.theme
	w := dm.width
	if w < 30 {
		w = 80
	}

	if len(dm.questions) == 0 {
		return lipgloss.NewStyle().Foreground(t.TextMuted).
			Render("No questions to answer.")
	}

	progress := fmt.Sprintf("Question %d of %d", dm.current+1, len(dm.questions))
	progressBar := lipgloss.NewStyle().Foreground(t.Brand).Bold(true).PaddingLeft(1).
		Render(progress)

	divider := lipgloss.NewStyle().Foreground(t.Border).Render(strings.Repeat("─", w))

	question := ""
	if dm.current < len(dm.questions) {
		question = dm.questions[dm.current]
	}

	qBox := lipgloss.NewStyle().
		Foreground(t.Text).
		PaddingLeft(2).
		Width(w - 4).
		Render(question)

	inputBox := lipgloss.NewStyle().
		Border(lipgloss.RoundedBorder()).
		BorderForeground(t.Brand).
		Padding(0, 1).
		MarginLeft(2).
		Width(w - 6).
		Render(dm.input.View())

	timeoutLine := ""
	if dm.hasDeadline {
		remaining := int(time.Until(dm.deadline).Seconds())
		if remaining < 0 {
			remaining = 0
		}
		timeoutLine = lipgloss.NewStyle().Foreground(t.Warning).PaddingLeft(2).
			Render(fmt.Sprintf("⏱ %ds remaining", remaining))
	}

	hints := "↵ answer  esc skip  ctrl+s skip all"
	footer := lipgloss.NewStyle().Foreground(t.TextMuted).PaddingLeft(2).Render(hints)

	parts := []string{
		progressBar, divider, "",
		qBox, "",
		inputBox,
	}
	if timeoutLine != "" {
		parts = append(parts, timeoutLine)
	}
	parts = append(parts, "", divider, footer)
	return lipgloss.JoinVertical(lipgloss.Left, parts...)
}
