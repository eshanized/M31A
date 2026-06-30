package tui

import (
	"fmt"
	"log/slog"
	"time"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/eshanized/M31A/internal/tools/subagent"
	"github.com/eshanized/M31A/internal/types"
)

// app_handlers_misc.go — miscellaneous event handling extracted from Update().

// handleSubagentEvent processes SubagentEventMsg: updates the subagent model,
// surfaces events in the REPL, and cleans up worktrees.
func (m *AppState) handleSubagentEvent(msg SubagentEventMsg) []tea.Cmd {
	var cmds []tea.Cmd

	if m.subagentsModel != nil {
		m.subagentsModel.ApplyEvent(msg.Event)
		if msg.Event.Type == subagent.EventSpawned {
			m.subagentsVisible = true
		}
		if m.sidebarModel != nil {
			total, active := m.subagentsModel.GetStatus()
			m.sidebarModel.UpdateSubAgentStatus(total, active)
		}
	}

	if m.replModel != nil {
		switch msg.Event.Type {
		case subagent.EventSpawned:
			label := msg.Event.Name
			if label == "" {
				label = msg.Event.AgentID
			}
			m.replModel.AddMessage(makeAssistantMsg(
				fmt.Sprintf("**Subagent %s** spawned", label),
			))
		case subagent.EventDone:
			label := msg.Event.Name
			if label == "" {
				label = msg.Event.AgentID
			}
			body := fmt.Sprintf("**Subagent %s done** (%d tools, %d+%d tokens)",
				label, msg.Event.ToolCalls, msg.Event.InputToks, msg.Event.OutputToks)
			if msg.Event.Summary != "" {
				body += "\n\n" + msg.Event.Summary
			}
			m.replModel.AddMessage(makeAssistantMsg(body))
			if m.subagentManager != nil {
				agentID := msg.Event.AgentID
				cmds = append(cmds, func() tea.Msg {
					if err := m.subagentManager.Cleanup(m.shutdownCtx, agentID); err != nil {
						slog.Warn("subagent worktree cleanup failed", "id", agentID, "error", err)
					}
					return nil
				})
			}
		case subagent.EventError:
			label := msg.Event.Name
			if label == "" {
				label = msg.Event.AgentID
			}
			m.replModel.AddMessage(makeAssistantMsg(
				fmt.Sprintf("**Subagent %s errored:** %s", label, msg.Event.Error),
			))
			if m.subagentManager != nil {
				agentID := msg.Event.AgentID
				cmds = append(cmds, func() tea.Msg {
					if err := m.subagentManager.Cleanup(m.shutdownCtx, agentID); err != nil {
						slog.Warn("subagent worktree cleanup failed", "id", agentID, "error", err)
					}
					return nil
				})
			}
		}
	}

	if m.subagentManager != nil {
		cmds = append(cmds, subagentListenerCmd(m.shutdownCtx, m.subagentManager.Events()))
	}

	return cmds
}

// handleChatHistoryContinue processes ChatHistoryContinueMsg: truncates messages
// to the selected point and persists the truncated session.
func (m *AppState) handleChatHistoryContinue(msg ChatHistoryContinueMsg) []tea.Cmd {
	var cmds []tea.Cmd

	if m.replModel != nil && msg.MessageIndex >= 0 && msg.MessageIndex < len(m.replModel.Messages()) {
		truncated := make([]types.Message, msg.MessageIndex+1)
		copy(truncated, m.replModel.Messages()[:msg.MessageIndex+1])
		m.replModel.SetMessages(truncated)
		if m.sessionManager != nil && m.sessionID != "" {
			sess, err := m.sessionManager.LoadSession(m.sessionID)
			if err == nil {
				sess.Messages = truncated
				sess.MessageCount = len(truncated)
				_ = m.sessionManager.SaveSession(sess)
			}
		}
		cmds = append(cmds, m.addToastCmd(
			fmt.Sprintf("Continued from message %d — %d messages remaining", msg.MessageIndex+1, len(truncated)),
			"success", 3*time.Second))
		cmds = append(cmds, m.navigateToScreen(ScreenREPL))
	}

	return cmds
}

// handlePhaseModelPicked processes PhaseModelPickedMsg: sets planning/coding
// model IDs and resumes the workflow.
func (m *AppState) handlePhaseModelPicked(msg PhaseModelPickedMsg) []tea.Cmd {
	m.planningModelID = msg.PlanningModelID
	m.codingModelID = msg.CodingModelID
	if m.workflowEngine != nil && msg.PlanningModelID != "" {
		m.workflowEngine.SetPhaseModel(types.PhaseDiscuss, msg.PlanningModelID)
		m.workflowEngine.SetPhaseModel(types.PhasePlan, msg.PlanningModelID)
		m.workflowEngine.SetPhaseModel(types.PhaseVerify, msg.PlanningModelID)
	}
	if m.workflowEngine != nil && msg.CodingModelID != "" {
		m.workflowEngine.SetPhaseModel(types.PhaseExecute, msg.CodingModelID)
		m.workflowEngine.SetPhaseModel(types.PhaseShip, msg.CodingModelID)
	}
	m.screen = ScreenREPL
	return []tea.Cmd{m.runWorkflowFromGoal(m.workflowGoal)}
}

// handleToast processes ToastMsg: adds a toast and schedules its expiry.
func (m *AppState) handleToast(msg ToastMsg) []tea.Cmd {
	id := m.addToast(msg.Text, msg.Type)
	duration := msg.Duration
	if duration <= 0 {
		duration = 3 * time.Second
	}
	for i := len(m.toasts) - 1; i >= 0; i-- {
		if m.toasts[i].ID == id {
			m.toasts[i].Duration = duration
			break
		}
	}
	return []tea.Cmd{tea.Tick(duration, func(time.Time) tea.Msg {
		return ToastExpiryMsg{ToastID: id}
	})}
}

// handleSidebarEvents processes SidebarRefreshTickMsg, SidebarRefreshMsg,
// SidebarRevertMsg, and SidebarTodoUpdateMsg.
func (m *AppState) handleSidebarRefreshTick(msg SidebarRefreshTickMsg) []tea.Cmd {
	var cmds []tea.Cmd
	if m.sidebarModel != nil {
		newSidebar, cmd := m.sidebarModel.Update(msg)
		m.sidebarModel = newSidebar
		cmds = append(cmds, cmd)
	}
	if m.fileWatcher != nil {
		cmds = append(cmds, m.drainFileWatcherCmd())
	}
	return cmds
}

func (m *AppState) handleSidebarRefresh(msg SidebarRefreshMsg) []tea.Cmd {
	var cmds []tea.Cmd
	if m.sidebarModel != nil {
		newSidebar, cmd := m.sidebarModel.Update(msg)
		m.sidebarModel = newSidebar
		cmds = append(cmds, cmd)
	}
	if m.replModel != nil && msg.Branch != "" {
		m.replModel.sidebarBranch = msg.Branch
	}
	if m.replModel != nil {
		m.replModel.SetChangedFiles(len(msg.Files))
	}
	return cmds
}

func (m *AppState) handleSidebarTodoUpdate(msg SidebarTodoUpdateMsg) []tea.Cmd {
	if m.sidebarModel != nil {
		if m.sidebarModel.GetMode() == SidebarModeFiles {
			m.sidebarModel.SetMode(SidebarModeTodo)
		}
		for _, item := range msg.Items {
			m.sidebarModel.AddTodoItem(SidebarTodoItem{
				Content:  item.Content,
				Status:   item.Status,
				Priority: item.Priority,
				Source:   "llm",
			})
		}
	}
	return nil
}

// checkContextWarnings checks context usage and emits toast warnings at 70% and 85%.
// Called after updateSidebarUsage() when token data is fresh. Returns cmds for toasts.
func (m *AppState) checkContextWarnings() []tea.Cmd {
	if m.sidebarModel == nil {
		return nil
	}
	total := m.sidebarModel.totalTokens
	ctxLen := m.sidebarModel.contextLen
	if ctxLen <= 0 || total <= 0 {
		return nil
	}
	pct := float64(total) / float64(ctxLen)

	var cmds []tea.Cmd

	// Auto-insert system message to suggest /compress when context reaches 70%
	if pct >= 0.70 && !m.ctxWarned70 {
		// Mark warning as shown but don't block further compaction suggestions at 85%
		m.ctxWarned70 = true
		// Format the warning message similar to what the toast would show
		warningMsg := fmt.Sprintf("Context at %d%%. Consider /compress to prevent overflow.", int(pct*100))
		// Parse and insert as a system message in the REPL state
		if m.replModel != nil {
			// Clean existing content but preserve tool calls for workflow state
			// This is a minimal intervention that just nudges the user
			m.replModel.InsertSystemPromptHint("⚠️ " + warningMsg)
		}
	}

	if pct >= 0.85 && !m.ctxWarned85 {
		m.ctxWarned85 = true
		cmds = append(cmds, m.addToastCmd(
			fmt.Sprintf("Context at %d%%. Compaction recommended — /compress to free space.", int(pct*100)),
			"warning", 8*time.Second))
	} else if pct >= 0.70 && !m.ctxWarned70 {
		m.ctxWarned70 = true
		cmds = append(cmds, m.addToastCmd(
			fmt.Sprintf("Context at %d%%. Consider /compress to prevent overflow.", int(pct*100)),
			"info", 6*time.Second))
	}
	return cmds
}
