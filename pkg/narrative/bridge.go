package narrative

import (
	"time"

	"github.com/eshanized/M31A/internal/workflow"
)

// Bridge converts workflow engine messages into narrative RawEvents.
// It is a pure adapter with no state of its own.
type Bridge struct{}

// NewBridge creates a new narrative bridge.
func NewBridge() *Bridge {
	return &Bridge{}
}

// MsgToRawEvent converts a workflow engine message into a RawEvent.
// Returns ok=true if the message was converted, ok=false if it should be ignored.
func (b *Bridge) MsgToRawEvent(msg interface{}) (RawEvent, bool) {
	now := time.Now()

	switch m := msg.(type) {
	// ── Task events ──────────────────────────────────────────────────────
	case workflow.TaskStartMsg:
		return RawEvent{
			Type:      EventTaskStart,
			Timestamp: now,
			Data: map[string]interface{}{
				"description": m.Task.Description,
				"task_id":     m.Task.ID,
			},
		}, true

	case workflow.TaskUpdateMsg:
		return RawEvent{
			Type:      EventTaskComplete,
			Timestamp: now,
			Data: map[string]interface{}{
				"description": m.Task.Description,
				"task_id":     m.Task.ID,
				"status":      m.Status,
			},
		}, true

	// ── Tool events ──────────────────────────────────────────────────────
	case workflow.ToolStartMsg:
		data := map[string]interface{}{
			"tool_name":   m.ToolName,
			"description": m.Description,
		}
		return RawEvent{
			Type:      EventToolStart,
			Timestamp: now,
			Data:      data,
		}, true

	case workflow.ToolCompleteMsg:
		return RawEvent{
			Type:      EventToolComplete,
			Timestamp: now,
			Data: map[string]interface{}{
				"tool_name":   m.ToolName,
				"success":     m.Success,
				"duration_ms": m.DurationMs,
				"error":       m.Error,
				"file":        m.FilePath,
			},
		}, true

	// ── Self-heal events ─────────────────────────────────────────────────
	case workflow.SelfHealStartMsg:
		return RawEvent{
			Type:      EventSelfHealStart,
			Timestamp: now,
			Data: map[string]interface{}{
				"task_id": m.TaskID,
				"attempt": m.Attempt,
				"max":     m.Max,
			},
		}, true

	case workflow.SelfHealCompleteMsg:
		return RawEvent{
			Type:      EventSelfHealComplete,
			Timestamp: now,
			Data: map[string]interface{}{
				"task_id": m.TaskID,
				"attempt": m.Attempt,
				"success": m.Success,
				"error":   m.Error,
			},
		}, true

	// ── Phase transitions ────────────────────────────────────────────────
	case workflow.PhaseTransitionStartMsg:
		return RawEvent{
			Type:      EventPhaseTransitionStart,
			Timestamp: now,
			Data: map[string]interface{}{
				"from":    m.From,
				"to":      m.To,
				"context": m.Context,
			},
		}, true

	case workflow.PhaseTransitionCompleteMsg:
		return RawEvent{
			Type:      EventPhaseTransitionComplete,
			Timestamp: now,
			Data: map[string]interface{}{
				"from":    m.From,
				"to":      m.To,
				"success": m.Success,
				"error":   m.Error,
			},
		}, true

	// ── Plan events ──────────────────────────────────────────────────────
	case workflow.PlanCheckMsg:
		return RawEvent{
			Type:      EventPlanReady,
			Timestamp: now,
			Data: map[string]interface{}{
				"passed":      m.Passed,
				"issue_count": m.IssueCount,
				"blockers":    m.Blockers,
				"warnings":    m.Warnings,
			},
		}, true

	case workflow.PlanRevisionMsg:
		return RawEvent{
			Type:      EventPlanRevision,
			Timestamp: now,
			Data: map[string]interface{}{
				"iteration":        m.Iteration,
				"max_iterations":   m.MaxIterations,
				"issues_remaining": m.IssuesRemaining,
			},
		}, true

	case workflow.PlanChunkProgressMsg:
		return RawEvent{
			Type:      EventPlanChunk,
			Timestamp: now,
			Data: map[string]interface{}{
				"current": m.Wave,
				"total":   m.TotalWaves,
			},
		}, true

	// ── Discuss events ───────────────────────────────────────────────────
	case workflow.DiscussQualityMsg:
		return RawEvent{
			Type:      EventDiscussStart,
			Timestamp: now,
			Data: map[string]interface{}{
				"passed":   m.Passed,
				"warnings": m.Warnings,
				"retried":  m.Retried,
			},
		}, true

	// ── Execute events ───────────────────────────────────────────────────
	case workflow.ExecutePreflightMsg:
		return RawEvent{
			Type:      EventExecutePreflight,
			Timestamp: now,
			Data: map[string]interface{}{
				"passed": m.Passed,
			},
		}, true

	case workflow.ExecuteLoopDetectMsg:
		return RawEvent{
			Type:      EventExecuteLoopDetect,
			Timestamp: now,
			Data: map[string]interface{}{
				"task_id":   m.TaskID,
				"tool_name": m.ToolName,
				"count":     m.Count,
			},
		}, true

	// ── Verify events ────────────────────────────────────────────────────
	case workflow.VerifyReportMsg:
		return RawEvent{
			Type:      EventVerifyReport,
			Timestamp: now,
			Data: map[string]interface{}{
				"report":    m.Report,
				"pass_rate": m.PassRate,
			},
		}, true

	// ── Ship events ──────────────────────────────────────────────────────
	case workflow.ShipPreflightMsg:
		return RawEvent{
			Type:      EventShipPreflight,
			Timestamp: now,
			Data: map[string]interface{}{
				"passed": m.Passed,
				"issues": m.Issues,
			},
		}, true

	case workflow.ShipChangelogMsg:
		return RawEvent{
			Type:      EventShipChangelog,
			Timestamp: now,
			Data: map[string]interface{}{
				"content":     m.Content,
				"entry_count": m.Entries,
			},
		}, true

	case workflow.DemonstrationReadyMsg:
		return RawEvent{
			Type:      EventDemonstrationReady,
			Timestamp: now,
			Data: map[string]interface{}{
				"content": m.Content,
			},
		}, true

	// ── Init events ──────────────────────────────────────────────────────
	case workflow.InitAnalysisMsg:
		return RawEvent{
			Type:      EventInitAnalysis,
			Timestamp: now,
			Data: map[string]interface{}{
				"project_type": m.ProjectType,
				"framework":    m.Framework,
				"language":     m.Language,
				"file_count":   m.FileCount,
				"health_score": m.HealthScore,
			},
		}, true

	case workflow.InitPreflightMsg:
		return RawEvent{
			Type:      EventInitPreflight,
			Timestamp: now,
			Data: map[string]interface{}{
				"passed": m.Passed,
			},
		}, true

	// ── Research events ──────────────────────────────────────────────────
	case workflow.ResearchProgressMsg:
		return RawEvent{
			Type:      EventResearchProgress,
			Timestamp: now,
			Data: map[string]interface{}{
				"message":  m.Message,
				"complete": m.Complete,
			},
		}, true

	// ── Compaction events ────────────────────────────────────────────────
	case workflow.CompactionCompleteMsg:
		return RawEvent{
			Type:      EventCompactionComplete,
			Timestamp: now,
			Data: map[string]interface{}{
				"tokens_before":    m.TokensBefore,
				"tokens_after":     m.TokensAfter,
				"messages_removed": m.MessagesRemoved,
			},
		}, true

	// ── Runtime events ───────────────────────────────────────────────────
	case workflow.RuntimeCheckCompleteMsg:
		return RawEvent{
			Type:      EventRuntimeCheck,
			Timestamp: now,
			Data: map[string]interface{}{
				"summary": m.Summary,
			},
		}, true

	// ── Task diff events ─────────────────────────────────────────────────
	case workflow.TaskDiffSummaryMsg:
		additions := 0
		deletions := 0
		if m.Summary != nil {
			additions = m.Summary.Additions
			deletions = m.Summary.Deletions
		}
		return RawEvent{
			Type:      EventTaskDiff,
			Timestamp: now,
			Data: map[string]interface{}{
				"task_id":   m.TaskID,
				"additions": additions,
				"deletions": deletions,
			},
		}, true

	default:
		return RawEvent{}, false
	}
}
