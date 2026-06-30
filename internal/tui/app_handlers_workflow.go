package tui

import (
	"fmt"
	"time"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/eshanized/M31A/internal/types"
	"github.com/eshanized/M31A/internal/workflow"
)

// app_handlers_workflow.go — workflow event handling extracted from Update().

// handleWorkflowTaskStart processes workflow.TaskStartMsg: sets the current task
// in the execute model and updates the sidebar todo list.
func (m *AppState) handleWorkflowTaskStart(msg workflow.TaskStartMsg) []tea.Cmd {
	var cmds []tea.Cmd

	if m.executeModel != nil {
		found := false
		for i, t := range m.executeModel.tasks {
			if t.ID == msg.Task.ID {
				m.executeModel.SetCurrentTask(i)
				m.executeModel.UpdateTaskStatus(msg.Task.ID, types.StatusRunning)
				found = true
				break
			}
		}
		if !found {
			m.executeModel.tasks = append(m.executeModel.tasks, msg.Task)
			m.executeModel.SetCurrentTask(len(m.executeModel.tasks) - 1)
			m.executeModel.UpdateTaskStatus(msg.Task.ID, types.StatusRunning)
		}
	}

	if m.sidebarModel != nil {
		if m.sidebarModel.GetMode() == SidebarModeFiles {
			m.sidebarModel.SetMode(SidebarModeTodo)
		}
		if m.executeModel != nil && m.sidebarModel.taskProgress.Total == 0 {
			m.sidebarModel.InitTaskProgress(len(m.executeModel.tasks))
		}
		m.sidebarModel.AddTodoItem(SidebarTodoItem{
			Content:  fmt.Sprintf("[Task %d] %s", msg.Task.ID, msg.Task.Description),
			Status:   "in_progress",
			Priority: "high",
			Source:   "task",
			TaskID:   msg.Task.ID,
		})
	}

	cmds = append(cmds, m.drainAdaptiveCmd())
	return cmds
}

// handleWorkflowTaskUpdate processes workflow.TaskUpdateMsg: updates task status
// in the execute model and the sidebar todo list.
func (m *AppState) handleWorkflowTaskUpdate(msg workflow.TaskUpdateMsg) []tea.Cmd {
	var cmds []tea.Cmd

	if m.executeModel != nil {
		var status types.TaskStatus
		switch msg.Status {
		case "done":
			status = types.StatusDone
		case "failed":
			status = types.StatusFailed
		default:
			status = types.StatusRunning
		}
		m.executeModel.UpdateTaskStatus(msg.Task.ID, status)
	}

	if m.sidebarModel != nil {
		todoStatus := msg.Status
		if todoStatus == "done" {
			todoStatus = "completed"
		}
		m.sidebarModel.AddTodoItem(SidebarTodoItem{
			Content:  fmt.Sprintf("[Task %d] %s", msg.Task.ID, msg.Task.Description),
			Status:   todoStatus,
			Priority: "high",
			Source:   "task",
			TaskID:   msg.Task.ID,
		})
		m.sidebarModel.UpdateTaskProgress(msg.Task)
		if m.sidebarModel.IsAllTasksDone() {
			cmds = append(cmds, func() tea.Msg {
				return SidebarRevertMsg{}
			})
		}
	}

	cmds = append(cmds, m.drainAdaptiveCmd())
	return cmds
}

// handleWorkflowToolStart processes workflow.ToolStartMsg: appends to the
// execute model's live output and updates the sidebar tool timeline.
func (m *AppState) handleWorkflowToolStart(msg workflow.ToolStartMsg) []tea.Cmd {
	if m.executeModel != nil {
		detail := msg.Description
		if detail == "" {
			detail = msg.ToolName
		}
		m.executeModel.AppendLiveOutput([]string{
			fmt.Sprintf("→ %s: %s", msg.ToolName, detail),
		})
	}
	if m.sidebarModel != nil {
		m.sidebarModel.AddToolCallStart(msg.ToolName, msg.Description)
	}
	return []tea.Cmd{m.drainAdaptiveCmd()}
}

// handleWorkflowToolComplete processes workflow.ToolCompleteMsg: appends to the
// execute model's live output and updates the sidebar tool timeline.
func (m *AppState) handleWorkflowToolComplete(msg workflow.ToolCompleteMsg) []tea.Cmd {
	if m.executeModel != nil {
		status := "ok"
		if !msg.Success {
			status = "failed"
		}
		m.executeModel.AppendLiveOutput([]string{
			fmt.Sprintf("  %s %s (%dms)", status, msg.ToolName, msg.DurationMs),
		})
	}
	if m.sidebarModel != nil {
		m.sidebarModel.CompleteToolCall(msg.ToolName, msg.Success, time.Duration(msg.DurationMs)*time.Millisecond)
		if msg.Success && msg.FilePath != "" {
			m.sidebarModel.MarkFileChanged(msg.FilePath)
		}
	}
	return []tea.Cmd{m.drainAdaptiveCmd()}
}

// handleWorkflowSelfHealStart processes workflow.SelfHealStartMsg.
func (m *AppState) handleWorkflowSelfHealStart(msg workflow.SelfHealStartMsg) []tea.Cmd {
	if m.executeModel != nil {
		m.executeModel.AppendLiveOutput([]string{
			fmt.Sprintf("  [warn] Self-heal attempt %d/%d for task %d", msg.Attempt, msg.Max, msg.TaskID),
		})
	}
	if m.verifyModel != nil {
		m.verifyModel.StartHealing(msg.TaskID, msg.Attempt)
	}
	return []tea.Cmd{m.drainAdaptiveCmd()}
}

// handleWorkflowSelfHealComplete processes workflow.SelfHealCompleteMsg.
func (m *AppState) handleWorkflowSelfHealComplete(msg workflow.SelfHealCompleteMsg) []tea.Cmd {
	if m.executeModel != nil {
		status := "ok"
		if !msg.Success {
			status = "failed"
		}
		m.executeModel.AppendLiveOutput([]string{
			fmt.Sprintf("  Self-heal %s (attempt %d/%d)", status, msg.Attempt, msg.Max),
		})
	}
	if m.verifyModel != nil {
		m.verifyModel.StopHealing()
	}
	return []tea.Cmd{m.drainAdaptiveCmd()}
}

// handlePhaseTransitionStart processes workflow.PhaseTransitionStartMsg.
func (m *AppState) handlePhaseTransitionStart(msg workflow.PhaseTransitionStartMsg) []tea.Cmd {
	if m.sidebarModel != nil {
		m.sidebarModel.SetCurrentPhase(msg.To)
	}
	return []tea.Cmd{m.drainAdaptiveCmd()}
}

// handlePhaseTransitionComplete processes workflow.PhaseTransitionCompleteMsg.
func (m *AppState) handlePhaseTransitionComplete(msg workflow.PhaseTransitionCompleteMsg) []tea.Cmd {
	if m.sidebarModel != nil && msg.To != "" {
		m.sidebarModel.MarkPhaseCompleted(msg.To)
	}
	return []tea.Cmd{m.drainAdaptiveCmd()}
}

// handleBisectStart processes BisectStartMsg: loads git log and sets up
// the bisect model's commit range.
func (m *AppState) handleBisectStart(msg BisectStartMsg) []tea.Cmd {
	if m.bisectModel != nil && m.git != nil {
		commits, err := m.git.Log(0)
		if err == nil && len(commits) > 0 {
			var bisectCommits []bisectCommit
			goodIdx := -1
			badIdx := -1
			for i, c := range commits {
				hash := c.Hash
				if len(hash) > 7 {
					hash = hash[:7]
				}
				status := "pending"
				if msg.GoodCommit != "" && (c.Hash == msg.GoodCommit || c.ShortHash == msg.GoodCommit || hash == msg.GoodCommit) {
					status = "good"
					goodIdx = i
				}
				if msg.BadCommit != "" && (c.Hash == msg.BadCommit || c.ShortHash == msg.BadCommit || hash == msg.BadCommit) {
					status = "bad"
					badIdx = i
				}
				bisectCommits = append(bisectCommits, bisectCommit{
					Hash:    c.Hash,
					Message: c.Message,
					Status:  status,
				})
			}
			if goodIdx >= 0 && badIdx >= 0 && goodIdx != badIdx {
				start, end := goodIdx, badIdx
				if start > end {
					start, end = end, start
				}
				bisectCommits = bisectCommits[start : end+1]
			}
			if len(bisectCommits) > 0 {
				m.bisectModel.SetCommits(bisectCommits)
			}
		}
	}
	return nil
}
