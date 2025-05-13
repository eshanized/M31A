package tui

import (
	"context"
	"encoding/json"
	"fmt"
	"strings"
	"time"

	"github.com/charmbracelet/bubbles/spinner"
	"github.com/charmbracelet/bubbles/textarea"
	"github.com/charmbracelet/bubbles/viewport"
	tea "github.com/charmbracelet/bubbletea"
	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/config"
	"github.com/eshanized/M31A/internal/provider"
	"github.com/eshanized/M31A/internal/tui/components"
	"github.com/eshanized/M31A/internal/tui/theme"
	"github.com/eshanized/M31A/internal/types"
	"github.com/eshanized/M31A/pkg/arbitrage"
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
	sidebarWidth int // width reserved for sidebar (0 if hidden)

	msgRenderer  *components.MessageRenderer
	streamCancel context.CancelFunc

	currentMessage    *types.Message
	streamSegments    []types.MessageSegment
	streamContent     strings.Builder
	thinkingStartAt   time.Time
	activeSegmentType string
	thinkingBlocks    map[int]*components.ThinkingBlock
	toolCards         map[int]*components.ToolCard

	fallbackBanner   string    // current fallback banner text, empty = no banner
	fallbackBannerAt time.Time // when the banner appeared (for 15s auto-dismiss)

	// Active question from AskUserQuestion tool
	activeQuestion   *QuestionRequestMsg

	// Provider access for LLM calls
	registry       *provider.Registry
	activeProvider string
	activeModel    *types.ModelInfo
	sessionID      string

	// Config access for arbitrage settings
	cfg *config.Config

	// Streaming channel — created when a stream starts, read by handleStreamMsg
	streamCh chan tea.Msg

	// streamDone is closed when the stream goroutine exits (normal or cancelled).
	// Used by the continuation cmd to detect stream termination.
	streamDone chan struct{}

	// Last stream usage and cost (from StreamDoneMsg)
	lastUsage *types.Usage
	lastCost  float64

	// Per-block thinking focus
	thinkingFocusIndex int // -1 = no focus, otherwise index into thinkingBlocks
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

	m := ReplModel{
		theme:          t,
		viewport:       vp,
		textarea:       ta,
		spinner:        s,
		inputHistory:   make([]string, 0),
		historyPos:     -1,
		msgRenderer:    renderer,
		thinkingBlocks: make(map[int]*components.ThinkingBlock),
		toolCards:      make(map[int]*components.ToolCard),
	}

	// Set welcome message in viewport
	m.viewport.SetContent(m.renderWelcome())

	return m
}

func (m *ReplModel) renderWelcome() string {
	var sb strings.Builder

	title := lipgloss.NewStyle().
		Foreground(m.theme.Brand).
		Bold(true).
		Render("Welcome to M31A")
	sb.WriteString(title)
	sb.WriteString("\n\n")

	sb.WriteString(lipgloss.NewStyle().
		Foreground(m.theme.TextSecondary).
		Render("Your terminal AI coding assistant."))
	sb.WriteString("\n\n")

	sb.WriteString(lipgloss.NewStyle().
		Foreground(m.theme.TextPrimary).
		Bold(true).
		Render("Getting started:"))
	sb.WriteString("\n")

	commands := []string{
		"/workflow <goal>   Start a full coding workflow",
		"/phase initialize   Run initialize phase",
		"/models             Browse available models",
		"/settings           Open settings",
		"/status             Show current session info",
		"/help               List all commands",
	}
	for _, cmd := range commands {
		sb.WriteString(lipgloss.NewStyle().
			Foreground(m.theme.TextSecondary).
			Render("  " + cmd))
		sb.WriteString("\n")
	}

	sb.WriteString("\n")
	sb.WriteString(lipgloss.NewStyle().
		Foreground(m.theme.TextSecondary).
		Render("Or just type your question and press Enter."))

	return sb.String()
}

func (m *ReplModel) Update(msg tea.Msg) ([]tea.Cmd, bool) {
	// Check if fallback banner has expired
	if m.fallbackBanner != "" && time.Now().After(m.fallbackBannerAt) {
		m.fallbackBanner = ""
	}

	switch msg := msg.(type) {
	case tea.WindowSizeMsg:
		m.width = msg.Width
		m.height = msg.Height
		inputHeight := 3
		// Account for header(1) + status bar(1) + textarea borders/padding(2)
		// + viewport borders(2) = 6 total chrome lines
		const chromeHeight = 6
		vpHeight := msg.Height - chromeHeight - inputHeight
		if vpHeight < 1 {
			vpHeight = 1
		}
		replWidth := msg.Width - m.sidebarWidth
		if replWidth < 20 {
			replWidth = 20
		}
		m.viewport.Width = replWidth
		m.viewport.Height = vpHeight
		m.textarea.SetWidth(replWidth)
		m.textarea.SetHeight(inputHeight)
		if m.msgRenderer != nil {
			if err := m.msgRenderer.SetWidth(replWidth - 4); err != nil {
				m.lastStatus = fmt.Sprintf("Renderer resize failed: %v", err)
			}
		}
		// Show welcome message if no messages yet
		if len(m.messages) == 0 {
			m.viewport.SetContent(m.renderWelcome())
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
			// If a question is active, submit the answer
			if m.activeQuestion != nil {
				cmd := m.HandleQuestionInput()
				var cmds []tea.Cmd
				if cmd != nil {
					cmds = append(cmds, cmd)
				}
				return cmds, false
			}

			// Dismiss fallback banner on user input
			if m.fallbackBanner != "" {
				m.fallbackBanner = ""
				m.renderMessages()
			}

			input := strings.TrimSpace(m.textarea.Value())
			if input == "" {
				var cmds []tea.Cmd
				return cmds, false
			}
			m.inputHistory = append(m.inputHistory, input)
			m.historyPos = len(m.inputHistory)

			// If input is a slash command, emit it for app-level handling
			if strings.HasPrefix(input, "/") {
				m.textarea.Reset()
				var cmds []tea.Cmd
				cmds = append(cmds, func() tea.Msg {
					return SlashCommandMsg{Command: input}
				})
				return cmds, false
			}

			userMsg := types.Message{
				Role:      "user",
				Content:   input,
				CreatedAt: time.Now(),
			}
			m.messages = append(m.messages, userMsg)
			m.renderMessages()
			m.viewport.GotoBottom()
			m.textarea.Reset()

			// Start streaming LLM response
			if m.registry != nil && m.activeProvider != "" {
				p := m.registry.ActiveProvider()
				if p != nil {
					modelID := ""
					if m.activeModel != nil {
						modelID = m.activeModel.ID
					} else {
						// No model configured — show helpful error
						errMsg := types.Message{
							Role:    "assistant",
							Content: "No model selected. Set one up via /settings or /model command.",
							Segments: []types.MessageSegment{{
								Type:    "content",
								Content: "No model selected. Set one up via /settings or /model command.",
								Visible: true,
							}},
							CreatedAt: time.Now(),
						}
						m.messages = append(m.messages, errMsg)
						m.renderMessages()
						m.viewport.GotoBottom()
						var cmds []tea.Cmd
						return cmds, true
					}

					// Auto-arbitrage: if enabled, check if a cheaper model can handle this task
					if m.cfg != nil && m.cfg.Model.AutoArbitrage && m.activeModel != nil {
						fetchCtx, fetchCancel := context.WithTimeout(context.Background(), 15*time.Second)
						allModels, err := p.FetchModels(fetchCtx)
						fetchCancel()
						if err == nil && len(allModels) > 0 {
							task := types.Task{
								Description: input,
								Files:       []string{},
							}
							rec, err := arbitrage.Recommend(allModels, task, m.cfg.Model.ArbitrageThreshold)
							if err == nil && rec != nil {
								// If the recommended model differs from current and is cheaper, switch
								currentCost := m.activeModel.Pricing.OutputPerMToken
								if rec.RecommendedModel.ModelID != modelID && rec.RecommendedModel.OutputCost < currentCost {
									// Switch to recommended model for this request
									modelID = rec.RecommendedModel.ModelID
								}
							}
						}
					}

					ctx, cancel := context.WithCancel(context.Background())
					m.streamCancel = cancel
					m.streaming = true
					m.thinking = false
					m.thinkingStartAt = time.Time{}
					m.activeSegmentType = ""
					m.streamContent.Reset()
					m.streamSegments = nil

					req := provider.ChatRequest{
						Model:    modelID,
						Messages: m.messages,
						Stream:   true,
					}
					m.streamCh = make(chan tea.Msg, 100)
					m.streamDone = make(chan struct{})
					cmd := StartStreamCmd(ctx, p, req, m.sessionID, m.streamCh, m.streamDone)
					return []tea.Cmd{cmd}, true
				}
			}

			// No provider configured — show helpful error
			errMsg := types.Message{
				Role:    "assistant",
				Content: "No AI provider configured. Set up an API key via /config or restart M31A to run first-run setup.",
				Segments: []types.MessageSegment{{
					Type:    "content",
					Content: "No AI provider configured. Set up an API key via /config or restart M31A to run first-run setup.",
					Visible: true,
				}},
				CreatedAt: time.Now(),
			}
			m.messages = append(m.messages, errMsg)
			m.renderMessages()
			m.viewport.GotoBottom()

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

		case "t":
			// Toggle focused thinking block (or first collapsed if none focused)
			if m.textarea.Value() == "" && len(m.thinkingBlocks) > 0 {
				m.toggleFocusedThinkingBlock()
				m.renderMessages()
				var cmds []tea.Cmd
				return cmds, false
			}
			var cmds []tea.Cmd
			return cmds, false

		case "T":
			// Toggle ALL thinking blocks (preserve existing behavior)
			if m.textarea.Value() == "" {
				m.toggleAllThinkingBlocks()
				m.renderMessages()
				var cmds []tea.Cmd
				return cmds, false
			}
			var cmds []tea.Cmd
			return cmds, false

		case "tab":
			// Cycle focus through thinking blocks
			if m.textarea.Value() == "" && len(m.thinkingBlocks) > 0 {
				m.cycleThinkingFocus()
				m.renderMessages()
				var cmds []tea.Cmd
				return cmds, false
			}
			var cmds []tea.Cmd
			return cmds, false

		case "shift+tab":
			// Cycle focus backwards through thinking blocks
			if m.textarea.Value() == "" && len(m.thinkingBlocks) > 0 {
				m.cycleThinkingFocusBackward()
				m.renderMessages()
				var cmds []tea.Cmd
				return cmds, false
			}
			var cmds []tea.Cmd
			return cmds, false

		case "x":
			// Dismiss fallback banner
			if m.fallbackBanner != "" {
				m.fallbackBanner = ""
				m.renderMessages()
			}
			var cmds []tea.Cmd
			return cmds, false

		case "esc":
			// Dismiss fallback banner on escape
			if m.fallbackBanner != "" {
				m.fallbackBanner = ""
				m.renderMessages()
			}
			m.textarea.Reset()
			var cmds []tea.Cmd
			return cmds, false
		}

		// Dismiss fallback banner on any key press when textarea has content
		if m.fallbackBanner != "" && m.textarea.Value() != "" {
			m.fallbackBanner = ""
			m.renderMessages()
		}

	case FallbackEventMsg:
		m.fallbackBanner = fmt.Sprintf("Provider switched: %s → %s (%s)", msg.From, msg.To, msg.Reason)
		m.fallbackBannerAt = time.Now().Add(15 * time.Second)
		m.renderMessages()
		var cmds []tea.Cmd
		return cmds, false

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
		if m.activeSegmentType == "thinking" && m.streamContent.Len() > 0 {
			m.streamSegments = append(m.streamSegments, types.MessageSegment{
				Type:    "thinking",
				Content: m.streamContent.String(),
				Visible: true,
			})
			m.streamContent.Reset()
		}
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
		if m.thinkingStartAt.IsZero() {
			m.thinkingStartAt = time.Now()
		}

		if chunk.Delta != "" {
			m.streamContent.WriteString(chunk.Delta)
		}
	case "done":
	}

	m.renderMessages()
	m.viewport.GotoBottom()

	// Continuation: schedule next read from stream channel.
	// Must also watch streamDone so the cmd exits when the stream
	// goroutine terminates (normal completion or cancellation).
	streamCh := m.streamCh
	streamDone := m.streamDone
	nextCmd := func() tea.Msg {
		select {
		case msg := <-streamCh:
			return msg
		case <-streamDone:
			// Stream goroutine exited — drain any remaining messages from
			// the channel, then return nil to stop the continuation chain.
			select {
			case msg := <-streamCh:
				return msg
			default:
				return nil
			}
		}
	}

	return []tea.Cmd{nextCmd}, false
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

	// Populate thinking blocks from finalized segments
	m.thinkingBlocks = make(map[int]*components.ThinkingBlock)
	for i, seg := range m.streamSegments {
		if seg.Type == "thinking" {
			tb := components.NewThinkingBlock(seg, m.theme, false, i)
			m.thinkingBlocks[i] = tb
		}
	}

	// Populate tool cards from tool calls
	m.toolCards = make(map[int]*components.ToolCard)
	for i, tc := range msg.Message.ToolCalls {
		card := components.NewToolCard(tc, nil, components.ToolRunning, m.theme)
		m.toolCards[i] = card
	}

	m.streaming = false
	m.thinking = false
	m.activeSegmentType = ""
	m.streamSegments = nil
	m.textarea.Focus()

	// Capture usage and cost from stream
	if msg.Usage != nil {
		m.lastUsage = msg.Usage
		if m.activeModel != nil {
			p := m.activeModel.Pricing
			m.lastCost = (p.InputPerMToken * float64(msg.Usage.PromptTokens) / 1e6) +
				(p.OutputPerMToken * float64(msg.Usage.CompletionTokens) / 1e6)
		}
	}

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
	m.thinkingBlocks = make(map[int]*components.ThinkingBlock)
	m.toolCards = make(map[int]*components.ToolCard)
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
	var toolCalls []types.ToolCall
	for _, seg := range m.streamSegments {
		if seg.Type == "tool_use" && seg.Content != "" {
			var tc types.ToolCall
			if err := json.Unmarshal([]byte(seg.Content), &tc); err == nil {
				toolCalls = append(toolCalls, tc)
			}
		}
	}
	return toolCalls
}

func (m *ReplModel) SetTheme(t theme.Theme) {
	m.theme = t
	if m.msgRenderer != nil {
		newRenderer, err := components.NewMessageRenderer(t, m.width-4)
		if err == nil {
			m.msgRenderer = newRenderer
		}
	}
}

// SetSidebarWidth updates the reserved width for the sidebar and recalculates
// the REPL's internal widths. Call this when the sidebar is shown/hidden.
func (m *ReplModel) SetSidebarWidth(sw int) {
	m.sidebarWidth = sw
	// Recalculate layout with current window dimensions
	replWidth := m.width - sw
	if replWidth < 20 {
		replWidth = 20
	}
	m.viewport.Width = replWidth
	if m.msgRenderer != nil {
		_ = m.msgRenderer.SetWidth(replWidth - 4)
	}
	m.textarea.SetWidth(replWidth)
}

func (m *ReplModel) SetProvider(registry *provider.Registry, activeProvider string, model *types.ModelInfo, sessionID string, cfg *config.Config) {
	m.registry = registry
	m.activeProvider = activeProvider
	m.activeModel = model
	m.sessionID = sessionID
	m.cfg = cfg
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
				Type:      "thinking",
				Content:   partial,
				Visible:   true,
				StartedAt: m.thinkingStartAt,
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

	// Render fallback banner if active. Expiry is checked atomically in
	// Update() above, so View() only needs to test whether the banner is set.
	if m.fallbackBanner != "" {
		bannerStyle := lipgloss.NewStyle().
			Background(lipgloss.Color("#FDD663")).
			Foreground(lipgloss.Color("#000000")).
			Padding(0, 1).
			Bold(true).
			Width(m.width)
		banner := bannerStyle.Render("⚠ " + m.fallbackBanner)
		return lipgloss.JoinVertical(
			lipgloss.Top,
			banner,
			viewportStr,
			inputStr,
		)
	}

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

func (m *ReplModel) toggleAllThinkingBlocks() {
	if len(m.thinkingBlocks) == 0 {
		return
	}

	// Determine current state: all expanded, all collapsed, or mixed
	allExpanded := true
	allCollapsed := true
	for _, block := range m.thinkingBlocks {
		if block.IsExpanded() {
			allCollapsed = false
		} else {
			allExpanded = false
		}
	}

	// If all expanded → collapse all. If all hidden → expand all. Mixed → collapse all.
	collapse := allExpanded || (!allExpanded && !allCollapsed)
	for _, block := range m.thinkingBlocks {
		if collapse {
			if block.IsExpanded() {
				block.Toggle()
			}
		} else {
			if !block.IsExpanded() {
				block.Toggle()
			}
		}
	}
}

// toggleFocusedThinkingBlock toggles the focused block, or focuses the first collapsed one.
func (m *ReplModel) toggleFocusedThinkingBlock() {
	if len(m.thinkingBlocks) == 0 {
		return
	}

	// Clear all focus first
	for _, block := range m.thinkingBlocks {
		block.SetFocused(false)
	}

	// If we have a valid focused block, toggle it
	if m.thinkingFocusIndex >= 0 {
		if block, ok := m.thinkingBlocks[m.thinkingFocusIndex]; ok {
			block.Toggle()
			block.SetFocused(true)
			return
		}
	}

	// No valid focus: find first collapsed block and focus+toggle it
	for idx, block := range m.thinkingBlocks {
		if !block.IsExpanded() {
			block.Toggle()
			block.SetFocused(true)
			m.thinkingFocusIndex = idx
			return
		}
	}

	// All expanded: focus first one
	for idx, block := range m.thinkingBlocks {
		block.SetFocused(false)
		if idx == 0 {
			block.SetFocused(true)
		}
	}
	m.thinkingFocusIndex = 0
}

// cycleThinkingFocus moves focus to the next thinking block.
func (m *ReplModel) cycleThinkingFocus() {
	if len(m.thinkingBlocks) == 0 {
		return
	}

	// Clear current focus
	for _, block := range m.thinkingBlocks {
		block.SetFocused(false)
	}

	// Get sorted indices
	indices := make([]int, 0, len(m.thinkingBlocks))
	for id := range m.thinkingBlocks {
		indices = append(indices, id)
	}
	// Simple sort
	for i := 0; i < len(indices); i++ {
		for j := i + 1; j < len(indices); j++ {
			if indices[j] < indices[i] {
				indices[i], indices[j] = indices[j], indices[i]
			}
		}
	}

	// Find next index after current focus
	nextIdx := 0
	for i, id := range indices {
		if id == m.thinkingFocusIndex {
			nextIdx = (i + 1) % len(indices)
			break
		}
	}

	m.thinkingFocusIndex = indices[nextIdx]
	m.thinkingBlocks[m.thinkingFocusIndex].SetFocused(true)
}

// cycleThinkingFocusBackward moves focus to the previous thinking block.
func (m *ReplModel) cycleThinkingFocusBackward() {
	if len(m.thinkingBlocks) == 0 {
		return
	}

	for _, block := range m.thinkingBlocks {
		block.SetFocused(false)
	}

	indices := make([]int, 0, len(m.thinkingBlocks))
	for id := range m.thinkingBlocks {
		indices = append(indices, id)
	}
	for i := 0; i < len(indices); i++ {
		for j := i + 1; j < len(indices); j++ {
			if indices[j] < indices[i] {
				indices[i], indices[j] = indices[j], indices[i]
			}
		}
	}

	prevIdx := len(indices) - 1
	for i, id := range indices {
		if id == m.thinkingFocusIndex {
			prevIdx = (i - 1 + len(indices)) % len(indices)
			break
		}
	}

	m.thinkingFocusIndex = indices[prevIdx]
	m.thinkingBlocks[m.thinkingFocusIndex].SetFocused(true)
}

// ShowQuestion displays a question from the AskUserQuestion tool inline in the REPL.
func (m *ReplModel) ShowQuestion(msg QuestionRequestMsg) {
	m.activeQuestion = &msg

	// Add a system message showing the question
	questionText := components.FormatQuestion(msg.Question, msg.Header, msg.Options, m.width, m.theme)
	questionMsg := types.Message{
		Role:      "system",
		Content:   questionText,
		CreatedAt: time.Now(),
	}
	m.messages = append(m.messages, questionMsg)
	m.renderMessages()
	m.viewport.GotoBottom()

	// Focus the textarea for user input
	m.textarea.Focus()
	m.textarea.Placeholder = "Type your answer and press Enter..."
}

// HandleQuestionInput processes user input when a question is active.
// Returns a tea.Cmd that sends the answer back to the tool, or nil if no answer.
func (m *ReplModel) HandleQuestionInput() tea.Cmd {
	if m.activeQuestion == nil {
		return nil
	}

	answer := strings.TrimSpace(m.textarea.Value())
	if answer == "" {
		return nil
	}

	m.textarea.Reset()
	m.activeQuestion = nil
	m.textarea.Placeholder = "Type a message, /command, or goal..."
	m.renderMessages()
	m.viewport.GotoBottom()

	return func() tea.Msg {
		return QuestionResponseMsg{Answer: answer}
	}
}

// LastUsage returns the usage from the last completed stream.
func (m *ReplModel) LastUsage() *types.Usage {
	return m.lastUsage
}

// LastCost returns the estimated cost of the last completed stream.
func (m *ReplModel) LastCost() float64 {
	return m.lastCost
}
