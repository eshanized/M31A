package tui

import (
	"context"
	"fmt"
	"strings"
	"time"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/eshanized/M31A/internal/provider"
	"github.com/eshanized/M31A/internal/tui/theme"
	"github.com/eshanized/M31A/internal/types"
)

// handleSlashCommand routes slash commands to the command registry.
// It also handles special chat messages (non-slash input).
func (m *AppState) handleSlashCommand(input string) tea.Cmd {
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
				Ctx:            m.shutdownCtx,
				Registry:       m.registry,
				SessionManager: m.sessionManager,
				SessionID:      m.sessionID,
				Config:         m.config,
				ConfigPath:     m.configPath,
				Dispatcher:     m.dispatcher,
				Git:            m.git,
				Ledger:         m.ledger,
				Rollback:       m.rollback,
				AutoDream:      m.autoDream,
				WorkflowEngine: m.workflowEngine,
				CmdRegistry:    m.cmdRegistry,
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
	if strings.Contains(input, "--- Attached file context ---") {
		count := strings.Count(input, "**File: ")
		if count > 0 {
			m.toasts = append(m.toasts, Toast{
				Text:      fmt.Sprintf("Attached %d file(s) via @-mention", count),
				Type:      "info",
				CreatedAt: time.Now(),
			})
		}
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
		if m.replModel != nil {
			m.replModel.AddMessage(makeAssistantMsg(
				"No model selected. Run /model to choose one.",
			))
		}
		return nil
	}

	// Build messages (chat history + new user turn)
	var msgs []types.Message
	if m.replModel != nil {
		for _, msg := range m.replModel.Messages() {
			if !msg.SkipForLLM {
				msgs = append(msgs, msg)
			}
		}
	}
	// Append new user message
	msgs = append(msgs, types.Message{
		Role:    "user",
		Content: input,
	})

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

	_ = t // used for error rendering if needed

	return cmd
}
