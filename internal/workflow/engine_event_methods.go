package workflow

// This file implements the narrative.WorkflowEvent interface on all message types.
// Each type returns its narrative event type and data payload.
// This allows pkg/narrative to convert workflow events without importing internal/workflow.

func (m TaskStartMsg) EventType() string { return "task_start" }
func (m TaskStartMsg) EventData() map[string]interface{} {
	return map[string]interface{}{
		"description": m.Task.Description,
		"task_id":     m.Task.ID,
	}
}

func (m TaskUpdateMsg) EventType() string { return "task_complete" }
func (m TaskUpdateMsg) EventData() map[string]interface{} {
	return map[string]interface{}{
		"description": m.Task.Description,
		"task_id":     m.Task.ID,
		"status":      m.Status,
	}
}

func (m ToolStartMsg) EventType() string { return "tool_start" }
func (m ToolStartMsg) EventData() map[string]interface{} {
	return map[string]interface{}{
		"tool_name":   m.ToolName,
		"description": m.Description,
	}
}

func (m ToolCompleteMsg) EventType() string { return "tool_complete" }
func (m ToolCompleteMsg) EventData() map[string]interface{} {
	return map[string]interface{}{
		"tool_name":   m.ToolName,
		"success":     m.Success,
		"duration_ms": m.DurationMs,
		"error":       m.Error,
		"file":        m.FilePath,
	}
}

func (m SelfHealStartMsg) EventType() string { return "selfheal_start" }
func (m SelfHealStartMsg) EventData() map[string]interface{} {
	return map[string]interface{}{
		"task_id": m.TaskID,
		"attempt": m.Attempt,
		"max":     m.Max,
	}
}

func (m SelfHealCompleteMsg) EventType() string { return "selfheal_complete" }
func (m SelfHealCompleteMsg) EventData() map[string]interface{} {
	return map[string]interface{}{
		"task_id": m.TaskID,
		"attempt": m.Attempt,
		"success": m.Success,
		"error":   m.Error,
	}
}

func (m PhaseTransitionStartMsg) EventType() string { return "phase_transition_start" }
func (m PhaseTransitionStartMsg) EventData() map[string]interface{} {
	return map[string]interface{}{
		"from":    m.From,
		"to":      m.To,
		"context": m.Context,
	}
}

func (m PhaseTransitionCompleteMsg) EventType() string { return "phase_transition_complete" }
func (m PhaseTransitionCompleteMsg) EventData() map[string]interface{} {
	return map[string]interface{}{
		"from":    m.From,
		"to":      m.To,
		"success": m.Success,
		"error":   m.Error,
	}
}

func (m IntermediateProgressMsg) EventType() string { return "intermediate_progress" }
func (m IntermediateProgressMsg) EventData() map[string]interface{} {
	return map[string]interface{}{
		"phase":   m.Phase,
		"message": m.Message,
	}
}

func (m ThinkingStartMsg) EventType() string { return "thinking_start" }
func (m ThinkingStartMsg) EventData() map[string]interface{} {
	return map[string]interface{}{
		"context": m.Context,
	}
}

func (m ThinkingCompleteMsg) EventType() string { return "thinking_complete" }
func (m ThinkingCompleteMsg) EventData() map[string]interface{} {
	return map[string]interface{}{
		"context": m.Context,
	}
}

func (m PlanCheckMsg) EventType() string { return "plan_ready" }
func (m PlanCheckMsg) EventData() map[string]interface{} {
	return map[string]interface{}{
		"passed":      m.Passed,
		"issue_count": m.IssueCount,
		"blockers":    m.Blockers,
		"warnings":    m.Warnings,
	}
}

func (m PlanRevisionMsg) EventType() string { return "plan_revision" }
func (m PlanRevisionMsg) EventData() map[string]interface{} {
	return map[string]interface{}{
		"iteration":        m.Iteration,
		"max_iterations":   m.MaxIterations,
		"issues_remaining": m.IssuesRemaining,
	}
}

func (m PlanChunkProgressMsg) EventType() string { return "plan_chunk" }
func (m PlanChunkProgressMsg) EventData() map[string]interface{} {
	return map[string]interface{}{
		"current": m.Wave,
		"total":   m.TotalWaves,
	}
}

func (m DiscussQualityMsg) EventType() string { return "discuss_start" }
func (m DiscussQualityMsg) EventData() map[string]interface{} {
	return map[string]interface{}{
		"passed":   m.Passed,
		"warnings": m.Warnings,
		"retried":  m.Retried,
	}
}

func (m DiscussCompletenessMsg) EventType() string { return "discuss_completeness" }
func (m DiscussCompletenessMsg) EventData() map[string]interface{} {
	return map[string]interface{}{
		"score":         m.Score,
		"missing_areas": m.MissingAreas,
	}
}

func (m ExecutePreflightMsg) EventType() string { return "execute_preflight" }
func (m ExecutePreflightMsg) EventData() map[string]interface{} {
	return map[string]interface{}{
		"passed": m.Passed,
	}
}

func (m ExecuteQualityGateMsg) EventType() string { return "execute_quality_gate" }
func (m ExecuteQualityGateMsg) EventData() map[string]interface{} {
	return map[string]interface{}{
		"task_id": m.TaskID,
		"passed":  m.Passed,
		"checked": m.Checked,
		"failed":  m.Failed,
	}
}

func (m ExecuteLoopDetectMsg) EventType() string { return "execute_loop_detect" }
func (m ExecuteLoopDetectMsg) EventData() map[string]interface{} {
	return map[string]interface{}{
		"task_id":   m.TaskID,
		"tool_name": m.ToolName,
		"count":     m.Count,
	}
}

func (m VerifyReportMsg) EventType() string { return "verify_report" }
func (m VerifyReportMsg) EventData() map[string]interface{} {
	return map[string]interface{}{
		"report":    m.Report,
		"pass_rate": m.PassRate,
	}
}

func (m RuntimeCheckCompleteMsg) EventType() string { return "runtime_check" }
func (m RuntimeCheckCompleteMsg) EventData() map[string]interface{} {
	return map[string]interface{}{
		"summary": m.Summary,
	}
}

func (m ShipPreflightMsg) EventType() string { return "ship_preflight" }
func (m ShipPreflightMsg) EventData() map[string]interface{} {
	return map[string]interface{}{
		"passed": m.Passed,
		"issues": m.Issues,
	}
}

func (m ShipChangelogMsg) EventType() string { return "ship_changelog" }
func (m ShipChangelogMsg) EventData() map[string]interface{} {
	return map[string]interface{}{
		"content":     m.Content,
		"entry_count": m.Entries,
	}
}

func (m DemonstrationReadyMsg) EventType() string { return "demonstration_ready" }
func (m DemonstrationReadyMsg) EventData() map[string]interface{} {
	return map[string]interface{}{
		"content": m.Content,
	}
}

func (m InitAnalysisMsg) EventType() string { return "init_analysis" }
func (m InitAnalysisMsg) EventData() map[string]interface{} {
	return map[string]interface{}{
		"project_type": m.ProjectType,
		"framework":    m.Framework,
		"language":     m.Language,
		"file_count":   m.FileCount,
		"health_score": m.HealthScore,
	}
}

func (m InitPreflightMsg) EventType() string { return "init_preflight" }
func (m InitPreflightMsg) EventData() map[string]interface{} {
	return map[string]interface{}{
		"passed": m.Passed,
	}
}

func (m ResearchProgressMsg) EventType() string { return "research_progress" }
func (m ResearchProgressMsg) EventData() map[string]interface{} {
	return map[string]interface{}{
		"message":  m.Message,
		"complete": m.Complete,
	}
}

func (m CompactionCompleteMsg) EventType() string { return "compaction_complete" }
func (m CompactionCompleteMsg) EventData() map[string]interface{} {
	return map[string]interface{}{
		"tokens_before":    m.TokensBefore,
		"tokens_after":     m.TokensAfter,
		"messages_removed": m.MessagesRemoved,
	}
}

func (m TaskDiffSummaryMsg) EventType() string { return "task_diff" }
func (m TaskDiffSummaryMsg) EventData() map[string]interface{} {
	additions := 0
	deletions := 0
	if m.Summary != nil {
		additions = m.Summary.Additions
		deletions = m.Summary.Deletions
	}
	return map[string]interface{}{
		"task_id":   m.TaskID,
		"additions": additions,
		"deletions": deletions,
	}
}
