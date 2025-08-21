package tui

import (
	"context"
	"fmt"
	"log/slog"
	"regexp"
	"strings"
	"time"

	"github.com/charmbracelet/bubbles/spinner"
	"github.com/charmbracelet/bubbles/textarea"
	"github.com/charmbracelet/bubbles/viewport"
	tea "github.com/charmbracelet/bubbletea"
	"github.com/eshanized/M31A/internal/config"
	"github.com/eshanized/M31A/internal/provider"
	"github.com/eshanized/M31A/internal/tools"
	"github.com/eshanized/M31A/internal/tui/components"
	"github.com/eshanized/M31A/internal/tui/theme"
	"github.com/eshanized/M31A/internal/types"
	"github.com/eshanized/M31A/pkg/arbitrage"
)

// fileRefPattern matches @filepath references for editor context auto-include.
// It captures @./path, @../path, @path/to/file, and @/absolute/path.
// It does NOT match standalone @user or @mention (requires a path separator '/').
var fileRefPattern = regexp.MustCompile(`@(\./|\.\./|[^\s@]+/)[^\s@]+`)

// ShellResultMsg is sent when a shell command (!prefix) completes execution.
type ShellResultMsg struct {
	Command string
	Output  string
	Err     string
}

type ReplModel struct {
	theme          theme.Theme
	version        string
	messages       []types.Message
	viewport       viewport.Model
	textarea       textarea.Model
	spinner        spinner.Model
	scrollPos      int
	inputHistory   []string
	historyPos     int
	frecentHistory *FrecentHistory
	placeholder    string
	streaming      bool
	thinking       bool
	lastStatus     string
	width          int
	height         int
	sidebarWidth   int // width reserved for sidebar (0 if hidden)

	msgRenderer  *components.MessageRenderer
	streamCancel context.CancelFunc

	currentMessage    *types.Message
	streamSegments    []types.MessageSegment
	streamContent     strings.Builder
	thinkingStartAt   time.Time
	activeSegmentType string
	thinkingBlocks    map[int]*components.ThinkingBlock
	toolCards         map[int]*components.ToolCard

	lastArbitrageFetch time.Time // throttle auto-arbitrage model fetches

	fallbackBanner   string    // current fallback banner text, empty = no banner
	fallbackBannerAt time.Time // when the banner appeared (for 15s auto-dismiss)

	// Active question from AskUserQuestion tool
	activeQuestion *QuestionRequestMsg

	// Provider access for LLM calls
	registry       *provider.Registry
	activeProvider string
	activeModel    *types.ModelInfo
	modelValid     bool // false when the active model is not in the current provider's catalog
	sessionID      string
	cwd            string // working directory for @filepath resolution

	// Config access for arbitrage settings
	cfg *config.Config

	// Dispatcher for shell mode (! prefix) command execution
	dispatcher *tools.Dispatcher

	// Fix C-3: streamCh is a read-only reference to the channel owned by
	// StartStreamCmd's goroutine. The REPL never creates or closes this
	// channel — it only reads from it via continuation cmds. The goroutine
	// owns the write side and closes streamCh when the stream completes.
	streamCh <-chan tea.Msg

	// Last stream usage and cost (from StreamDoneMsg)
	lastUsage *types.Usage
	lastCost  float64

	// Per-block thinking focus
	thinkingFocusIndex int // -1 = no focus, otherwise index into thinkingBlocks

	// References for View() rendering
	keyRegistry  *KeyRegistry
	lastActivity time.Time

	// Slash command autocomplete
	slashSuggestions []CommandInfo
	slashSelected    int
	slashVisible     bool
	cmdRegistry      *CommandRegistry

	// Recent session activity sparkline (populated by AppState via SetSessionSparkline)
	sessionSparkline string

	// Auto-scroll control: track if user has manually scrolled up
	userScrolled bool

	// Debounce for thinking block toggle during streaming
	lastToggleAt time.Time
}

// autoScrollConditionally scrolls to bottom only if the user hasn't manually scrolled up.
func (m *ReplModel) autoScrollConditionally() {
	if !m.userScrolled {
		m.viewport.GotoBottom()
	}
}

// AtBottom returns true if the viewport is at or near the bottom.
func (m *ReplModel) atBottom() bool {
	return m.viewport.AtBottom()
}

// MaxMessageHistory is the maximum number of messages retained in the REPL.
const MaxMessageHistory = 1000

func NewReplModel(t theme.Theme, version string) ReplModel {
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
		version:        version,
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

func (m *ReplModel) Update(msg tea.Msg) ([]tea.Cmd, bool) {
	// Check if fallback banner has expired
	if m.fallbackBanner != "" && time.Now().After(m.fallbackBannerAt) {
		m.fallbackBanner = ""
	}

	// Handle slash command autocomplete key interactions
	if m.slashVisible {
		if keyMsg, ok := msg.(tea.KeyMsg); ok {
			switch keyMsg.String() {
			case "tab":
				// Autocomplete the selected suggestion
				if m.slashSelected >= 0 && m.slashSelected < len(m.slashSuggestions) {
					sel := m.slashSuggestions[m.slashSelected]
					current := m.textarea.Value()
					parts := strings.Fields(current)
					if len(parts) > 0 && strings.HasPrefix(parts[0], "/") {
						// Replace the partial command with the full suggestion
						parts[0] = sel.Slash
						completed := strings.Join(parts, " ")
						m.textarea.SetValue(completed)
						// Move cursor to end
						m.textarea.CursorEnd()
					}
				}
				m.slashVisible = false
				m.slashSuggestions = nil
				return nil, false
			case "up":
				if m.slashSelected > 0 {
					m.slashSelected--
				}
				return nil, false
			case "down":
				if m.slashSelected < len(m.slashSuggestions)-1 {
					m.slashSelected++
				}
				return nil, false
			case "esc":
				m.slashVisible = false
				m.slashSuggestions = nil
				return nil, false
			}
		}
	}

	switch msg := msg.(type) {
	case tea.WindowSizeMsg:
		m.width = msg.Width
		m.height = msg.Height
		inputHeight := 3
		// Account for textarea(3) + metadataRow(1) + bottomBorder(1) + statusBar(1) = 6 total chrome lines.
		// The viewport itself has no borders. Previous value of 8 was wrong
		// (counted phantom "viewport borders" and "thinking indicator" lines that
		// don't exist in the current layout).
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
			m.autoScrollConditionally()
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
			case "t":
				// Toggle focused thinking block during streaming (debounced)
				if len(m.thinkingBlocks) > 0 && time.Since(m.lastToggleAt) > 100*time.Millisecond {
					m.lastToggleAt = time.Now()
					m.toggleFocusedThinkingBlock()
					m.renderMessages()
				}
				var cmds []tea.Cmd
				return cmds, false
			case "T":
				// Toggle ALL thinking blocks during streaming (debounced)
				if len(m.thinkingBlocks) > 0 && time.Since(m.lastToggleAt) > 100*time.Millisecond {
					m.lastToggleAt = time.Now()
					m.toggleAllThinkingBlocks()
					m.renderMessages()
				}
				var cmds []tea.Cmd
				return cmds, false
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

			// Upsert into frecency history for LLM-bound prompts
			// Only upsert non-shell, non-slash inputs (actual LLM prompts)
			isShellMode := strings.HasPrefix(input, "!") || strings.HasPrefix(input, "\uff01")
			isSlashCmd := strings.HasPrefix(input, "/")
			if !isShellMode && !isSlashCmd && m.frecentHistory != nil {
				m.frecentHistory.Upsert(input)
				// Non-blocking save — fire and forget
				go func() {
					if err := m.frecentHistory.Save(); err != nil {
						slog.Debug("failed to save prompt history", "error", err)
					}
				}()
			}

			// Shell mode: ! prefix bypasses LLM for direct command execution
			// Must come BEFORE the / prefix check so !/bin/ls isn't caught as a slash command.
			if strings.HasPrefix(input, "!") || strings.HasPrefix(input, "\uff01") {
				command := strings.TrimPrefix(input, "!")
				command = strings.TrimPrefix(command, "\uff01")
				command = strings.TrimSpace(command)
				if command == "" {
					// Show help for shell mode
					errMsg := types.Message{
						Role:    "assistant",
						Content: "Shell mode: type `!command` to execute a shell command directly (e.g., `!git status`).",
						Segments: []types.MessageSegment{{
							Type:    "content",
							Content: "Shell mode: type `!command` to execute a shell command directly (e.g., `!git status`).",
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
				m.textarea.Reset()
				return m.executeShellCommand(command)
			}

			// If input is a slash command, emit it for app-level handling
			if strings.HasPrefix(input, "/") {
				m.textarea.Reset()
				var cmds []tea.Cmd
				cmds = append(cmds, func() tea.Msg {
					return SlashCommandMsg{Command: input}
				})
				return cmds, false
			}

			// Expand @filepath references for LLM context
			expandedInput := m.expandFileRefs(input)

			userMsg := types.Message{
				Role:      "user",
				Content:   expandedInput,
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

					// Auto-arbitrage: if enabled, check if a cheaper model can handle this task.
					// Throttle fetches to once per 5 minutes; FetchModels returns cached
					// data when the cache is still fresh.
					if m.cfg != nil && m.cfg.Model.AutoArbitrage && m.activeModel != nil {
						if time.Since(m.lastArbitrageFetch) > 5*time.Minute || m.lastArbitrageFetch.IsZero() {
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
							m.lastArbitrageFetch = time.Now()
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
						Messages: m.messagesForLLM(),
					}
					// Fix C-3: StartStreamCmd owns its channels internally.
					// The REPL stores a read-only reference for continuation only.
					cmd, streamCh := StartStreamCmd(ctx, p, req, m.sessionID)
					m.streamCh = streamCh
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
			// Use frecency history if available and textarea has content (prefix matching)
			currentText := m.textarea.Value()
			if m.frecentHistory != nil && len(currentText) > 0 {
				results := m.frecentHistory.Search(currentText, 10)
				if len(results) > 0 {
					// Cycle through results
					if m.historyPos == -1 {
						m.historyPos = 0
					} else if m.historyPos < len(results)-1 {
						m.historyPos++
					} else {
						m.historyPos = 0 // wrap around
					}
					m.textarea.SetValue(results[m.historyPos].Text)
					m.textarea.SetCursor(len(m.textarea.Value()))
					var cmds []tea.Cmd
					return cmds, false
				}
			}
			// Fallback to in-memory session history
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
			// Navigate through history items
			if m.frecentHistory != nil && m.historyPos > 0 {
				currentText := m.textarea.Value()
				results := m.frecentHistory.Search(currentText, 10)
				if len(results) > 0 && m.historyPos > 0 {
					m.historyPos--
					m.textarea.SetValue(results[m.historyPos].Text)
					m.textarea.SetCursor(len(m.textarea.Value()))
					var cmds []tea.Cmd
					return cmds, false
				}
			}
			// Fallback to in-memory session history
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
			m.userScrolled = true
			var cmds []tea.Cmd
			return cmds, false

		case "pgdown":
			m.viewport.HalfViewDown()
			// Reset auto-scroll if user scrolls back to bottom
			if m.viewport.AtBottom() {
				m.userScrolled = false
			}
			var cmds []tea.Cmd
			return cmds, false

		case "t":
			// Toggle focused thinking block (or first collapsed if none focused)
			// Only when textarea is empty and there are thinking blocks
			if m.textarea.Value() == "" && len(m.thinkingBlocks) > 0 {
				m.toggleFocusedThinkingBlock()
				m.renderMessages()
				var cmds []tea.Cmd
				return cmds, false
			}
			// Fall through to textarea when typing

		case "T":
			// Toggle ALL thinking blocks (preserve existing behavior)
			// Only when textarea is empty
			if m.textarea.Value() == "" {
				m.toggleAllThinkingBlocks()
				m.renderMessages()
				var cmds []tea.Cmd
				return cmds, false
			}
			// Fall through to textarea when typing

		case "tab":
			// Cycle focus through thinking blocks
			// Only when textarea is empty and there are thinking blocks
			if m.textarea.Value() == "" && len(m.thinkingBlocks) > 0 {
				m.cycleThinkingFocus()
				m.renderMessages()
				var cmds []tea.Cmd
				return cmds, false
			}
			// Fall through to textarea when typing

		case "shift+tab":
			// Cycle focus backwards through thinking blocks
			// Only when textarea is empty and there are thinking blocks
			if m.textarea.Value() == "" && len(m.thinkingBlocks) > 0 {
				m.cycleThinkingFocusBackward()
				m.renderMessages()
				var cmds []tea.Cmd
				return cmds, false
			}
			// Fall through to textarea when typing

		case "x":
			// Dismiss fallback banner only when banner is visible
			if m.fallbackBanner != "" {
				m.fallbackBanner = ""
				m.renderMessages()
				var cmds []tea.Cmd
				return cmds, false
			}
			// Fall through to textarea when no banner to dismiss

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

	case ShellResultMsg:
		// Replace the "Running..." message with actual command output
		formatted := fmt.Sprintf("$ %s\n%s", msg.Command, msg.Output)
		if msg.Err != "" {
			formatted += fmt.Sprintf("\n[Error: %s]", msg.Err)
		}
		// Find the last assistant message with $ prefix and update it
		for i := len(m.messages) - 1; i >= 0; i-- {
			if m.messages[i].Role == "assistant" && strings.HasPrefix(m.messages[i].Content, "$ ") {
				m.messages[i].Content = formatted
				m.messages[i].Segments = []types.MessageSegment{{
					Type:    "content",
					Content: formatted,
					Visible: true,
				}}
				break
			}
		}
		m.renderMessages()
		m.viewport.GotoBottom()
		var cmds []tea.Cmd
		return cmds, true

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

	// Detect slash command typing and update suggestions
	if m.cmdRegistry != nil {
		current := m.textarea.Value()
		if strings.HasPrefix(current, "/") {
			// Extract the partial command (first word after /)
			parts := strings.Fields(current)
			partial := ""
			if len(parts) > 0 {
				partial = strings.TrimPrefix(parts[0], "/")
			}

			// Generate matching commands
			allCmds := m.cmdRegistry.AllCommands()
			m.slashSuggestions = nil
			if partial == "" {
				// Show all commands when just "/" is typed
				m.slashSuggestions = allCmds
			} else {
				// Filter by partial match
				q := strings.ToLower(partial)
				for _, cmd := range allCmds {
					name := strings.ToLower(cmd.Name)
					slash := strings.ToLower(cmd.Slash)
					if strings.HasPrefix(name, q) || strings.HasPrefix(slash, q) || strings.Contains(name, q) {
						m.slashSuggestions = append(m.slashSuggestions, cmd)
					}
				}
			}

			// Show suggestions if we have matches and more than one option
			if len(m.slashSuggestions) > 0 {
				m.slashVisible = true
				m.slashSelected = 0
				// Limit to 8 suggestions
				if len(m.slashSuggestions) > 8 {
					m.slashSuggestions = m.slashSuggestions[:8]
				}
			} else {
				m.slashVisible = false
			}
		} else {
			m.slashVisible = false
			m.slashSuggestions = nil
		}
	}

	cmds = append(cmds, taCmd, vpCmd, spCmd)
	return cmds, false
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

// ProviderModelsFetchedMsg is returned when FetchModels completes asynchronously.
type ProviderModelsFetchedMsg struct {
	Models []types.ModelInfo
	Model  *types.ModelInfo
	Err    error
}

// SetProvider configures the active provider and returns a tea.Cmd that
// asynchronously validates the model by fetching the provider's model catalog.
// The returned cmd performs FetchModels in the background and emits a
// ProviderModelsFetchedMsg when complete, keeping the TUI responsive.
func (m *ReplModel) SetProvider(registry *provider.Registry, activeProvider string, model *types.ModelInfo, sessionID string, cfg *config.Config) tea.Cmd {
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

	// Return an async command that validates the model against the provider's catalog
	p := registry.ActiveProvider()
	if p == nil {
		return nil
	}
	return func() tea.Msg {
		ctx, cancel := context.WithTimeout(context.Background(), 15*time.Second)
		defer cancel()
		models, err := p.FetchModels(ctx)
		return ProviderModelsFetchedMsg{Models: models, Model: model, Err: err}
	}
}

// handleProviderModelsFetched processes the async result of SetProvider's FetchModels call.
func (m *ReplModel) handleProviderModelsFetched(msg ProviderModelsFetchedMsg) {
	if msg.Err != nil || msg.Model == nil {
		return
	}
	found := false
	for _, model := range msg.Models {
		if model.ID == msg.Model.ID {
			found = true
			break
		}
	}
	if found {
		// Refresh cached model info (pricing, context length may differ between providers)
		if m.registry != nil {
			p := m.registry.ActiveProvider()
			if p != nil {
				if info, _ := p.GetModel(msg.Model.ID); info != nil {
					m.activeModel = info
					return
				}
			}
		}
	}
	// Model not found on new provider
	m.modelValid = false
}

func (m *ReplModel) SetDispatcher(d *tools.Dispatcher) {
	m.dispatcher = d
}

func (m *ReplModel) SetCommandRegistry(reg *CommandRegistry) {
	m.cmdRegistry = reg
}

// SetFrecentHistory sets the frecency history for prompt history navigation.
func (m *ReplModel) SetFrecentHistory(fh *FrecentHistory) {
	m.frecentHistory = fh
}

// SetCwd sets the working directory for @filepath resolution.
func (m *ReplModel) SetCwd(cwd string) {
	m.cwd = cwd
}

func (m *ReplModel) SetKeyRegistry(kr *KeyRegistry) {
	m.keyRegistry = kr
}

// SetSessionSparkline updates the recent-activity sparkline shown in the
// provider card on the welcome screen. Passing an empty string hides it.
func (m *ReplModel) SetSessionSparkline(spark string) {
	m.sessionSparkline = spark
}

func (m *ReplModel) SetLastActivity(t time.Time) {
	m.lastActivity = t
}

func (m *ReplModel) SetStreaming(v bool) {
	m.streaming = v
}

func (m *ReplModel) SetThinking(v bool) {
	m.thinking = v
}

// SetSessionID updates the session ID for this REPL model.
func (m *ReplModel) SetSessionID(id string) {
	m.sessionID = id
}

func (m *ReplModel) AddMessage(msg types.Message) {
	m.messages = append(m.messages, msg)
	if len(m.messages) > MaxMessageHistory {
		m.messages = m.messages[len(m.messages)-500:]
	}
	m.renderMessages()
	m.autoScrollConditionally()
}

func (m *ReplModel) InputValue() string {
	return strings.TrimSpace(m.textarea.Value())
}

func (m *ReplModel) Messages() []types.Message {
	return m.messages
}

// ClearMessages removes all messages from the REPL model and re-renders the viewport.
// M-38 fix: also resets streaming state so /clear works during active streams.
func (m *ReplModel) ClearMessages() {
	m.messages = nil
	// M-38 fix: reset streaming state
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

// LastUsage returns the usage from the last completed stream.
func (m *ReplModel) LastUsage() *types.Usage {
	return m.lastUsage
}

// LastCost returns the estimated cost of the last completed stream.
func (m *ReplModel) LastCost() float64 {
	return m.lastCost
}
