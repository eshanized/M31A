package tui

import (
	"context"
	"strings"
	"time"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/eshanized/M31A/internal/config"
	"github.com/eshanized/M31A/internal/provider"
	"github.com/eshanized/M31A/internal/tools"
	"github.com/eshanized/M31A/internal/tui/components"
	"github.com/eshanized/M31A/internal/tui/theme"
	"github.com/eshanized/M31A/internal/types"
)

// ─── Theme / layout setters ───────────────────────────────────────────────────

// SetTheme updates the theme and reinitializes the message renderer.
func (m *ReplModel) SetTheme(t theme.Theme) {
	m.theme = t
	if m.msgRenderer != nil {
		newRenderer, err := components.NewMessageRenderer(t, m.width-4)
		if err == nil {
			m.msgRenderer = newRenderer
		}
	}
}

// replWidth returns the available REPL width accounting for sidebar.
func (m *ReplModel) replWidth() int {
	w := m.width - m.sidebarWidth
	if w < 20 {
		w = 20
	}
	return w
}

// SetSidebarWidth updates the reserved width for the sidebar.
func (m *ReplModel) SetSidebarWidth(sw int) {
	m.sidebarWidth = sw
	replWidth := m.replWidth()
	m.viewport.Width = replWidth
	if m.msgRenderer != nil {
		_ = m.msgRenderer.SetWidth(replWidth - 4)
	}
	m.textarea.SetWidth(replWidth)
}

// ─── Provider / session setters ───────────────────────────────────────────────

// SetProvider configures the active provider and returns a tea.Cmd that
// asynchronously validates the model by fetching the provider's model catalog.
func (m *ReplModel) SetProvider(ctx context.Context, registry *provider.Registry, activeProvider string, model *types.ModelInfo, sessionID string, cfg *config.Config) tea.Cmd {
	m.registry = registry
	m.activeProvider = activeProvider
	m.sessionID = sessionID
	m.cfg = cfg
	m.modelValid = true

	if model == nil || registry == nil {
		m.activeModel = model
		return nil
	}
	m.activeModel = model

	p := registry.ActiveProvider()
	if p == nil {
		return nil
	}
	return func() tea.Msg {
		fetchCtx, cancel := context.WithTimeout(ctx, 15*time.Second)
		defer cancel()
		models, err := p.FetchModels(fetchCtx)
		return ProviderModelsFetchedMsg{Models: models, Model: model, Err: err}
	}
}

// handleProviderModelsFetched processes the async result of SetProvider's FetchModels call.
func (m *ReplModel) handleProviderModelsFetched(msg ProviderModelsFetchedMsg) {
	if msg.Err != nil || msg.Model == nil {
		return
	}
	for _, model := range msg.Models {
		if model.ID == msg.Model.ID {
			if m.registry != nil {
				p := m.registry.ActiveProvider()
				if p != nil {
					if info, _ := p.GetModel(msg.Model.ID); info != nil {
						m.activeModel = info
						return
					}
				}
			}
			return
		}
	}
	m.modelValid = false
}

// SetDispatcher sets the tool dispatcher.
func (m *ReplModel) SetDispatcher(d *tools.Dispatcher) {
	m.dispatcher = d
}

// SetCommandRegistry sets the command registry.
func (m *ReplModel) SetCommandRegistry(reg *CommandRegistry) {
	m.cmdRegistry = reg
}

// SetFrecentHistory sets the frecency history for prompt history navigation.
func (m *ReplModel) SetFrecentHistory(fh *FrecentHistory) {
	m.frecentHistory = fh
}

// SetCwd sets the working directory for @filepath resolution.
// Changing cwd also invalidates the cached mention completer.
func (m *ReplModel) SetCwd(cwd string) {
	if m.cwd != cwd {
		m.mentionCompleter = nil // force re-scan on next @-mention
	}
	m.cwd = cwd
}

// SetChangedFiles updates the count of git-changed files shown on the welcome screen.
func (m *ReplModel) SetChangedFiles(n int) {
	m.changedFiles = n
}

// SetKeyRegistry sets the key registry.
func (m *ReplModel) SetKeyRegistry(kr *KeyRegistry) {
	m.keyRegistry = kr
}

// SetSessionSparkline updates the recent-activity sparkline shown on the welcome screen.
func (m *ReplModel) SetSessionSparkline(spark string) {
	m.sessionSparkline = spark
}

// SetLastActivity updates the last-activity timestamp.
func (m *ReplModel) SetLastActivity(t time.Time) {
	m.lastActivity = t
}

// SetStreaming sets the streaming state.
func (m *ReplModel) SetStreaming(v bool) {
	m.streaming = v
}

// SetThinking sets the thinking state.
func (m *ReplModel) SetThinking(v bool) {
	m.thinking = v
}

// SetSessionID updates the session ID.
func (m *ReplModel) SetSessionID(id string) {
	m.sessionID = id
}

// ─── Message management ───────────────────────────────────────────────────────

// AddMessage adds a message to the REPL and re-renders.
func (m *ReplModel) AddMessage(msg types.Message) {
	m.messages = append(m.messages, msg)
	if len(m.messages) > MaxMessageHistory {
		m.messages = m.messages[len(m.messages)-500:]
	}
	m.renderMessages()
	m.autoScrollConditionally()
}

// InputValue returns the trimmed current textarea input.
func (m *ReplModel) InputValue() string {
	return strings.TrimSpace(m.textarea.Value())
}

// Messages returns all messages in the REPL.
func (m *ReplModel) Messages() []types.Message {
	return m.messages
}

// ClearMessages removes all messages and resets streaming state.
func (m *ReplModel) ClearMessages() {
	m.messages = nil
	m.streaming = false
	m.thinking = false
	m.activeSegmentType = ""
	m.streamSegments = nil
	m.streamContent.Reset()
	m.thinkingBlocks = make(map[int]*components.ThinkingBlock)
	m.toolCards = make(map[int]*components.ToolCard)
	m.renderMessages()
	m.viewport.GotoBottom()
	m.userScrolled = false
}

// RefreshViewport forces a re-render of the viewport content.
func (m *ReplModel) RefreshViewport() {
	m.renderMessages()
}

// ─── Status getters ───────────────────────────────────────────────────────────

// SpinnerTick returns a tea.Cmd that ticks the spinner at 10fps.
func (m *ReplModel) SpinnerTick() tea.Cmd {
	return StreamTickCmd()
}

// GetStatusText returns the current status text for the status bar.
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

// LastUsage returns the usage from the last completed stream.
func (m *ReplModel) LastUsage() *types.Usage {
	return m.lastUsage
}

// LastCost returns the estimated cost of the last completed stream.
func (m *ReplModel) LastCost() float64 {
	return m.lastCost
}

// ShowQuestion displays a question in the REPL.
func (m *ReplModel) ShowQuestion(msg QuestionRequestMsg) {
	header := msg.Header
	if header == "" {
		header = "Question"
	}
	qMsg := makeAssistantMsg(header + "\n\n" + msg.Question)
	m.AddMessage(qMsg)
}

// ─── Viewport management ──────────────────────────────────────────────────────

// autoScrollConditionally scrolls to the bottom only if the user hasn't manually scrolled.
func (m *ReplModel) autoScrollConditionally() {
	if !m.userScrolled {
		m.viewport.GotoBottom()
	}
}

// renderMessages rebuilds the viewport content from the message list.
func (m *ReplModel) renderMessages() {
	if len(m.messages) == 0 && !m.streaming {
		// Welcome screen is set when messages == 0 and not streaming.
		// Content is set by the view loop; just clear the viewport here.
		m.viewport.SetContent("")
		return
	}

	if m.msgRenderer == nil {
		r, err := components.NewMessageRenderer(m.theme, m.replWidth()-4)
		if err != nil {
			m.viewport.SetContent("(render error)")
			return
		}
		m.msgRenderer = r
	}

	var sb strings.Builder
	rw := m.replWidth()

	for i, msg := range m.messages {
		if i > 0 {
			sb.WriteString("\n")
			sb.WriteString(components.RenderTimestampBar(m.theme, msg.CreatedAt, rw))
			sb.WriteString("\n")

			// Add extra blank line between conversation turns (role switches)
			prevRole := m.messages[i-1].Role
			if prevRole != msg.Role {
				sb.WriteString("\n")
			}
		}
		sb.WriteString(m.msgRenderer.RenderMessage(msg, rw))
		sb.WriteString("\n")
	}

	// Append active streaming content
	if m.streaming || m.thinking {
		streamContent := m.streamContent.String()
		if streamContent != "" {
			streamMsg := types.Message{
				Role:    "assistant",
				Content: streamContent,
			}
			if m.activeSegmentType == "thinking" {
				streamMsg.Segments = []types.MessageSegment{{
					Type:    "thinking",
					Content: streamContent,
					Visible: true,
				}}
			}
			sb.WriteString("\n")
			sb.WriteString(m.msgRenderer.RenderMessage(streamMsg, rw))
		}
	}

	// Append thinking toggle hints after finalized thinking blocks
	if len(m.thinkingBlocks) > 0 {
		for idx := range m.thinkingBlocks {
			hint := m.renderThinkingToggleHint(idx, 0)
			if hint != "" {
				sb.WriteString("\n")
				sb.WriteString(hint)
			}
		}
	}

	m.viewport.SetContent(sb.String())
}
