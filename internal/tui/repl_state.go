package tui

import (
	"context"
	"fmt"
	"strings"
	"time"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/config"
	"github.com/eshanized/M31A/internal/provider"
	"github.com/eshanized/M31A/internal/tools"
	"github.com/eshanized/M31A/internal/tui/components"
	"github.com/eshanized/M31A/internal/tui/theme"
	"github.com/eshanized/M31A/internal/types"
	"github.com/eshanized/M31A/pkg/history"
)

// updatePlaceholder sets the textarea placeholder based on current state
// so the input hint reflects what the user can do right now.
func (m *ReplModel) updatePlaceholder() {
	switch {
	case m.streaming || m.thinking:
		m.textarea.Placeholder = "Waiting for response… (ctrl+c to cancel)"
	case m.newMessagesWhileScrolled > 0 && m.userScrolled:
		m.textarea.Placeholder = "Type a message… (ctrl+l to jump to latest)"
	default:
		m.textarea.Placeholder = "Type a message, /command, or goal..."
	}
}

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
			// Model exists in the catalog — try to enrich from provider cache.
			if m.registry != nil {
				p := m.registry.ActiveProvider()
				if p != nil {
					if info, _ := p.GetModel(msg.Model.ID); info != nil {
						m.activeModel = info
						return
					}
				}
			}
			// Cache miss — the model is in the catalog but the provider's local
			// cache isn't warm yet. Use the model info from the fetched list
			// directly so pricing/context are populated.
			enriched := model
			m.activeModel = &enriched
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
func (m *ReplModel) SetFrecentHistory(fh *history.FrecentHistory) {
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
		m.messages = m.messages[len(m.messages)-MaxMessageHistory/2:]
	}
	// Invalidate incremental rendering cache when messages change
	m.cachedMessageContent = ""
	m.cachedMessageCount = 0
	// Invalidate streaming render cache when a new message arrives
	m.cachedThinkingBlock = nil
	m.cachedThinkingContent = ""
	if m.userScrolled {
		m.newMessagesWhileScrolled++
	}
	m.renderMessages()
	m.autoScrollConditionally()
}

// SetMessages replaces all messages and re-renders the viewport.
func (m *ReplModel) SetMessages(msgs []types.Message) {
	m.messages = msgs
	// Invalidate incremental rendering cache when messages change
	m.cachedMessageContent = ""
	m.cachedMessageCount = 0
	// Invalidate streaming render cache to prevent stale thinking blocks
	m.cachedThinkingBlock = nil
	m.cachedThinkingContent = ""
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
	m.awaitingResponse = false
	m.activeSegmentType = ""
	m.streamSegments = nil
	m.streamContent.Reset()
	m.thinkingBlocks = make(map[int]*components.ThinkingBlock)
	m.toolCards = make(map[int]*components.ToolCard)
	m.liveToolIndex = make(map[string]int)
	// Invalidate incremental rendering cache
	m.cachedMessageContent = ""
	m.cachedMessageCount = 0
	// Invalidate streaming render cache
	m.cachedThinkingBlock = nil
	m.cachedThinkingContent = ""
	m.renderMessages()
	m.viewport.GotoBottom()
	m.userScrolled = false
}

// RefreshViewport forces a re-render of the viewport content.

// maxTextareaHeight is the maximum number of rows the textarea can grow to.
const maxTextareaHeight = 10

// updateAutoExpandHeight grows the textarea height based on the number of lines
// in the current input, up to maxTextareaHeight rows.
func (m *ReplModel) updateAutoExpandHeight() {
	lines := strings.Count(m.textarea.Value(), "\n") + 1
	newHeight := lines
	if newHeight < inputHeight {
		newHeight = inputHeight
	}
	if newHeight > maxTextareaHeight {
		newHeight = maxTextareaHeight
	}
	if m.textarea.Height() != newHeight {
		m.textarea.SetHeight(newHeight)
	}
}
func (m *ReplModel) RefreshViewport() {
	// Invalidate cache on explicit refresh
	m.cachedMessageContent = ""
	m.cachedMessageCount = 0
	m.renderMessages()
}

// FlushViewport resets the viewport scroll position to the bottom and forces
// a full re-render of the message content. Messages are preserved.
func (m *ReplModel) FlushViewport() {
	m.cachedMessageContent = ""
	m.cachedMessageCount = 0
	m.renderMessages()
	m.viewport.GotoBottom()
	m.userScrolled = false
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

// minRenderInterval is the minimum time between full viewport re-renders
// during streaming. Prevents rebuilding the entire viewport at 10fps when
// only the streaming tail changes.
const minRenderInterval = time.Second / 5 // 5fps during streaming

// autoScrollConditionally scrolls to the bottom only if the user hasn't manually scrolled.
// Uses smooth ease-out scrolling: sets a target offset and lets the tick handler animate toward it.
// Snaps immediately if the gap exceeds one viewport height (initial load, session restore).
func (m *ReplModel) autoScrollConditionally() {
	if m.userScrolled {
		return
	}
	lineCount := strings.Count(m.viewportContent, "\n") + 1
	target := lineCount - m.viewport.Height
	if target < 0 {
		target = 0
	}
	m.smoothScrollTarget = target
	if m.smoothScrollTarget-m.viewport.YOffset > m.viewport.Height {
		m.viewport.GotoBottom()
	}
}

// renderMessages rebuilds the viewport content from the message list.
// During streaming, uses incremental rendering to avoid rebuilding the
// entire viewport on every 100ms tick.
func (m *ReplModel) renderMessages() {
	// Throttle during streaming: skip if rendered too recently
	if m.streaming || m.thinking {
		if !m.lastRenderTime.IsZero() && time.Since(m.lastRenderTime) < minRenderInterval {
			return
		}
	}
	if len(m.messages) == 0 && !m.streaming {
		// Render welcome screen content
		m.viewport.SetContent(m.renderWelcome())
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

	rw := m.replWidth()

	// During streaming, use incremental rendering if we have cached content
	if m.streaming && m.cachedMessageContent != "" && len(m.messages) > 0 {
		m.renderMessagesIncremental(rw)
		return
	}

	// Full render
	var sb strings.Builder

	// Build a line-offset table so mouse clicks can map Y → message index.
	offsets := make([]int, len(m.messages))
	lineCount := 0

	prevRole := ""
	var prevTime time.Time
	for i, msg := range m.messages {
		offsets[i] = lineCount
		rendered := m.msgRenderer.RenderMessage(msg, rw)
		if rendered == "" {
			offsets[i] = -1
			continue
		}
		// Write separator AFTER confirming message has content.
		if i > 0 {
			timeGap := !msg.CreatedAt.IsZero() && !prevTime.IsZero() &&
				msg.CreatedAt.Sub(prevTime) > 60*time.Second
			if prevRole != "" && msg.Role != prevRole {
				sepChar := "·"
				sepLine := lipgloss.NewStyle().
					Foreground(m.theme.BorderSubtle).
					Faint(true).
					Render(strings.Repeat(sepChar, rw/2))
				sb.WriteString("\n")
				sb.WriteString(sepLine)
				sb.WriteString("\n")
				lineCount += 2
			} else if timeGap {
				timeLabel := msg.CreatedAt.Format("15:04")
				timeGapLine := lipgloss.NewStyle().
					Foreground(m.theme.TextMuted).
					Faint(true).
					Render("── " + timeLabel + " " + strings.Repeat("─", rw/2-8))
				sb.WriteString("\n")
				sb.WriteString(timeGapLine)
				sb.WriteString("\n")
				lineCount += 2
			} else {
				sb.WriteString("\n")
				lineCount++
			}
		}
		sb.WriteString(rendered)
		lineCount += strings.Count(rendered, "\n") + 1
		sb.WriteString("\n")
		lineCount++
		prevRole = msg.Role
		if !msg.CreatedAt.IsZero() {
			prevTime = msg.CreatedAt
		}
	}
	m.messageLineOffsets = offsets

	// Cache the rendered messages content for incremental streaming
	m.cachedMessageContent = sb.String()
	m.cachedMessageCount = len(m.messages)

	// Append active streaming content
	if m.streaming || m.thinking {
		sb.WriteString("\n")
		sb.WriteString(m.renderStreamingContent(rw))
	}

	content := sb.String()
	m.viewportContent = content
	m.viewport.SetContent(content)
	m.lastRenderTime = time.Now()
}

// renderMessagesIncremental appends only the streaming content to the cached
// message content, avoiding a full re-render during streaming.
func (m *ReplModel) renderMessagesIncremental(rw int) {
	var sb strings.Builder
	sb.WriteString(m.cachedMessageContent)
	sb.WriteString("\n")
	sb.WriteString(m.renderStreamingContent(rw))

	content := sb.String()
	m.viewportContent = content
	m.viewport.SetContent(content)
	m.lastRenderTime = time.Now()
}

// renderStreamingContent renders the current streaming content (thinking or response).
// Uses plain lipgloss styling during streaming instead of Glamour's full markdown
// pipeline. Glamour is O(n) with accumulated text and re-parses markdown on every
// chunk, making it the primary bottleneck for streaming throughput. Full markdown
// rendering is deferred to stream completion when the message is finalized.
func (m *ReplModel) renderStreamingContent(rw int) string {
	streamContent := m.streamContent.String()
	if streamContent != "" {
		if m.activeSegmentType == "thinking" {
			// Cache ThinkingBlock to avoid re-allocating on every tick.
			// Only recreate when content has actually changed.
			if m.cachedThinkingBlock == nil || m.cachedThinkingContent != streamContent {
				m.cachedThinkingBlock = components.NewThinkingBlock(
					types.MessageSegment{
						Type:      "thinking",
						Content:   streamContent,
						Visible:   true,
						StartedAt: m.thinkingStartAt,
					}, m.theme, true, -1)
				m.cachedThinkingContent = streamContent
			}
			return m.cachedThinkingBlock.Render(rw)
		}
		// Plain lipgloss rendering during streaming — bypass Glamour entirely.
		// Apply basic text styling: brand color for the content, no markdown parsing.
		contentWidth := rw - 6 // account for gutter + padding
		if contentWidth < 20 {
			contentWidth = 20
		}
		// Animated block cursor: alternates between █ and ░ for visibility
		cursorFrames := []string{"█", "▓", "▒", "░", "▒", "▓"}
		frameIdx := int(time.Now().UnixMilli()/150) % len(cursorFrames)
		cursorChar := cursorFrames[frameIdx]
		cursor := lipgloss.NewStyle().Foreground(m.theme.Brand).Render(cursorChar)
		rendered := lipgloss.NewStyle().
			Foreground(m.theme.TextPrimary).
			Width(contentWidth).
			Render(streamContent + cursor)
		return rendered
	}
	// Show spinner when streaming but no content yet
	spinnerFrame := m.spinner.Peek()
	return lipgloss.NewStyle().
		Foreground(m.theme.TextMuted).
		Render("  " + spinnerFrame + " generating response…")
}

// TrackLiveTool registers an in-progress agent loop tool card by name,
// mapping it to the message index so UpdateLiveTool can find it later.
func (m *ReplModel) TrackLiveTool(toolName string, msgIndex int) {
	if m.liveToolIndex == nil {
		m.liveToolIndex = make(map[string]int)
	}
	m.liveToolIndex[toolName] = msgIndex
}

// UpdateLiveTool updates an in-progress tool card with its result.
// It modifies the message's tool_use segment to carry result data,
// then re-renders the viewport.
func (m *ReplModel) UpdateLiveTool(toolName string, err error, durationMs int64) {
	if m.liveToolIndex == nil {
		return
	}
	idx, ok := m.liveToolIndex[toolName]
	if !ok || idx >= len(m.messages) {
		return
	}
	delete(m.liveToolIndex, toolName)

	msg := &m.messages[idx]
	for i := range msg.Segments {
		if msg.Segments[i].Type == "tool_use" {
			resultSuffix := " ✓"
			if err != nil {
				resultSuffix = " ✗"
			}
			msg.Segments[i].DurationMs = durationMs
			if durationMs > 0 {
				resultSuffix = fmt.Sprintf(" (%dms)%s", durationMs, resultSuffix)
			}
			msg.Segments[i].Content += resultSuffix
			break
		}
	}
	// Invalidate cache since we modified a message
	m.cachedMessageContent = ""
	m.cachedMessageCount = 0
	m.renderMessages()
	m.autoScrollConditionally()
}
