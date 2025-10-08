package tui

import (
	"context"
	"log/slog"
	"strings"
	"time"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/eshanized/M31A/internal/provider"
	"github.com/eshanized/M31A/internal/types"
	"github.com/eshanized/M31A/pkg/arbitrage"
)

// handleStreamingKeyMsg processes keyboard input while streaming is active.
func (m *ReplModel) handleStreamingKeyMsg(msg tea.KeyMsg) ([]tea.Cmd, bool) {
	switch msg.String() {
	case "ctrl+c":
		if m.streamCancel != nil {
			m.streamCancel()
		}
		m.streaming = false
		m.thinking = false
		m.textarea.Focus()
		m.renderMessages()
		return nil, true
	case "t":
		// Toggle focused thinking block during streaming (debounced)
		if len(m.thinkingBlocks) > 0 && time.Since(m.lastToggleAt) > 100*time.Millisecond {
			m.lastToggleAt = time.Now()
			m.toggleFocusedThinkingBlock()
			m.renderMessages()
		}
		return nil, false
	case "T":
		// Toggle ALL thinking blocks during streaming (debounced)
		if len(m.thinkingBlocks) > 0 && time.Since(m.lastToggleAt) > 100*time.Millisecond {
			m.lastToggleAt = time.Now()
			m.toggleAllThinkingBlocks()
			m.renderMessages()
		}
		return nil, false
	}
	return nil, false
}

// handleKeyMsg processes non-streaming keyboard input.
func (m *ReplModel) handleKeyMsg(msg tea.KeyMsg) ([]tea.Cmd, bool) {
	switch msg.String() {
	case "enter":
		return m.handleEnterKey()
	case "up":
		return m.handleHistoryUp()
	case "down":
		return m.handleHistoryDown()
	case "pgup":
		m.viewport.HalfViewUp()
		m.userScrolled = true
		return nil, false
	case "pgdown":
		m.viewport.HalfViewDown()
		if m.viewport.AtBottom() {
			m.userScrolled = false
		}
		return nil, false
	case "t":
		if m.textarea.Value() == "" && len(m.thinkingBlocks) > 0 {
			m.toggleFocusedThinkingBlock()
			m.renderMessages()
			return nil, false
		}
	case "T":
		if m.textarea.Value() == "" {
			m.toggleAllThinkingBlocks()
			m.renderMessages()
			return nil, false
		}
	case "tab":
		if len(m.slashSuggestions) > 0 && !m.slashVisible {
			m.slashVisible = true
			m.slashSelected = 0
			return nil, false
		}
		if m.textarea.Value() == "" && len(m.thinkingBlocks) > 0 {
			m.cycleThinkingFocus()
			m.renderMessages()
			return nil, false
		}
	case "shift+tab":
		if m.textarea.Value() == "" && len(m.thinkingBlocks) > 0 {
			m.cycleThinkingFocusBackward()
			m.renderMessages()
			return nil, false
		}
	case "x":
		if m.fallbackBanner != "" {
			m.fallbackBanner = ""
			m.renderMessages()
			return nil, false
		}
	case "esc":
		if m.fallbackBanner != "" {
			m.fallbackBanner = ""
			m.renderMessages()
		}
		m.textarea.Reset()
		return nil, false
	}

	// Dismiss fallback banner on any key press when textarea has content
	if m.fallbackBanner != "" && m.textarea.Value() != "" {
		m.fallbackBanner = ""
		m.renderMessages()
	}

	// Forward unmatched keys to the textarea so character input works.
	var taCmd tea.Cmd
	m.textarea, taCmd = m.textarea.Update(msg)

	// Update slash command suggestions after textarea changes
	if m.cmdRegistry != nil {
		current := m.textarea.Value()
		if strings.HasPrefix(current, "/") {
			parts := strings.Fields(current)
			partial := ""
			if len(parts) > 0 {
				partial = strings.TrimPrefix(parts[0], "/")
			}

			allCmds := m.cmdRegistry.AllCommands()
			m.slashSuggestions = nil
			if partial == "" {
				m.slashSuggestions = allCmds
			} else {
				q := strings.ToLower(partial)
				for _, cmd := range allCmds {
					name := strings.ToLower(cmd.Name)
					slash := strings.ToLower(cmd.Slash)
					if strings.HasPrefix(name, q) || strings.HasPrefix(slash, q) || strings.Contains(name, q) {
						m.slashSuggestions = append(m.slashSuggestions, cmd)
					}
				}
			}

			if len(m.slashSuggestions) > 0 {
				m.slashVisible = false
				m.slashSelected = 0
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

	return []tea.Cmd{taCmd}, false
}

// handleEnterKey processes the Enter key press.
func (m *ReplModel) handleEnterKey() ([]tea.Cmd, bool) {
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
		return nil, false
	}
	m.inputHistory = append(m.inputHistory, input)
	m.historyPos = len(m.inputHistory)

	// Upsert into frecency history for LLM-bound prompts
	isShellMode := strings.HasPrefix(input, "!") || strings.HasPrefix(input, "\uff01")
	isSlashCmd := strings.HasPrefix(input, "/")
	if !isShellMode && !isSlashCmd && m.frecentHistory != nil {
		m.frecentHistory.Upsert(input)
		go func() {
			if err := m.frecentHistory.Save(); err != nil {
				slog.Debug("failed to save prompt history", "error", err)
			}
		}()
	}

	// Shell mode: ! prefix bypasses LLM for direct command execution
	if strings.HasPrefix(input, "!") || strings.HasPrefix(input, "\uff01") {
		command := strings.TrimPrefix(input, "!")
		command = strings.TrimPrefix(command, "\uff01")
		command = strings.TrimSpace(command)
		if command == "" {
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
			return nil, true
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
				return nil, true
			}

			// Auto-arbitrage
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
							currentCost := m.activeModel.Pricing.OutputPerMToken
							if rec.RecommendedModel.ModelID != modelID && rec.RecommendedModel.OutputCost < currentCost {
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
			cmd, streamCh := StartStreamCmd(ctx, p, req, m.sessionID)
			m.streamCh = streamCh
			return []tea.Cmd{cmd}, true
		}
	}

	// No provider configured
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

	return nil, true
}

// handleHistoryUp navigates to the previous input in history.
func (m *ReplModel) handleHistoryUp() ([]tea.Cmd, bool) {
	currentText := m.textarea.Value()
	if m.frecentHistory != nil && len(currentText) > 0 {
		results := m.frecentHistory.Search(currentText, 10)
		if len(results) > 0 {
			if m.historyPos == -1 {
				m.historyPos = 0
			} else if m.historyPos < len(results)-1 {
				m.historyPos++
			} else {
				m.historyPos = 0
			}
			m.textarea.SetValue(results[m.historyPos].Text)
			m.textarea.SetCursor(len(m.textarea.Value()))
			return nil, false
		}
	}
	if m.historyPos == -1 || len(m.inputHistory) == 0 {
		return nil, false
	}
	if m.historyPos > 0 && m.historyPos <= len(m.inputHistory) {
		m.historyPos--
		m.textarea.SetValue(m.inputHistory[m.historyPos])
		m.textarea.SetCursor(len(m.textarea.Value()))
	}
	return nil, false
}

// handleHistoryDown navigates to the next input in history.
func (m *ReplModel) handleHistoryDown() ([]tea.Cmd, bool) {
	if m.frecentHistory != nil && m.historyPos > 0 {
		currentText := m.textarea.Value()
		results := m.frecentHistory.Search(currentText, 10)
		if len(results) > 0 && m.historyPos > 0 {
			m.historyPos--
			m.textarea.SetValue(results[m.historyPos].Text)
			m.textarea.SetCursor(len(m.textarea.Value()))
			return nil, false
		}
	}
	if m.historyPos < len(m.inputHistory)-1 {
		m.historyPos++
		m.textarea.SetValue(m.inputHistory[m.historyPos])
		m.textarea.SetCursor(len(m.textarea.Value()))
	} else {
		m.historyPos = len(m.inputHistory)
		m.textarea.Reset()
	}
	return nil, false
}

// handleShellResult processes the result of a shell command execution.
func (m *ReplModel) handleShellResult(msg ShellResultMsg) ([]tea.Cmd, bool) {
	formatted := "$ " + msg.Command + "\n" + msg.Output
	if msg.Err != "" {
		formatted += "\n[Error: " + msg.Err + "]"
	}
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
	return nil, true
}

// handleFallbackEvent processes a provider fallback event.
func (m *ReplModel) handleFallbackEvent(msg FallbackEventMsg) ([]tea.Cmd, bool) {
	m.fallbackBanner = "Provider switched: " + msg.From + " → " + msg.To + " (" + msg.Reason + ")"
	m.fallbackBannerAt = time.Now().Add(time.Duration(m.cfg.UI.FallbackBannerSecs) * time.Second)
	m.renderMessages()
	return nil, false
}
