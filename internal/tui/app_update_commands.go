package tui

import (
	"context"
	"fmt"
	"log/slog"
	"strings"
	"time"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/eshanized/M31A/internal/codeintel"
	m31errors "github.com/eshanized/M31A/internal/errors"
	"github.com/eshanized/M31A/internal/provider"
	"github.com/eshanized/M31A/internal/engine/tokens"
	"github.com/eshanized/M31A/internal/tui/streaming"
	"github.com/eshanized/M31A/internal/tui/theme"
	"github.com/eshanized/M31A/internal/types"
	"github.com/eshanized/M31A/internal/engine/workflow"
)

// handleSlashCommand routes slash commands to the command registry.
// It also handles special chat messages (non-slash input).
func (m *AppState) handleSlashCommand(input string, attachedFiles int) tea.Cmd {
	if input == "" {
		return nil
	}

	t := m.themeManager.Current()

	// Shell command (! prefix)
	if strings.HasPrefix(input, "!") {
		if m.replModel != nil {
			return m.replModel.executeShellCommand(input, m.shutdownCtx)
		}
		return nil
	}

	// Slash command
	if strings.HasPrefix(input, "/") {
		if m.cmdRegistry != nil {
			ctx := CommandContext{
				Ctx:             m.shutdownCtx,
				Registry:        m.registry,
				SessionManager:  m.sessionManager,
				SessionID:       m.sessionID,
				Config:          m.config,
				ConfigPath:      m.configPath,
				Dispatcher:      m.dispatcher,
				Git:             m.git,
				Ledger:          m.ledger,
				Rollback:        m.rollback,
				AutoDream:       m.autoDream,
				WorkflowEngine:  m.workflowEngine,
				CmdRegistry:     m.cmdRegistry,
				FrecentHistory:  m.frecentHistory,
				SubagentManager: m.subagentManager,
				AgentMode:       &m.agentMode,
				SetAgentMode:    func(v bool) { m.agentMode = v },
				QuickMode:       &m.quickMode,
				SetQuickMode:    func(v bool) { m.quickMode = v },
				CancelAgent: func() {
					if m.streamCancelFn != nil {
						m.streamCancelFn()
						m.streamCancelFn = nil
					}
					m.agentCh = nil
				},
				Version:  m.version,
				Keychain: m.keychain,
			}
			if m.replModel != nil {
				ctx.ClearMessages = m.replModel.ClearMessages
				ctx.FlushViewport = m.replModel.FlushViewport
				ctx.CopyError = m.replModel.copyLastError
			}
			result, handled := m.cmdRegistry.Execute(input, ctx)
			if handled {
				return m.processCommandResult(result)
			}
		}

		// Unknown command
		if m.replModel != nil {
			m.replModel.AddMessage(MakeAssistantMsg(
				"Unknown command. Type /help for available commands.",
			))
		}
		return nil
	}

	// Regular chat message — route to the LLM
	// Show toast for @-mention file attachments
	if attachedFiles > 0 {
		m.toasts = append(m.toasts, Toast{
			Text:      fmt.Sprintf("Attached %d file(s) via @-mention", attachedFiles),
			Type:      "info",
			CreatedAt: time.Now(),
		})
	}
	return m.sendChatMessage(input, t)
}

// processCommandResult converts a CommandResult into a tea.Cmd.
func (m *AppState) processCommandResult(result CommandResult) tea.Cmd {
	// Confirmation required — store pending and show prompt
	if result.ConfirmRequired {
		m.pendingConfirm = &result
		prompt := result.ConfirmPrompt
		if prompt == "" {
			prompt = "Are you sure?"
		}
		m.confirmPrompt = prompt
		if m.replModel != nil {
			m.replModel.AddMessage(MakeAssistantMsg(prompt + " (y/n)"))
		}
		return nil
	}

	// Show the result message in REPL if any
	if result.Message != "" && m.replModel != nil {
		m.replModel.AddMessage(MakeAssistantMsg(result.Message))
	}

	// Screen transition
	if result.Screen != nil {
		return m.navigateToScreen(*result.Screen)
	}

	// Session navigation
	if result.SessionID != nil {
		return m.loadAndRestoreSession(*result.SessionID, true)
	}

	// Workflow resume
	if result.WorkflowResume && result.ResumePhase != "" {
		m.workflowGoal = result.ResumeGoal
		m.workflowPhase = result.ResumePhase
		return m.runWorkflowFromGoal(result.ResumeGoal)
	}

	// Skip to phase
	if result.SkipToPhase != "" {
		m.workflowPhase = types.WorkflowPhase(result.SkipToPhase)
		return nil
	}

	// Cmd callback
	if result.Cmd != nil {
		cmd := result.Cmd
		return func() tea.Msg { return cmd() }
	}

	return nil
}

// sendChatMessage dispatches a user message to the active LLM provider.
// When agentMode is true, it starts the autonomous agent loop with tool use.
// When agentMode is false, it sends a plain text-only chat request.
func (m *AppState) sendChatMessage(input string, t theme.Theme) tea.Cmd {
	if m.registry == nil || m.activeProvider == "" {
		if m.replModel != nil {
			m.replModel.AddMessage(MakeAssistantMsg(
				"No provider configured. Run /settings to add an API key.",
			))
		}
		return nil
	}

	p := m.registry.ActiveProvider()
	if p == nil {
		if m.replModel != nil {
			m.replModel.AddMessage(MakeAssistantMsg(
				"Provider not available. Run /settings to configure.",
			))
		}
		return nil
	}

	if m.activeModel == nil {
		if m.config != nil && m.config.Model.Default != "" {
			defaultID := m.config.Model.Default
			if info, err := p.GetModel(defaultID); err == nil && info != nil {
				m.activeModel = info
			} else {
				m.activeModel = &types.ModelInfo{ID: defaultID}
			}
			if m.replModel != nil {
				m.replModel.activeModel = m.activeModel
			}
		}
		if m.activeModel == nil {
			if m.replModel != nil {
				m.replModel.AddMessage(MakeAssistantMsg(
					"No model selected. Run /model to choose one.",
				))
			}
			return nil
		}
	}

	_ = t

	// Intent classification: when enabled, pre-classify the input to route
	// between workflow and chat modes intelligently.
	if m.config != nil && m.config.Features.IntentClassification {
		// Ensure prompt registry is loaded for the classifier
		if m.promptRegistry == nil {
			registry, err := workflow.LoadPrompts(m.config.Prompts, "")
			if err == nil {
				m.promptRegistry = registry
			}
		}
		if m.promptRegistry != nil {
			return m.classifyAndRoute(p, input)
		}
	}

	// Autonomous agent mode: use agent loop with tool definitions
	if m.agentMode {
		return m.startAgentLoop(p, input)
	}

	// Plain text mode: no tools, single response
	return m.sendPlainTextChat(p, input)
}

// startAgentLaunches the autonomous agent loop with tool use.
func (m *AppState) startAgentLoop(p provider.LLMProvider, input string) tea.Cmd {
	// Load and cache prompts
	if m.promptRegistry == nil {
		registry, err := workflow.LoadPrompts(m.config.Prompts, "")
		if err != nil {
			if m.replModel != nil {
				m.replModel.AddMessage(MakeAssistantMsg(
					fmt.Sprintf("Failed to load prompts: %s", m31errors.UserMessage(err)),
				))
			}
			return nil
		}
		m.promptRegistry = registry
	}

	// Compose system prompt: base + autonomous + tool-use + context-awareness + code-quality + code-intelligence
	sysContent := m.promptRegistry.Base + "\n\n---\n\n" +
		m.promptRegistry.Autonomous + "\n\n---\n\n" +
		m.promptRegistry.ToolUse + "\n\n---\n\n" +
		m.promptRegistry.ContextAwareness + "\n\n---\n\n" +
		m.promptRegistry.CodeQuality + "\n\n---\n\n" +
		m.promptRegistry.CodeIntelligence

	// Load project context (AGENTS.md / MEMORY.md)
	projectCtx := LoadProjectContextForAgent(m.cwd)
	if projectCtx != "" {
		sysContent += "\n\n## Project Context (from AGENTS.md)\n\n" + projectCtx
	}

	// Codebase intelligence — build index and inject project summary
	idx := codeintel.NewIndexer(m.cwd)
	if err := idx.Build(m.shutdownCtx); err == nil {
		if summary := idx.ProjectSummary(4000); summary != "" {
			sysContent += "\n\n" + summary
		}
	}

	// Build message history
	var replMsgs []types.Message
	if m.replModel != nil {
		replMsgs = m.replModel.Messages()
	}
	msgs := BuildAgentMessages(replMsgs, input)

	// Smart truncation to prevent context overflow
	if m.activeModel != nil {
		estimator := tokens.NewEstimator(m.activeModel.ID)
		truncatedMsgs, truncated := streaming.TruncateMessagesForLLM(msgs, m.activeModel.ContextLength, estimator)
		if truncated {
			slog.Debug("truncated messages for LLM context window", "original", len(msgs), "kept", len(truncatedMsgs))
		}
		msgs = truncatedMsgs
	}

	ctx, cancel := context.WithCancel(m.shutdownCtx)
	m.streamCancelFn = cancel

	// Switch sidebar to todo mode so the user sees task/tool progress
	// instead of the file tree while the agent loop is running.
	if m.sidebarModel != nil {
		m.sidebarModel.SetMode(SidebarModeTodo)
		m.sidebarModel.AddTodoItem(SidebarTodoItem{
			Content:  formatAgentTodoContent(input),
			Status:   "in_progress",
			Priority: "medium",
			Source:   "agent",
		})
	}

	cmd, ch := AgentLoop(ctx, p, m.activeModel.ID, m.dispatcher, msgs, sysContent, m.activeModel.ContextLength)
	m.agentCh = ch

	if m.replModel != nil {
		m.replModel.streaming = true
	}

	return cmd
}

// formatAgentTodoContent creates a descriptive TODO item from the user's prompt.
// Instead of showing raw prompt text, it formats as an actionable task.
func formatAgentTodoContent(input string) string {
	lower := strings.ToLower(strings.TrimSpace(input))

	// Map common exploration patterns to structured descriptions
	switch {
	case strings.Contains(lower, "study") || strings.Contains(lower, "explore") || strings.Contains(lower, "understand"):
		return "Explore codebase structure and key files"
	case strings.Contains(lower, "find") || strings.Contains(lower, "search") || strings.Contains(lower, "locate"):
		return "Search for: " + truncateText(input, 50)
	case strings.Contains(lower, "fix") || strings.Contains(lower, "bug") || strings.Contains(lower, "debug"):
		return "Debug and fix: " + truncateText(input, 50)
	case strings.Contains(lower, "explain") || strings.Contains(lower, "how") || strings.Contains(lower, "what"):
		return "Analyze: " + truncateText(input, 50)
	case strings.Contains(lower, "build") || strings.Contains(lower, "create") || strings.Contains(lower, "implement"):
		return "Implement: " + truncateText(input, 50)
	case strings.Contains(lower, "test"):
		return "Run tests and verify"
	case strings.Contains(lower, "review") || strings.Contains(lower, "audit"):
		return "Review: " + truncateText(input, 50)
	default:
		return "Execute: " + truncateText(input, 60)
	}
}

// truncateText shortens text to maxLen, adding "…" if truncated.
func truncateText(s string, maxLen int) string {
	s = strings.TrimSpace(s)
	if len(s) <= maxLen {
		return s
	}
	return s[:maxLen-3] + "…"
}

// sendPlainTextChat sends a chat request without tools (original behavior).
func (m *AppState) sendPlainTextChat(p provider.LLMProvider, input string) tea.Cmd {
	var msgs []types.Message
	if m.replModel != nil {
		for _, msg := range m.replModel.Messages() {
			if !msg.SkipForLLM {
				msgs = append(msgs, msg)
			}
		}
	}

	userMsg := types.Message{
		Role:    "user",
		Content: input,
	}
	if len(msgs) > 0 && msgs[len(msgs)-1].Role == "user" {
		msgs[len(msgs)-1] = userMsg
	} else {
		msgs = append(msgs, userMsg)
	}

	// Smart truncation to prevent context overflow
	if m.activeModel != nil {
		estimator := tokens.NewEstimator(m.activeModel.ID)
		truncatedMsgs, truncated := streaming.TruncateMessagesForLLM(msgs, m.activeModel.ContextLength, estimator)
		if truncated {
			slog.Debug("truncated messages for LLM context window", "original", len(msgs), "kept", len(truncatedMsgs))
		}
		msgs = truncatedMsgs
	}

	model := m.activeModel
	req := provider.ChatRequest{
		Model:    model.ID,
		Messages: msgs,
	}

	ctx, cancel := context.WithCancel(m.shutdownCtx)
	m.streamCancelFn = cancel

	cmd, streamCh := StartStreamCmd(ctx, p, req, m.sessionID)
	if m.replModel != nil {
		m.replModel.streaming = true
		m.replModel.streamCh = streamCh
	}

	return cmd
}

// classifyAndRoute triggers an async LLM intent classification and returns
// the result as a tea.Cmd. The IntentClassifiedMsg handler in Update routes
// the input based on the classification result.
func (m *AppState) classifyAndRoute(p provider.LLMProvider, input string) tea.Cmd {
	if m.replModel != nil {
		m.replModel.lastStatus = "Classifying intent…"
	}

	modelID := m.activeModel.ID
	prompts := m.promptRegistry
	ctx := m.shutdownCtx
	timeoutSecs := 0
	if m.config != nil && m.config.Features.IntentClassifyTimeoutSecs > 0 {
		timeoutSecs = m.config.Features.IntentClassifyTimeoutSecs
	}

	return func() tea.Msg {
		result, err := workflow.ClassifyIntent(ctx, p, modelID, input, prompts, timeoutSecs)
		if err != nil || result == nil {
			return IntentClassifiedMsg{
				Input: input,
				Err:   err,
			}
		}
		return IntentClassifiedMsg{
			Result: *result,
			Input:  input,
		}
	}
}

// handleIntentClassified processes the async intent classification result and
// routes the input to the appropriate handler based on the detected intent.
func (m *AppState) handleIntentClassified(msg IntentClassifiedMsg) tea.Cmd {
	// Classification failed — fall back to existing behavior
	if msg.Err != nil {
		slog.Warn("intent classification failed, using fallback", "error", msg.Err)
		p := m.registry.ActiveProvider()
		if p == nil {
			return nil
		}
		if m.agentMode {
			return m.startAgentLoop(p, msg.Input)
		}
		return m.sendPlainTextChat(p, msg.Input)
	}

	result := msg.Result
	intentLabel := string(result.Intent)
	if m.replModel != nil {
		m.replModel.lastStatus = fmt.Sprintf("Intent: %s (%.0f%%)", intentLabel, result.Confidence*100)
	}

	// Store the intent result on the engine for downstream enrichment
	if m.workflowEngine != nil {
		if eng, ok := m.workflowEngine.(*workflow.Engine); ok {
			eng.SetIntentResult(&result)
		}
	}

	// Workflow-worthy intents (feature, bugfix, refactor) with high confidence
	// get a confirmation prompt before starting the full workflow.
	if workflow.ShouldStartWorkflow(&result) && result.Intent != types.IntentChore {
		m.pendingIntent = &result
		m.pendingIntentInput = msg.Input
		prompt := fmt.Sprintf("This looks like a **%s** request (confidence: %.0f%%). Start a structured workflow? (y/n)",
			intentLabel, result.Confidence*100)
		if m.replModel != nil {
			m.replModel.AddMessage(MakeAssistantMsg(prompt))
		}
		return nil
	}

	// Chore intents auto-start workflow in Direct mode (no confirmation needed)
	if result.Intent == types.IntentChore && result.Confidence >= 0.7 {
		m.workflowMode = types.ModeDirect
		return m.runWorkflowFromGoal(msg.Input)
	}

	// Question/explanation intents go to plain chat (no tools needed)
	if result.Intent == types.IntentQuestion || result.Intent == types.IntentExplanation {
		p := m.registry.ActiveProvider()
		if p == nil {
			return nil
		}
		return m.sendPlainTextChat(p, msg.Input)
	}

	// Exploration intents go to agent loop (tools enabled, no workflow)
	if result.Intent == types.IntentExploration {
		p := m.registry.ActiveProvider()
		if p == nil {
			return nil
		}
		return m.startAgentLoop(p, msg.Input)
	}

	// Low confidence or unknown — fall back to agent mode or plain chat
	p := m.registry.ActiveProvider()
	if p == nil {
		return nil
	}
	if m.agentMode {
		return m.startAgentLoop(p, msg.Input)
	}
	return m.sendPlainTextChat(p, msg.Input)
}
