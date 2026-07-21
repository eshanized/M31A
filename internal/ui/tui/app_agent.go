package tui

// Agent communication, provider fallback, and session persistence for AppState.

import (
	"fmt"
	"log/slog"
	"strings"
	"time"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/eshanized/M31A/internal/core/types"
	"github.com/eshanized/M31A/internal/integrations/provider"
)

// attemptAutoFallback tries to switch to a fallback provider when the active one fails.
func (m *AppState) attemptAutoFallback(origErr error) tea.Cmd {
	if origErr != nil {
		slog.Warn("attempting auto-fallback due to error", "error", origErr)
	}
	if m.registry == nil || m.activeProvider == "" {
		return nil
	}

	// Extract Retry-After header from rate-limit errors (embedded by provider clients).
	retryAfter := ""
	if origErr != nil {
		errMsg := origErr.Error()
		const prefix = "(retry-after: "
		if idx := strings.Index(errMsg, prefix); idx != -1 {
			start := idx + len(prefix)
			if end := strings.Index(errMsg[start:], ")"); end != -1 {
				retryAfter = errMsg[start : start+end]
			}
		}
	}

	result := provider.FindFallbackWithRetryAfter(m.registry, m.activeProvider, retryAfter, m.config.Provider.FallbackPriority, m.config.Provider.HealthCheckTimeoutSecs)
	if result.Err != nil {
		slog.Warn("auto-fallback failed: no healthy fallback provider", "error", result.Err)
		return m.addToastCmd("Auto-fallback failed: no healthy provider available", "error", 5*time.Second)
	}
	if result.Event == nil {
		return nil
	}

	oldProvider := m.activeProvider
	m.activeProvider = result.Event.To
	m.addToast(fmt.Sprintf("Switched to %s (was %s: %s)", result.Event.To, oldProvider, result.Event.Reason), "warning")

	// Sync the new provider to replModel so the next manual chat uses the
	// fallback provider instead of the old (failed) one.
	if m.replModel != nil {
		m.replModel.activeProvider = result.Event.To
	}

	// Update workflow engine if active
	if m.workflowEngine != nil {
		p := m.registry.ActiveProvider()
		if p != nil && m.activeModel != nil {
			m.workflowEngine.SetModel(m.activeModel.ID, p)
		}
	}

	// Emit FallbackEventMsg so the notification system tracks auto-fallback events
	return func() tea.Msg {
		return FallbackEventMsg{
			From:   oldProvider,
			To:     result.Event.To,
			Reason: result.Event.Reason,
		}
	}
}

// reRegisterProvidersFromConfig re-registers OpenRouter and Zen with the current
// API keys from m.config. Called after both SettingsSavedMsg and ConfigSavedMsg.
func (m *AppState) reRegisterProvidersFromConfig() {
	if m.registry == nil || m.config == nil {
		return
	}
	if m.config.Provider.OpenRouter.APIKey != "" {
		if err := RegisterProvider(m.registry, m.config, types.ProviderOpenRouter, m.config.Provider.OpenRouter.APIKey, m.version); err != nil {
			slog.Warn("failed to re-register OpenRouter after config save", "error", err)
		}
	}
	if m.config.Provider.Zen.APIKey != "" {
		if err := RegisterProvider(m.registry, m.config, types.ProviderZen, m.config.Provider.Zen.APIKey, m.version); err != nil {
			slog.Warn("failed to re-register Zen after config save", "error", err)
		}
	}
	if m.config.Provider.Nvidia.APIKey != "" {
		if err := RegisterProvider(m.registry, m.config, types.ProviderNvidia, m.config.Provider.Nvidia.APIKey, m.version); err != nil {
			slog.Warn("failed to re-register NVIDIA after config save", "error", err)
		}
	}
}

// readAgentCh returns a tea.Cmd that reads the next message from the agent
// loop channel. Used to continue the Bubble Tea cmd chain for agent loop events.
func (m *AppState) readAgentCh() tea.Cmd {
	if m.agentCh == nil {
		return nil
	}
	return func() tea.Msg {
		msg, ok := <-m.agentCh
		if !ok {
			return nil
		}
		return msg
	}
}

// saveAgentSession persists the current REPL messages to the session file
// after an agent loop completes. This ensures the agent's conversation
// (including tool calls and results) survives session resume.
func (m *AppState) saveAgentSession() {
	if m.sessionManager == nil || m.sessionID == "" || m.replModel == nil {
		return
	}
	sess, err := m.sessionManager.LoadSession(m.sessionID)
	if err != nil || sess == nil {
		slog.Warn("saveAgentSession: failed to load session", "error", err)
		return
	}
	sess.Messages = m.replModel.Messages()
	sess.MessageCount = len(sess.Messages)
	if err := m.sessionManager.SaveSession(sess); err != nil {
		slog.Warn("saveAgentSession: failed to save session", "error", err)
	}
}
