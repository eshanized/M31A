package tui

import (
	"context"
	"fmt"
	"strings"
	"time"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/eshanized/M31A/internal/codeintel"
	m31errors "github.com/eshanized/M31A/internal/errors"
	"github.com/eshanized/M31A/internal/provider"
	"github.com/eshanized/M31A/internal/tokens"
	"github.com/eshanized/M31A/internal/tui/streaming"
	"github.com/eshanized/M31A/internal/tui/theme"
	"github.com/eshanized/M31A/internal/types"
	"github.com/eshanized/M31A/internal/workflow"
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
				ctx.CopyError = m.replModel.copyLastError
			}
			result, handled := m.cmdRegistry.Execute(input, ctx)
			if handled {
				return m.processCommandResult(result)
			}
		}

		// Unknown command
		if m.replModel != nil {
			m.replModel.AddMessage(makeAssistantMsg(
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
			m.replModel.AddMessage(makeAssistantMsg(prompt + " (y/n)"))
		}
		return nil
	}

	// Show the result message in REPL if any
	if result.Message != "" && m.replModel != nil {
		m.replModel.AddMessage(makeAssistantMsg(result.Message))
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
			m.replModel.AddMessage(makeAssistantMsg(
				"No provider configured. Run /settings to add an API key.",
			))
		}
		return nil
	}

	p := m.registry.ActiveProvider()
	if p == nil {
		if m.replModel != nil {
			m.replModel.AddMessage(makeAssistantMsg(
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
				m.replModel.AddMessage(makeAssistantMsg(
					"No model selected. Run /model to choose one.",
				))
			}
			return nil
		}
	}

	_ = t

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
		registry, err := workflow.LoadPrompts()
		if err != nil {
			if m.replModel != nil {
				m.replModel.AddMessage(makeAssistantMsg(
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
		msgs, _ = streaming.TruncateMessagesForLLM(msgs, m.activeModel.ContextLength, estimator)
	}

	ctx, cancel := context.WithCancel(m.shutdownCtx)
	m.streamCancelFn = cancel

	// Switch sidebar to todo mode so the user sees task/tool progress
	// instead of the file tree while the agent loop is running.
	if m.sidebarModel != nil {
		m.sidebarModel.SetMode(SidebarModeTodo)
	}

	cmd, ch := AgentLoop(ctx, p, m.activeModel.ID, m.dispatcher, msgs, sysContent, m.activeModel.ContextLength)
	m.agentCh = ch

	if m.replModel != nil {
		m.replModel.streaming = true
	}

	return cmd
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
		msgs, _ = streaming.TruncateMessagesForLLM(msgs, m.activeModel.ContextLength, estimator)
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
