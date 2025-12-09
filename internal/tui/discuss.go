package tui

import (
	"fmt"
	"strings"
	"time"

	"github.com/charmbracelet/bubbles/textinput"
	tea "github.com/charmbracelet/bubbletea"
	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/tui/components"
	"github.com/eshanized/M31A/internal/tui/theme"
)

// DiscussModel presents discuss Q&A questions one-by-one.
type DiscussModel struct {
	theme      theme.Theme
	questions  []string
	current    int
	answers    []string
	input      textinput.Model
	timeout    int // seconds, 0 = no timeout
	deadline   time.Time
	hasDeadline bool
	width      int
	height     int
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

	// Emit answer message
	answerCmd := func() tea.Msg {
		return QuestionResponseMsg{Answer: answer}
	}

	dm.current++
	dm.input.SetValue("")
	dm.input.Focus()

	if dm.hasDeadline {
		dm.deadline = time.Now().Add(time.Duration(dm.timeout) * time.Second)
	}

	if dm.current >= len(dm.questions) {
		// All done — emit the last answer along with navigation
		return tea.Batch(answerCmd, func() tea.Msg {
			return AppMsg{Screen: ScreenREPL}
		})
	}
	return answerCmd
}

// View implements tea.Model.
func (dm *DiscussModel) View() string {
	t := dm.theme
	w := dm.width
	if w < 40 {
		w = 80
	}

	if len(dm.questions) == 0 {
		return lipgloss.NewStyle().Foreground(t.TextMuted).
			Render("No questions to answer.")
	}

	// ── Dot progress indicators ─────────────────────────────────────────────
	// ● = done/current, ○ = pending
	var dots []string
	for i := range dm.questions {
		var dotStyle lipgloss.Style
		if i <= dm.current {
			// Done or current → filled dot in brand
			dotStyle = lipgloss.NewStyle().Foreground(t.Brand).Bold(true)
		} else {
			// Pending → empty dot in muted
			dotStyle = lipgloss.NewStyle().Foreground(t.TextMuted)
		}
		dots = append(dots, dotStyle.Render("●"))
	}
	dotLine := lipgloss.NewStyle().Render(
		strings.Join(dots, " "))

	progressText := lipgloss.NewStyle().Foreground(t.TextSecondary).
		Render(fmt.Sprintf("Question %d of %d", dm.current+1, len(dm.questions)))

	progress := lipgloss.JoinHorizontal(lipgloss.Left,
		dotLine, "  ", progressText)

	// ── Question in ThinBorder card with brand left border ───────────────────
	question := ""
	if dm.current < len(dm.questions) {
		question = dm.questions[dm.current]
	}

	qCard := components.Card{
		Content: lipgloss.NewStyle().Foreground(t.Text).Render(question),
		Width:   w - 6,
		Border:  theme.ThinBorder,
		Style:   components.CardBrand,
		Theme:   t,
	}.Render()

	// ── Input area ───────────────────────────────────────────────────────────
	inputView := dm.input.View()
	inputBox := lipgloss.NewStyle().
		Border(lipgloss.RoundedBorder()).
		BorderForeground(t.Brand).
		Padding(0, 1).
		MarginLeft(2).
		Width(w - 8).
		Render(inputView)

	// ── Timer (only when < 30 seconds remaining) ────────────────────────────
	timeoutLine := ""
	if dm.hasDeadline && dm.timeout > 0 {
		remaining := int(time.Until(dm.deadline).Seconds())
		if remaining < 0 {
			remaining = 0
		}
		if remaining < 30 {
		timeoutLine = lipgloss.NewStyle().Foreground(t.Warning).
			Render(fmt.Sprintf("%ds remaining", remaining))
		}
	}

	// ── Footer ──────────────────────────────────────────────────────────────
	footer := lipgloss.NewStyle().Foreground(t.TextMuted).
		Render("↵ answer  esc skip  ctrl+s skip all")

	// ── Question separator ─────────────────────────────────────────────────
	qSep := components.SectionDivider{
		Width: w - 4,
		Theme: t,
	}.Render()

	// ── Assemble ────────────────────────────────────────────────────────────
	parts := []string{
		"", progress, "",
		qSep,
		qCard, "",
		inputBox,
	}
	if timeoutLine != "" {
		parts = append(parts, timeoutLine)
	}
	parts = append(parts, "", footer)
	return lipgloss.JoinVertical(lipgloss.Left, parts...)
}
