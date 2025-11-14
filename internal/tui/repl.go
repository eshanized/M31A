package tui

import (
	"strings"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/charmbracelet/bubbles/viewport"
	"github.com/charmbracelet/lipgloss"
)

// ─── REPL keyboard handling ───────────────────────────────────────────────────

// Init implements tea.Model; the REPL starts with a spinner tick.
func (m *ReplModel) Init() tea.Cmd {
	return StreamTickCmd()
}

// Update processes messages for the REPL. Delegates to handleKeyMsg,
// handleStreamMsg, etc. based on message type.
func (m *ReplModel) Update(msg tea.Msg) (tea.Model, tea.Cmd) {
	var cmds []tea.Cmd

	switch msg := msg.(type) {
	case tea.WindowSizeMsg:
		// Resize all components
		m.width = msg.Width
		m.height = msg.Height

		rw := m.replWidth()
		vpH := viewportHeight(msg.Height)

		if m.viewport.Width == 0 {
			m.viewport = viewport.New(rw, vpH)
		} else {
			m.viewport.Width = rw
			m.viewport.Height = vpH
		}
		m.textarea.SetWidth(rw)
		if m.msgRenderer != nil {
			_ = m.msgRenderer.SetWidth(rw - 4)
		}
		m.renderMessages()

	case tea.KeyMsg:
		if m.streaming {
			c := m.handleStreamingKeyMsg(msg)
			if c != nil {
				cmds = append(cmds, c)
			}
		} else {
			c := m.handleKeyMsg(msg)
			if c != nil {
				cmds = append(cmds, c)
			}
		}

	case StreamMsg:
		cs, _ := m.handleStreamMsg(msg)
		cmds = append(cmds, cs...)

	case StreamDoneMsg:
		cs, _ := m.handleStreamDoneMsg(msg)
		cmds = append(cmds, cs...)

	case StreamErrorMsg:
		cs, _ := m.handleStreamErrorMsg(msg)
		cmds = append(cmds, cs...)

	case TickMsg:
		if m.streaming || m.thinking {
			m.spinner.Next()
			cmds = append(cmds, StreamTickCmd())
		}

	case ProviderModelsFetchedMsg:
		m.handleProviderModelsFetched(msg)

	case SidebarRefreshMsg:
		// Sidebar messages are handled by AppState; ignored here.
	}

	return m, tea.Batch(cmds...)
}

// handleKeyMsg handles key events when NOT streaming.
func (m *ReplModel) handleKeyMsg(msg tea.KeyMsg) tea.Cmd {
	switch msg.String() {
	case "ctrl+c":
		return nil

	case "enter":
		if m.slashVisible && len(m.slashSuggestions) > 0 {
			// Complete slash command
			return m.handleSlashComplete()
		}
		return m.handleEnterKey()

	case "tab":
		if m.slashVisible && len(m.slashSuggestions) > 0 {
			return m.handleSlashComplete()
		}

	case "up":
		if m.slashVisible {
			if m.slashSelected > 0 {
				m.slashSelected--
			}
			return nil
		}
		// History navigation
		m.navigateHistoryUp()
		return nil

	case "down":
		if m.slashVisible {
			if m.slashSelected < len(m.slashSuggestions)-1 {
				m.slashSelected++
			}
			return nil
		}
		m.navigateHistoryDown()
		return nil

	case "esc":
		if m.slashVisible {
			m.slashVisible = false
			m.slashSuggestions = nil
			return nil
		}
		m.textarea.SetValue("")

	case "ctrl+p":
		// Command palette — emit for AppState
		return func() tea.Msg {
			return KeyActionMsg{Action: "open_palette"}
		}

	case "ctrl+b":
		return func() tea.Msg {
			return KeyActionMsg{Action: "toggle_sidebar"}
		}

	case "ctrl+x":
		if m.keyRegistry != nil {
			handled, cmd := m.keyRegistry.Handle("ctrl+x", CtxREPL)
			if handled {
				return cmd
			}
		}

	case "ctrl+l":
		// Scroll to bottom
		m.viewport.GotoBottom()
		m.userScrolled = false
		return nil

	case "pgup", "ctrl+u":
		m.viewport.ViewUp()
		m.userScrolled = true
		return nil

	case "pgdown", "ctrl+d":
		m.viewport.ViewDown()
		return nil

	default:
		// Leader key chord handling
		if m.keyRegistry != nil {
			handled, cmd := m.keyRegistry.Handle(msg.String(), CtxREPL)
			if handled {
				return cmd
			}
		}
	}

	// Pass to textarea for typing
	var cmd tea.Cmd
	m.textarea, cmd = m.textarea.Update(msg)
	m.updateSlashSuggestions()
	return cmd
}

// handleStreamingKeyMsg handles key events while streaming.
func (m *ReplModel) handleStreamingKeyMsg(msg tea.KeyMsg) tea.Cmd {
	switch msg.String() {
	case "ctrl+c":
		// Signal cancellation (AppState handles ctx cancel)
		return func() tea.Msg {
			return KeyActionMsg{Action: "cancel_stream"}
		}
	case "pgup", "ctrl+u":
		m.viewport.ViewUp()
		m.userScrolled = true
	case "pgdown", "ctrl+d":
		m.viewport.ViewDown()
	case "ctrl+l":
		m.viewport.GotoBottom()
		m.userScrolled = false
	}
	return nil
}

// handleEnterKey submits the textarea content as a user message.
func (m *ReplModel) handleEnterKey() tea.Cmd {
	input := m.InputValue()
	if input == "" {
		return nil
	}

	// Slash command: emit SlashCommandMsg for AppState to route
	if strings.HasPrefix(input, "/") {
		m.textarea.SetValue("")
		m.slashVisible = false
		m.slashSuggestions = nil
		return func() tea.Msg {
			return SlashCommandMsg{Command: input}
		}
	}

	// Shell command
	if strings.HasPrefix(input, "!") {
		m.textarea.SetValue("")
		m.slashVisible = false
		return func() tea.Msg {
			return SlashCommandMsg{Command: input}
		}
	}

	// Regular message: add to conversation and emit for AppState to dispatch
	m.textarea.SetValue("")
	m.slashVisible = false
	m.historyIndex = -1
	if m.frecentHistory != nil {
		m.frecentHistory.Upsert(input)
	}

	userMsg := m.messages
	_ = userMsg

	// Build user message
	newMsg := makeAssistantMsg(input)
	newMsg.Role = "user"

	// Add to messages
	m.messages = append(m.messages, newMsg)
	m.renderMessages()
	m.viewport.GotoBottom()
	m.userScrolled = false

	// Emit for routing
	return func() tea.Msg {
		return SlashCommandMsg{Command: input}
	}
}

// handleSlashComplete completes the selected slash suggestion.
func (m *ReplModel) handleSlashComplete() tea.Cmd {
	if m.slashSelected >= len(m.slashSuggestions) {
		return nil
	}
	chosen := m.slashSuggestions[m.slashSelected]
	m.textarea.SetValue(chosen.Slash)
	m.textarea.CursorEnd()
	m.slashVisible = false
	m.slashSuggestions = nil
	return nil
}

// navigateHistoryUp moves to the previous command in frecent history.
func (m *ReplModel) navigateHistoryUp() {
	if m.frecentHistory == nil {
		return
	}
	entries := m.frecentHistory.Search("", 20)
	if len(entries) == 0 {
		return
	}
	m.historyIndex++
	if m.historyIndex >= len(entries) {
		m.historyIndex = len(entries) - 1
	}
	m.textarea.SetValue(entries[m.historyIndex].Text)
	m.textarea.CursorEnd()
}

// navigateHistoryDown moves to the next command in frecent history.
func (m *ReplModel) navigateHistoryDown() {
	if m.historyIndex <= 0 {
		m.historyIndex = -1
		m.textarea.SetValue("")
		return
	}
	m.historyIndex--
	if m.frecentHistory != nil {
		entries := m.frecentHistory.Search("", 20)
		if m.historyIndex >= 0 && m.historyIndex < len(entries) {
			m.textarea.SetValue(entries[m.historyIndex].Text)
			m.textarea.CursorEnd()
		}
	}
}

// ThinkingBlockToggleMsg is emitted when the user toggles a thinking block.
type ThinkingBlockToggleMsg struct {
	Index int
}

// renderQuickActions renders the quick action hints if no messages.
func (m *ReplModel) renderQuickActions() string {
	t := m.theme
	return lipgloss.NewStyle().
		Foreground(t.TextMuted).
		Italic(true).
		Render("Quick actions: /help  /settings  /models  /workflow")
}
