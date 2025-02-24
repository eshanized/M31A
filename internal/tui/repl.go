package tui

import (
	"fmt"
	"strings"
	"time"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/charmbracelet/bubbles/spinner"
	"github.com/charmbracelet/bubbles/textarea"
	"github.com/charmbracelet/bubbles/viewport"
	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/tui/theme"
	"github.com/eshanized/M31A/internal/types"
)

type ReplModel struct {
	theme        theme.Theme
	messages     []types.Message
	viewport     viewport.Model
	textarea     textarea.Model
	spinner      spinner.Model
	scrollPos    int
	inputHistory []string
	historyPos   int
	placeholder  string
	streaming    bool
	thinking     bool
	lastStatus   string
	width        int
	height       int
}

func NewReplModel(t theme.Theme) ReplModel {
	ta := textarea.New()
	ta.Placeholder = "Type a message, /command, or goal..."
	ta.SetWidth(80)
	ta.SetHeight(3)
	ta.ShowLineNumbers = false
	ta.KeyMap.InsertNewline.SetEnabled(false)
	ta.Focus()
	ta.CharLimit = 0

	vp := viewport.New(80, 20)

	s := spinner.NewModel()
	s.Spinner = spinner.Dot
	s.Style = t.Spinner

	return ReplModel{
		theme:        t,
		viewport:     vp,
		textarea:     ta,
		spinner:      s,
		inputHistory: make([]string, 0),
		historyPos:   -1,
	}
}

func (m *ReplModel) Update(msg tea.Msg) ([]tea.Cmd, bool) {
	switch msg := msg.(type) {
	case tea.WindowSizeMsg:
		m.width = msg.Width
		m.height = msg.Height
		inputHeight := 3
		vpHeight := msg.Height - 2 - inputHeight
		if vpHeight < 1 {
			vpHeight = 1
		}
		m.viewport.Width = msg.Width
		m.viewport.Height = vpHeight
		m.textarea.SetWidth(msg.Width)
		m.textarea.SetHeight(inputHeight)

	case tea.KeyMsg:
		switch msg.String() {
		case "enter":
			input := strings.TrimSpace(m.textarea.Value())
			if input == "" {
				var cmds []tea.Cmd
				return cmds, false
			}
			m.inputHistory = append(m.inputHistory, input)
			m.historyPos = len(m.inputHistory)

			userMsg := types.Message{
				Role:      "user",
				Content:   input,
				CreatedAt: time.Now(),
			}
			m.messages = append(m.messages, userMsg)
			m.renderMessages()
			m.viewport.GotoBottom()
			m.textarea.Reset()

			var cmds []tea.Cmd
			return cmds, true

		case "up":
			if m.historyPos == -1 || len(m.inputHistory) == 0 {
				var cmds []tea.Cmd
				return cmds, false
			}
			if m.historyPos > 0 && m.historyPos <= len(m.inputHistory) {
				m.historyPos--
				m.textarea.SetValue(m.inputHistory[m.historyPos])
				m.textarea.SetCursor(len(m.textarea.Value()))
			}
			var cmds []tea.Cmd
			return cmds, false

		case "down":
			if m.historyPos < len(m.inputHistory)-1 {
				m.historyPos++
				m.textarea.SetValue(m.inputHistory[m.historyPos])
				m.textarea.SetCursor(len(m.textarea.Value()))
			} else {
				m.historyPos = len(m.inputHistory)
				m.textarea.Reset()
			}
			var cmds []tea.Cmd
			return cmds, false

		case "pgup":
			m.viewport.HalfViewUp()
			var cmds []tea.Cmd
			return cmds, false

		case "pgdown":
			m.viewport.HalfViewDown()
			var cmds []tea.Cmd
			return cmds, false

		case "esc":
			m.textarea.Reset()
			var cmds []tea.Cmd
			return cmds, false
		}

	case spinner.TickMsg:
		var spCmd tea.Cmd
		m.spinner, spCmd = m.spinner.Update(msg)
		return []tea.Cmd{spCmd}, false
	}

	var cmds []tea.Cmd
	var taCmd tea.Cmd
	var vpCmd tea.Cmd
	var spCmd tea.Cmd

	m.textarea, taCmd = m.textarea.Update(msg)
	m.viewport, vpCmd = m.viewport.Update(msg)
	m.spinner, spCmd = m.spinner.Update(msg)

	cmds = append(cmds, taCmd, vpCmd, spCmd)
	return cmds, false
}

func (m *ReplModel) SetTheme(t theme.Theme) {
	m.theme = t
}

func (m *ReplModel) SetStreaming(v bool) {
	m.streaming = v
}

func (m *ReplModel) SetThinking(v bool) {
	m.thinking = v
}

func (m *ReplModel) AddMessage(msg types.Message) {
	m.messages = append(m.messages, msg)
	m.renderMessages()
	m.viewport.GotoBottom()
}

func (m *ReplModel) renderMessages() {
	var b strings.Builder
	for _, msg := range m.messages {
		role := msg.Role
		if role == "user" {
			role = "You"
		} else if role == "assistant" {
			role = "Assistant"
		}
		b.WriteString(fmt.Sprintf("%s: %s\n", role, msg.Content))
	}
	m.viewport.SetContent(b.String())
}

func (m *ReplModel) View() string {
	if m.width == 0 {
		return "Initializing..."
	}

	viewportStr := m.viewport.View()
	inputStr := m.textarea.View()

	return lipgloss.JoinVertical(
		lipgloss.Top,
		viewportStr,
		inputStr,
	)
}

func (m *ReplModel) InputValue() string {
	return strings.TrimSpace(m.textarea.Value())
}

func (m *ReplModel) Messages() []types.Message {
	return m.messages
}

func (m *ReplModel) SpinnerTick() tea.Cmd {
	return m.spinner.Tick
}

func (m *ReplModel) GetStatusText() string {
	if m.streaming {
		return "Streaming..."
	}
	if m.thinking {
		return "Thinking..."
	}
	if m.lastStatus != "" {
		return m.lastStatus
	}
	return ""
}
