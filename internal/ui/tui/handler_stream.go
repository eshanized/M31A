package tui

import (
	stderrors "errors"
	"fmt"
	"time"

	tea "github.com/charmbracelet/bubbletea"
	m31errors "github.com/eshanized/M31A/internal/core/errors"
	"github.com/eshanized/M31A/internal/ui/tui/streaming"
)

// handler_stream.go — stream message handling extracted from Update().

// handleStreamMsg processes streaming chunks from the LLM.
func handleStreamMsg(m *AppState, msg streaming.StreamMsg) (tea.Model, tea.Cmd) {
	if m.replModel != nil {
		// Start token burn tracking on the first chunk of a new response.
		if !m.replModel.streaming && m.sidebarModel != nil {
			m.sidebarModel.StartTokenBurn()
		}
		cs := m.replModel.handleStreamMsg(msg)
		return m, tea.Batch(cs...)
	}
	return m, nil
}

// handleStreamDoneMsg processes a completed stream response.
func handleStreamDoneMsg(m *AppState, msg streaming.StreamDoneMsg) (tea.Model, tea.Cmd) {
	var cmds []tea.Cmd
	if m.replModel != nil {
		collapsed := m.replModel.handleStreamDoneMsg(msg)
		m.checkAutoDream()
		// Update sidebar with token usage
		m.updateSidebarUsage()
		// Wave 2A: proactive context warnings
		cmds = append(cmds, m.checkContextWarnings()...)
		if collapsed > 0 {
			cmds = append(cmds, m.addToastCmd(
				fmt.Sprintf("↓ %d tool output(s) collapsed — press Enter to expand", collapsed),
				"info", 3*time.Second))
		}
	}
	m.streamCancelFn = nil
	return m, tea.Batch(cmds...)
}

// handleStreamErrorMsg processes a stream error.
func handleStreamErrorMsg(m *AppState, msg streaming.StreamErrorMsg) (tea.Model, tea.Cmd) {
	var cmds []tea.Cmd
	if m.replModel != nil {
		m.replModel.handleStreamErrorMsg(msg)
	}
	m.streamCancelFn = nil
	// Auto-fallback on rate limit or provider unreachable
	if m.config != nil && m.config.Provider.AutoFallback && m.registry != nil {
		if stderrors.Is(msg.Err, m31errors.ErrRateLimited) || stderrors.Is(msg.Err, m31errors.ErrProviderUnreachable) {
			cmds = append(cmds, m.attemptAutoFallback(msg.Err))
		}
	}
	return m, tea.Batch(cmds...)
}
