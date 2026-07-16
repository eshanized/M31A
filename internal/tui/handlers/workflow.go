package handlers

import (
	"fmt"
	"time"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/eshanized/M31A/internal/workflow"
	"github.com/eshanized/M31A/internal/types"
)

// Workflow handlers - exported for use by AppState.Update

// HandleWorkflowTaskStart processes workflow.TaskStartMsg: sets the current task
// in the execute model and updates the sidebar todo list.
func HandleWorkflowTaskStart(m *AppState, msg workflow.TaskStartMsg) []tea.Cmd {
	var cmds []tea.Cmd

	if m.ExecuteModel != nil {
		found := false
		for i, t := range m.ExecuteModel.Tasks {
			if t.ID == msg.Task.ID {
				m.ExecuteModel.SetCurrentTask(i)
				m.ExecuteModel.UpdateTaskStatus(msg.Task.ID, types.StatusRunning)
				found = true
				break
			}
		}
		if !found {
			m.ExecuteModel.Tasks = append(m.ExecuteModel.Tasks, msg.Task)
			m.ExecuteModel.SetCurrentTask(len(m.ExecuteModel.Tasks) - 1)
			m.ExecuteModel.UpdateTaskStatus(msg.Task.ID, types.StatusRunning)
		}
	}

	if m.SidebarModel != nil {
		if m.SidebarModel.GetMode() == SidebarModeFiles {
			m.SidebarModel.SetMode(SidebarModeTodo)
		}
		if m.ExecuteModel != nil && m.SidebarModel.TaskProgress.Total == 0 {
			m.SidebarModel.InitTaskProgress(len(m.ExecuteModel.Tasks))
		}
		m.SidebarModel.AddTodoItem(SidebarTodoItem{
			Content:  fmt.Sprintf("[Task %d] %s", msg.Task.ID, msg.Task.Description),
			Status:   "in_progress",
			Priority: "high",
			Source:   "task",
			TaskID:   msg.Task.ID,
		})
	}

	cmds = append(cmds, DrainAdaptiveCmd(m))
	return cmds
}

// HandleWorkflowTaskUpdate processes workflow.TaskUpdateMsg: updates task status
// in the execute model and the sidebar todo list.
func HandleWorkflowTaskUpdate(m *AppState, msg workflow.TaskUpdateMsg) []tea.Cmd {
	var cmds []tea.Cmd

	if m.ExecuteModel != nil {
		var status types.TaskStatus
		switch msg.Status {
		case "done":
			status = types.StatusDone
		case "failed":
			status = types.StatusFailed
		default:
			status = types.StatusRunning
		}
		m.ExecuteModel.UpdateTaskStatus(msg.Task.ID, status)
	}

	if m.SidebarModel != nil {
		todoStatus := msg.Status
		if todoStatus == "done" {
			todoStatus = "completed"
		}
		m.SidebarModel.AddTodoItem(SidebarTodoItem{
			Content:  fmt.Sprintf("[Task %d] %s", msg.Task.ID, msg.Task.Description),
			Status:   todoStatus,
			Priority: "high",
			Source:   "task",
			TaskID:   msg.Task.ID,
		})
		m.SidebarModel.UpdateTaskProgress(msg.Task)
		if m.SidebarModel.IsAllTasksDone() {
			cmds = append(cmds, func() tea.Msg {
				return SidebarRevertMsg{}
			})
		}
	}

	cmds = append(cmds, DrainAdaptiveCmd(m))
	return cmds
}

// HandleWorkflowToolStart processes workflow.ToolStartMsg: appends to the
// execute model's live output and updates the sidebar tool timeline.
func HandleWorkflowToolStart(m *AppState, msg workflow.ToolStartMsg) []tea.Cmd {
	if m.ExecuteModel != nil {
		detail := msg.Description
		if detail == "" {
			detail = msg.ToolName
		}
		m.ExecuteModel.AppendLiveOutput([]string{
			fmt.Sprintf("→ %s: %s", msg.ToolName, detail),
		})
	}
	if m.SidebarModel != nil {
		m.SidebarModel.AddToolCallStart(msg.ToolName, msg.Description)
	}
	return []tea.Cmd{DrainAdaptiveCmd(m)}
}

// HandleWorkflowToolComplete processes workflow.ToolCompleteMsg: appends to the
// execute model's live output and updates the sidebar tool timeline.
func HandleWorkflowToolComplete(m *AppState, msg workflow.ToolCompleteMsg) []tea.Cmd {
	if m.ExecuteModel != nil {
		status := "ok"
		if !msg.Success {
			status = "failed"
		}
		m.ExecuteModel.AppendLiveOutput([]string{
			fmt.Sprintf("  %s %s (%dms)", status, msg.ToolName, msg.DurationMs),
		})
	}
	if m.SidebarModel != nil {
		m.SidebarModel.CompleteToolCall(msg.ToolName, msg.Success, time.Duration(msg.DurationMs)*time.Millisecond)
		if msg.Success && msg.FilePath != "" {
			m.SidebarModel.MarkFileChanged(msg.FilePath)
		}
	}
	return []tea.Cmd{DrainAdaptiveCmd(m)}
}

// HandleWorkflowSelfHealStart processes workflow.SelfHealStartMsg.
func HandleWorkflowSelfHealStart(m *AppState, msg workflow.SelfHealStartMsg) []tea.Cmd {
	if m.ExecuteModel != nil {
		m.ExecuteModel.AppendLiveOutput([]string{
			fmt.Sprintf("  [warn] Self-heal attempt %d/%d for task %d", msg.Attempt, msg.Max, msg.TaskID),
		})
	}
	if m.VerifyModel != nil {
		m.VerifyModel.StartHealing(msg.TaskID, msg.Attempt)
	}
	return []tea.Cmd{DrainAdaptiveCmd(m)}
}

// HandleWorkflowSelfHealComplete processes workflow.SelfHealCompleteMsg.
func HandleWorkflowSelfHealComplete(m *AppState, msg workflow.SelfHealCompleteMsg) []tea.Cmd {
	if m.ExecuteModel != nil {
		status := "ok"
		if !msg.Success {
			status = "failed"
		}
		m.ExecuteModel.AppendLiveOutput([]string{
			fmt.Sprintf("  Self-heal %s (attempt %d/%d)", status, msg.Attempt, msg.Max),
		})
	}
	if m.VerifyModel != nil {
		m.VerifyModel.StopHealing()
	}
	return []tea.Cmd{DrainAdaptiveCmd(m)}
}

// HandlePhaseTransitionStart processes workflow.PhaseTransitionStartMsg.
func HandlePhaseTransitionStart(m *AppState, msg workflow.PhaseTransitionStartMsg) []tea.Cmd {
	if m.SidebarModel != nil {
		m.SidebarModel.SetCurrentPhase(msg.To)
	}
	return []tea.Cmd{DrainAdaptiveCmd(m)}
}

// HandlePhaseTransitionComplete processes workflow.PhaseTransitionCompleteMsg.
func HandlePhaseTransitionComplete(m *AppState, msg workflow.PhaseTransitionCompleteMsg) []tea.Cmd {
	if m.SidebarModel != nil && msg.To != "" {
		m.SidebarModel.MarkPhaseCompleted(msg.To)
	}
	return []tea.Cmd{DrainAdaptiveCmd(m)}
}

// HandleBisectStart processes BisectStartMsg: loads git log and sets up
// the bisect model's commit range.
func HandleBisectStart(m *AppState, msg BisectStartMsg) []tea.Cmd {
	if m.BisectModel != nil && m.Git != nil {
		commits, err := m.Git.Log(0)
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
				m.BisectModel.SetCommits(bisectCommits)
			}
		}
	}
	return nil
}

// ─── W7: Unwired workflow event handlers ──────────────────────────────────────

// HandleInitAnalysis processes InitAnalysisMsg: project analysis complete.
func HandleInitAnalysis(m *AppState, msg workflow.InitAnalysisMsg) []tea.Cmd {
	if m.SidebarModel != nil {
		m.SidebarModel.AddTodoItem(SidebarTodoItem{
			Content:  fmt.Sprintf("Analyzed: %s %s (%d files, health %d%%)", msg.Language, msg.ProjectType, msg.FileCount, msg.HealthScore),
			Status:   "completed",
			Priority: "low",
			Source:   "init",
		})
	}
	return []tea.Cmd{DrainAdaptiveCmd(m)}
}

// HandleInitPreflight processes InitPreflightMsg: environment preflight complete.
func HandleInitPreflight(m *AppState, msg workflow.InitPreflightMsg) []tea.Cmd {
	if m.SidebarModel != nil {
		status := "passed"
		if !msg.Passed {
			status = fmt.Sprintf("failed (%d issues)", len(msg.Issues))
		}
		m.SidebarModel.AddTodoItem(SidebarTodoItem{
			Content:  fmt.Sprintf("Preflight: %s", status),
			Status:   "completed",
			Priority: "low",
			Source:   "init",
		})
	}
	return []tea.Cmd{DrainAdaptiveCmd(m)}
}

// HandleResearchProgress processes ResearchProgressMsg: research sub-step update.
func HandleResearchProgress(m *AppState, msg workflow.ResearchProgressMsg) []tea.Cmd {
	if m.SidebarModel != nil {
		m.SidebarModel.AddTodoItem(SidebarTodoItem{
			Content:  fmt.Sprintf("Research: %s", msg.Message),
			Status:   "in_progress",
			Priority: "low",
			Source:   "research",
		})
	}
	return []tea.Cmd{DrainAdaptiveCmd(m)}
}

// HandlePlanCheck processes PlanCheckMsg: plan quality check result.
func HandlePlanCheck(m *AppState, msg workflow.PlanCheckMsg) []tea.Cmd {
	if m.SidebarModel != nil {
		status := "passed"
		if !msg.Passed {
			status = fmt.Sprintf("failed (%d issues, %d blockers)", msg.IssueCount, msg.Blockers)
		}
		m.SidebarModel.AddTodoItem(SidebarTodoItem{
			Content:  fmt.Sprintf("Plan check: %s", status),
			Status:   "completed",
			Priority: "medium",
			Source:   "plan",
		})
	}
	return []tea.Cmd{DrainAdaptiveCmd(m)}
}

// HandlePlanRevision processes PlanRevisionMsg: plan revision iteration.
func HandlePlanRevision(m *AppState, msg workflow.PlanRevisionMsg) []tea.Cmd {
	if m.SidebarModel != nil {
		m.SidebarModel.AddTodoItem(SidebarTodoItem{
			Content:  fmt.Sprintf("Plan revision %d/%d (%d issues remaining)", msg.Iteration, msg.MaxIterations, msg.IssuesRemaining),
			Status:   "in_progress",
			Priority: "medium",
			Source:   "plan",
		})
	}
	return []tea.Cmd{DrainAdaptiveCmd(m)}
}

// HandlePlanChunkProgress processes PlanChunkProgressMsg: chunked plan generation.
func HandlePlanChunkProgress(m *AppState, msg workflow.PlanChunkProgressMsg) []tea.Cmd {
	if m.SidebarModel != nil {
		m.SidebarModel.AddTodoItem(SidebarTodoItem{
			Content:  fmt.Sprintf("Plan wave %d/%d (%d tasks)", msg.Wave, msg.TotalWaves, msg.TasksInWave),
			Status:   "in_progress",
			Priority: "medium",
			Source:   "plan",
		})
	}
	return []tea.Cmd{DrainAdaptiveCmd(m)}
}

// HandleDiscussQuality processes DiscussQualityMsg: question quality check.
func HandleDiscussQuality(m *AppState, msg workflow.DiscussQualityMsg) []tea.Cmd {
	if m.SidebarModel != nil {
		status := "passed"
		if !msg.Passed {
			status = fmt.Sprintf("failed (%d warnings)", msg.Warnings)
		}
		m.SidebarModel.AddTodoItem(SidebarTodoItem{
			Content:  fmt.Sprintf("Question quality: %s", status),
			Status:   "completed",
			Priority: "low",
			Source:   "discuss",
		})
	}
	return []tea.Cmd{DrainAdaptiveCmd(m)}
}

// HandleDiscussCompleteness processes DiscussCompletenessMsg: answer completeness.
func HandleDiscussCompleteness(m *AppState, msg workflow.DiscussCompletenessMsg) []tea.Cmd {
	if m.SidebarModel != nil {
		m.SidebarModel.AddTodoItem(SidebarTodoItem{
			Content:  fmt.Sprintf("Answer completeness: %d%%", msg.Score),
			Status:   "completed",
			Priority: "low",
			Source:   "discuss",
		})
	}
	return []tea.Cmd{DrainAdaptiveCmd(m)}
}

// HandleExecutePreflight processes ExecutePreflightMsg: pre-execution validation.
func HandleExecutePreflight(m *AppState, msg workflow.ExecutePreflightMsg) []tea.Cmd {
	if m.SidebarModel != nil {
		status := "passed"
		if !msg.Passed {
			status = fmt.Sprintf("failed (%d issues)", len(msg.Issues))
		}
		m.SidebarModel.AddTodoItem(SidebarTodoItem{
			Content:  fmt.Sprintf("Execute preflight: %s", status),
			Status:   "completed",
			Priority: "medium",
			Source:   "execute",
		})
	}
	return []tea.Cmd{DrainAdaptiveCmd(m)}
}

// HandleExecuteQualityGate processes ExecuteQualityGateMsg: per-task quality check.
func HandleExecuteQualityGate(m *AppState, msg workflow.ExecuteQualityGateMsg) []tea.Cmd {
	if m.ExecuteModel != nil {
		status := "passed"
		if !msg.Passed {
			status = fmt.Sprintf("failed (%d/%d checked, %d failed)", msg.Checked, msg.Checked+msg.Failed, msg.Failed)
		}
		m.ExecuteModel.AppendLiveOutput([]string{
			fmt.Sprintf("  [quality] Task %d: %s", msg.TaskID, status),
		})
	}
	return []tea.Cmd{DrainAdaptiveCmd(m)}
}

// HandleExecuteLoopDetect processes ExecuteLoopDetectMsg: tool call loop detection.
func HandleExecuteLoopDetect(m *AppState, msg workflow.ExecuteLoopDetectMsg) []tea.Cmd {
	if m.ExecuteModel != nil {
		m.ExecuteModel.AppendLiveOutput([]string{
			fmt.Sprintf("  [loop] Task %d: %s called %d times (loop detected)", msg.TaskID, msg.ToolName, msg.Count),
		})
	}
	return []tea.Cmd{DrainAdaptiveCmd(m)}
}

// HandleVerifyReport processes VerifyReportMsg: verification report.
func HandleVerifyReport(m *AppState, msg workflow.VerifyReportMsg) []tea.Cmd {
	if m.SidebarModel != nil {
		m.SidebarModel.AddTodoItem(SidebarTodoItem{
			Content:  fmt.Sprintf("Verify report: %d%% pass rate", msg.PassRate),
			Status:   "completed",
			Priority: "medium",
			Source:   "verify",
		})
	}
	return []tea.Cmd{DrainAdaptiveCmd(m)}
}

// HandleShipPreflight processes ShipPreflightMsg: pre-ship checklist.
func HandleShipPreflight(m *AppState, msg workflow.ShipPreflightMsg) []tea.Cmd {
	if m.SidebarModel != nil {
		status := "passed"
		if !msg.Passed {
			status = fmt.Sprintf("failed (%d issues)", len(msg.Issues))
		}
		m.SidebarModel.AddTodoItem(SidebarTodoItem{
			Content:  fmt.Sprintf("Ship preflight: %s", status),
			Status:   "completed",
			Priority: "medium",
			Source:   "ship",
		})
	}
	return []tea.Cmd{DrainAdaptiveCmd(m)}
}

// HandleShipChangelog processes ShipChangelogMsg: changelog generated.
func HandleShipChangelog(m *AppState, msg workflow.ShipChangelogMsg) []tea.Cmd {
	if m.SidebarModel != nil {
		m.SidebarModel.AddTodoItem(SidebarTodoItem{
			Content:  fmt.Sprintf("Changelog: %d entries", msg.Entries),
			Status:   "completed",
			Priority: "low",
			Source:   "ship",
		})
	}
	return []tea.Cmd{DrainAdaptiveCmd(m)}
}

// HandleCompactionComplete processes CompactionCompleteMsg: context compaction done.
func HandleCompactionComplete(m *AppState, msg workflow.CompactionCompleteMsg) []tea.Cmd {
	if m.SidebarModel != nil {
		m.SidebarModel.AddTodoItem(SidebarTodoItem{
			Content:  fmt.Sprintf("Compaction: %d → %d tokens (%d messages removed)", msg.TokensBefore, msg.TokensAfter, msg.MessagesRemoved),
			Status:   "completed",
			Priority: "low",
			Source:   "compaction",
		})
	}
	return []tea.Cmd{DrainAdaptiveCmd(m)}
}

// HandleTaskDiffSummary processes TaskDiffSummaryMsg: task commit diff stats.
func HandleTaskDiffSummary(m *AppState, msg workflow.TaskDiffSummaryMsg) []tea.Cmd {
	if m.ExecuteModel != nil && msg.Summary != nil {
		m.ExecuteModel.AppendLiveOutput([]string{
			fmt.Sprintf("  [diff] Task %d: +%d/-%d lines (%d files)",
				msg.TaskID, msg.Summary.Additions, msg.Summary.Deletions, len(msg.Summary.Files)),
		})
	}
	return []tea.Cmd{DrainAdaptiveCmd(m)}
}

// HandleAgentSwitch processes AgentSwitchMsg: agent mode switch.
func HandleAgentSwitch(m *AppState, msg workflow.AgentSwitchMsg) []tea.Cmd {
	if m.SidebarModel != nil {
		m.SidebarModel.AddTodoItem(SidebarTodoItem{
			Content:  fmt.Sprintf("Agent switch: %s → %s", msg.FromAgent, msg.ToAgent),
			Status:   "completed",
			Priority: "low",
			Source:   "agent",
		})
	}
	return []tea.Cmd{DrainAdaptiveCmd(m)}
}

// DrainAdaptiveCmd is a wrapper for m.drainAdaptiveCmd() to avoid direct access.
func DrainAdaptiveCmd(m *AppState) tea.Cmd {
	return m.drainAdaptiveCmd()
}