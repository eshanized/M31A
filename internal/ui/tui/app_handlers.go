package tui

import (
	"encoding/json"
	stderrors "errors"
	"fmt"
	"strings"
	"time"

	tea "github.com/charmbracelet/bubbletea"
	m31errors "github.com/eshanized/M31A/internal/core/errors"
	"github.com/eshanized/M31A/internal/core/types"
)

// app_handlers.go — extracted message handler methods for AppState.
//
// Each handler is responsible for one logical group of message types.
// This keeps Update() as a dispatch table rather than a monolith.

// handleAgentMsg handles all Agent*Msg types from the autonomous agent loop.
func (m *AppState) handleAgentMsg(msg tea.Msg) tea.Cmd {
	var cmds []tea.Cmd

	switch msg := msg.(type) {
	case AgentStreamMsg:
		if m.replModel != nil && msg.Chunk != nil {
			if !m.replModel.streaming && m.sidebarModel != nil {
				m.sidebarModel.StartTokenBurn()
			}
			sm := StreamMsg{
				Chunk:     msg.Chunk,
				ModelID:   activeModelID(m.activeModel),
				SessionID: m.sessionID,
			}
			cs := m.replModel.handleStreamMsg(sm)
			cmds = append(cmds, cs[1:]...)
		}
		cmds = append(cmds, m.readAgentCh())

	case AgentThinkingMsg:
		if m.replModel != nil {
			m.replModel.lastStatus = fmt.Sprintf("⟳ Iteration %d — thinking…", msg.Iteration)
			m.replModel.thinking = true
			if m.replModel.thinkingStartAt.IsZero() {
				m.replModel.thinkingStartAt = time.Now()
			}
		}
		cmds = append(cmds, m.readAgentCh())

	case AgentToolStartMsg:
		if m.replModel != nil {
			tcJSON, _ := json.Marshal(types.ToolCall{
				ID:    msg.ToolCall.ID,
				Name:  msg.ToolCall.Name,
				Input: msg.ToolCall.Input,
			})
			toolMsg := types.Message{
				Role: "assistant",
				Segments: []types.MessageSegment{{
					Type:    "tool_use",
					Content: string(tcJSON),
					Visible: true,
				}},
				CreatedAt: time.Now(),
			}
			m.replModel.AddMessage(toolMsg)
			m.replModel.TrackLiveTool(msg.ToolCall.Name, len(m.replModel.Messages())-1)
		}
		if m.sidebarModel != nil {
			m.sidebarModel.AddToolCallStart(msg.ToolCall.Name, "")
		}
		cmds = append(cmds, m.readAgentCh())

	case AgentToolProgressMsg:
		if m.replModel != nil {
			elapsed := float64(msg.ElapsedMs) / 1000.0
			m.replModel.lastStatus = fmt.Sprintf("⏳ %s — %.1fs", msg.ToolCall.Name, elapsed)
		}
		cmds = append(cmds, m.readAgentCh())

	case AgentToolDoneMsg:
		if m.replModel != nil {
			m.replModel.UpdateLiveTool(msg.ToolCall.Name, msg.Err, msg.DurationMs)
		}
		if m.sidebarModel != nil {
			m.sidebarModel.CompleteToolCall(msg.ToolCall.Name, msg.Err == nil, time.Duration(msg.DurationMs)*time.Millisecond)
		}
		cmds = append(cmds, m.readAgentCh())

	case AgentIterationDoneMsg:
		if m.replModel != nil {
			m.replModel.streaming = false
			m.replModel.thinking = false
			m.replModel.thinkingStartAt = time.Time{}
		}
		cmds = append(cmds, m.readAgentCh())

	case AgentIterationMsg:
		if m.replModel != nil {
			var chips []string
			for _, tc := range msg.ToolCalls {
				inputSnippet := extractToolInputSnippet(tc)
				chips = append(chips, tc.Name+"|"+inputSnippet)
			}
			iterContent := fmt.Sprintf("iter:%d:chips:%s", msg.Iteration, strings.Join(chips, ","))
			iterMsg := types.Message{
				Role: "assistant",
				Segments: []types.MessageSegment{{
					Type:    "content",
					Content: iterContent,
					Visible: true,
				}},
				CreatedAt: time.Now(),
			}
			m.replModel.AddMessage(iterMsg)
		}
		cmds = append(cmds, m.readAgentCh())

	case AgentDoneMsg:
		if m.replModel != nil {
			m.replModel.streaming = false
			m.replModel.thinking = false
			m.replModel.thinkingStartAt = time.Time{}
			doneMsg := StreamDoneMsg{
				Message:   msg.Message,
				Usage:     msg.Usage,
				ModelID:   activeModelID(m.activeModel),
				SessionID: m.sessionID,
			}
			collapsed := m.replModel.handleStreamDoneMsg(doneMsg)
			m.checkAutoDream()
			if collapsed > 0 {
				cmds = append(cmds, m.addToastCmd(
					fmt.Sprintf("↓ %d tool output(s) collapsed — press Enter to expand", collapsed),
					"info", 3*time.Second))
			}
		}
		m.streamCancelFn = nil
		m.agentCh = nil
		if m.sidebarModel != nil && m.sidebarModel.GetMode() == SidebarModeTodo {
			m.sidebarModel.RevertToFiles()
		}
		m.saveAgentSession()

	case AgentErrorMsg:
		if stderrors.Is(msg.Err, m31errors.ErrContextExceeded) && m.autoDream != nil && m.replModel != nil {
			msgs := m.replModel.Messages()
			m.autoDream.SetMessages(msgs)
			if m.autoDream.CanConsolidate() {
				result := m.autoDream.Consolidate()
				if result.Success {
					m.replModel.SetMessages(m.autoDream.Messages())
					m.addToast(fmt.Sprintf("Context compressed: %d messages removed, ~%d tokens saved. Re-send your message to retry.", result.MessagesRemoved, result.TokensSaved), "info")
				}
			}
		}
		if m.replModel != nil {
			m.replModel.streaming = false
			m.replModel.thinking = false
			m.replModel.thinkingStartAt = time.Time{}
			errMsg := StreamErrorMsg{Err: msg.Err, ModelID: activeModelID(m.activeModel), ProviderName: m.activeProvider}
			m.replModel.handleStreamErrorMsg(errMsg)
		}
		m.streamCancelFn = nil
		m.agentCh = nil
		if m.sidebarModel != nil && m.sidebarModel.GetMode() == SidebarModeTodo {
			m.sidebarModel.RevertToFiles()
		}

	case AgentCompressedMsg:
		m.addToast(fmt.Sprintf("Auto-compressed: %d messages removed, ~%d tokens saved", msg.MessagesRemoved, msg.TokensSaved), "info")
	}

	return tea.Batch(cmds...)
}
