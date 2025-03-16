package tui

import (
	"context"
	"fmt"
	"strings"
	"time"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/charmbracelet/bubbles/spinner"
	"github.com/charmbracelet/bubbles/textarea"
	"github.com/charmbracelet/bubbles/viewport"
	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/tui/components"
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

	msgRenderer    *components.MessageRenderer
	streamCancel   context.CancelFunc

	currentMessage    *types.Message
	streamSegments    []types.MessageSegment
	streamContent     strings.Builder
	thinkingStartAt   time.Time
	activeSegmentType string
	thinkingBlocks    map[int]*components.ThinkingBlock
	toolCards         map[int]*components.ToolCard
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

	renderer, err := components.NewMessageRenderer(t, 80)
	if err != nil {
		renderer = nil
	}

	return ReplModel{
		theme:        t,
		viewport:     vp,
		textarea:     ta,
		spinner:      s,
		inputHistory: make([]string, 0),
		historyPos:   -1,
		msgRenderer:  renderer,
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
		if m.msgRenderer != nil {
			m.msgRenderer.SetWidth(msg.Width - 4)
		}

	case StreamMsg:
		return m.handleStreamMsg(msg)

	case StreamDoneMsg:
		return m.handleStreamDoneMsg(msg)

	case StreamErrorMsg:
		return m.handleStreamErrorMsg(msg)

	case TickMsg:
		if m.streaming {
			m.renderMessages()
			m.viewport.GotoBottom()
		}
		return m.streamTickCmds()

	case tea.KeyMsg:
		if m.streaming {
			switch msg.String() {
			case "ctrl+c":
				if m.streamCancel != nil {
					m.streamCancel()
				}
				m.streaming = false
				m.thinking = false
				m.textarea.Focus()
				m.renderMessages()
				var cmds []tea.Cmd
				return cmds, true
			}
			var cmds []tea.Cmd
			return cmds, false
		}

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

func (m *ReplModel) handleStreamMsg(msg StreamMsg) ([]tea.Cmd, bool) {
	chunk := msg.Chunk
	if chunk == nil {
		return nil, false
	}

	m.streaming = true

	switch chunk.Type {
	case "content":
		m.activeSegmentType = "content"
		m.thinking = false
		m.streamContent.WriteString(chunk.Delta)
	case "thinking":
		if m.activeSegmentType == "content" && m.streamContent.Len() > 0 {
			m.streamSegments = append(m.streamSegments, types.MessageSegment{
				Type:    "content",
				Content: m.streamContent.String(),
				Visible: true,
			})
			m.streamContent.Reset()
		}
		m.activeSegmentType = "thinking"
		m.thinking = true
		m.thinkingStartAt = time.Now()

		if chunk.Delta != "" {
			m.streamContent.WriteString(chunk.Delta)
		}
	case "done":
	}

	m.renderMessages()
	m.viewport.GotoBottom()

	return nil, false
}

func (m *ReplModel) handleStreamDoneMsg(msg StreamDoneMsg) ([]tea.Cmd, bool) {
	if m.streamContent.Len() > 0 {
		m.streamSegments = append(m.streamSegments, types.MessageSegment{
			Type:    "content",
			Content: m.streamContent.String(),
			Visible: true,
		})
		m.streamContent.Reset()
	}

	msg.Message.Segments = m.streamSegments
	if msg.Message.ToolCalls == nil || len(msg.Message.ToolCalls) == 0 {
		msg.Message.ToolCalls = m.getToolCallsFromSegments()
	}

	m.messages = append(m.messages, msg.Message)

	m.streaming = false
	m.thinking = false
	m.activeSegmentType = ""
	m.streamSegments = nil
	m.thinkingBlocks = nil
	m.toolCards = nil
	m.textarea.Focus()

	m.renderMessages()
	m.viewport.GotoBottom()

	var cmds []tea.Cmd
	return cmds, true
}

func (m *ReplModel) handleStreamErrorMsg(msg StreamErrorMsg) ([]tea.Cmd, bool) {
	errMsg := types.Message{
		Role:    "assistant",
		Content: fmt.Sprintf("Error during streaming: %v", msg.Err),
		Segments: []types.MessageSegment{{
			Type:    "content",
			Content: fmt.Sprintf("Error during streaming: %v", msg.Err),
			Visible: true,
		}},
		CreatedAt: time.Now(),
	}
	m.messages = append(m.messages, errMsg)

	m.streaming = false
	m.thinking = false
	m.activeSegmentType = ""
	m.streamSegments = nil
	m.streamContent.Reset()
	m.thinkingBlocks = nil
	m.toolCards = nil
	m.textarea.Focus()

	m.renderMessages()
	m.viewport.GotoBottom()

	var cmds []tea.Cmd
	return cmds, true
}

func (m *ReplModel) streamTickCmds() ([]tea.Cmd, bool) {
	var cmds []tea.Cmd
	cmds = append(cmds, StreamTickCmd())
	return cmds, false
}

func (m *ReplModel) getToolCallsFromSegments() []types.ToolCall {
	return nil
}

func (m *ReplModel) SetTheme(t theme.Theme) {
	m.theme = t
	if m.msgRenderer != nil {
		m.msgRenderer, _ = components.NewMessageRenderer(t, m.width-4)
	}
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
		if m.msgRenderer != nil {
			rendered := m.msgRenderer.RenderMessage(msg, m.width)
			b.WriteString(rendered)
			b.WriteString("\n")
		} else {
			role := msg.Role
			if role == "user" {
				role = "You"
			} else if role == "assistant" {
				role = "Assistant"
			}
			b.WriteString(fmt.Sprintf("%s: %s\n", role, msg.Content))
		}
	}

	if m.streaming {
		b.WriteString(m.renderStreamingContent())
	}

	m.viewport.SetContent(b.String())
}

func (m *ReplModel) renderStreamingContent() string {
	if m.msgRenderer == nil {
		content := m.streamContent.String()
		if content != "" {
			return fmt.Sprintf("Assistant: %s\n", content)
		}
		return "Assistant: ...\n"
	}

	var segments []types.MessageSegment
	segments = append(segments, m.streamSegments...)

	if m.streamContent.Len() > 0 {
		partial := m.streamContent.String()
		if m.activeSegmentType == "thinking" {
			segments = append(segments, types.MessageSegment{
				Type:       "thinking",
				Content:    partial,
				Visible:    true,
			})
		} else {
			segments = append(segments, types.MessageSegment{
				Type:    "content",
				Content: partial,
				Visible: true,
			})
		}
	}

	msg := types.Message{
		Role:     "assistant",
		Content:  m.streamContent.String(),
		Segments: segments,
	}

	return m.msgRenderer.RenderMessage(msg, m.width)
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
