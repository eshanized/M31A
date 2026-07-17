package narrative

import (
	"testing"

	m31types "github.com/eshanized/M31A/internal/types"
	"github.com/eshanized/M31A/internal/workflow"
)

func TestBridgeTaskStart(t *testing.T) {
	b := NewBridge()
	event, ok := b.MsgToRawEvent(workflow.TaskStartMsg{
		Task: m31types.Task{ID: 1, Description: "implement auth"},
	})
	if !ok {
		t.Fatal("Should convert TaskStartMsg")
	}
	if event.Type != EventTaskStart {
		t.Errorf("Type = %q, want %q", event.Type, EventTaskStart)
	}
	if event.GetString("description") != "implement auth" {
		t.Errorf("description = %q", event.GetString("description"))
	}
}

func TestBridgeTaskUpdate(t *testing.T) {
	b := NewBridge()
	event, ok := b.MsgToRawEvent(workflow.TaskUpdateMsg{
		Task:   m31types.Task{ID: 1, Description: "fix bug"},
		Status: "done",
	})
	if !ok {
		t.Fatal("Should convert TaskUpdateMsg")
	}
	if event.Type != EventTaskComplete {
		t.Errorf("Type = %q, want %q", event.Type, EventTaskComplete)
	}
}

func TestBridgeToolStart(t *testing.T) {
	b := NewBridge()
	event, ok := b.MsgToRawEvent(workflow.ToolStartMsg{
		ToolName:    "FileRead",
		Description: "reading main.go",
	})
	if !ok {
		t.Fatal("Should convert ToolStartMsg")
	}
	if event.Type != EventToolStart {
		t.Errorf("Type = %q, want %q", event.Type, EventToolStart)
	}
	if event.GetString("tool_name") != "FileRead" {
		t.Errorf("tool_name = %q", event.GetString("tool_name"))
	}
}

func TestBridgeToolComplete(t *testing.T) {
	b := NewBridge()
	event, ok := b.MsgToRawEvent(workflow.ToolCompleteMsg{
		ToolName:   "Edit",
		Success:    true,
		DurationMs: 150,
		FilePath:   "main.go",
	})
	if !ok {
		t.Fatal("Should convert ToolCompleteMsg")
	}
	if event.Type != EventToolComplete {
		t.Errorf("Type = %q, want %q", event.Type, EventToolComplete)
	}
	if !event.GetBool("success") {
		t.Error("success should be true")
	}
}

func TestBridgeSelfHealStart(t *testing.T) {
	b := NewBridge()
	event, ok := b.MsgToRawEvent(workflow.SelfHealStartMsg{
		TaskID:  1,
		Attempt: 2,
		Max:     3,
	})
	if !ok {
		t.Fatal("Should convert SelfHealStartMsg")
	}
	if event.Type != EventSelfHealStart {
		t.Errorf("Type = %q, want %q", event.Type, EventSelfHealStart)
	}
}

func TestBridgeSelfHealComplete(t *testing.T) {
	b := NewBridge()
	event, ok := b.MsgToRawEvent(workflow.SelfHealCompleteMsg{
		TaskID:  1,
		Attempt: 2,
		Success: true,
	})
	if !ok {
		t.Fatal("Should convert SelfHealCompleteMsg")
	}
	if event.Type != EventSelfHealComplete {
		t.Errorf("Type = %q, want %q", event.Type, EventSelfHealComplete)
	}
}

func TestBridgePhaseTransitionStart(t *testing.T) {
	b := NewBridge()
	event, ok := b.MsgToRawEvent(workflow.PhaseTransitionStartMsg{
		From: "plan",
		To:   "execute",
	})
	if !ok {
		t.Fatal("Should convert PhaseTransitionStartMsg")
	}
	if event.Type != EventPhaseTransitionStart {
		t.Errorf("Type = %q, want %q", event.Type, EventPhaseTransitionStart)
	}
	if event.GetString("from") != "plan" {
		t.Errorf("from = %q", event.GetString("from"))
	}
	if event.GetString("to") != "execute" {
		t.Errorf("to = %q", event.GetString("to"))
	}
}

func TestBridgePhaseTransitionComplete(t *testing.T) {
	b := NewBridge()
	event, ok := b.MsgToRawEvent(workflow.PhaseTransitionCompleteMsg{
		From:    "execute",
		To:      "verify",
		Success: true,
	})
	if !ok {
		t.Fatal("Should convert PhaseTransitionCompleteMsg")
	}
	if event.Type != EventPhaseTransitionComplete {
		t.Errorf("Type = %q, want %q", event.Type, EventPhaseTransitionComplete)
	}
}

func TestBridgePlanCheck(t *testing.T) {
	b := NewBridge()
	event, ok := b.MsgToRawEvent(workflow.PlanCheckMsg{
		Passed:     true,
		IssueCount: 2,
	})
	if !ok {
		t.Fatal("Should convert PlanCheckMsg")
	}
	if event.Type != EventPlanReady {
		t.Errorf("Type = %q, want %q", event.Type, EventPlanReady)
	}
}

func TestBridgePlanRevision(t *testing.T) {
	b := NewBridge()
	event, ok := b.MsgToRawEvent(workflow.PlanRevisionMsg{
		Iteration:       1,
		MaxIterations:   3,
		IssuesRemaining: 5,
	})
	if !ok {
		t.Fatal("Should convert PlanRevisionMsg")
	}
	if event.Type != EventPlanRevision {
		t.Errorf("Type = %q, want %q", event.Type, EventPlanRevision)
	}
}

func TestBridgePlanChunkProgress(t *testing.T) {
	b := NewBridge()
	event, ok := b.MsgToRawEvent(workflow.PlanChunkProgressMsg{
		Wave:       2,
		TotalWaves: 5,
	})
	if !ok {
		t.Fatal("Should convert PlanChunkProgressMsg")
	}
	if event.Type != EventPlanChunk {
		t.Errorf("Type = %q, want %q", event.Type, EventPlanChunk)
	}
}

func TestBridgeExecutePreflight(t *testing.T) {
	b := NewBridge()
	event, ok := b.MsgToRawEvent(workflow.ExecutePreflightMsg{
		Passed: true,
	})
	if !ok {
		t.Fatal("Should convert ExecutePreflightMsg")
	}
	if event.Type != EventExecutePreflight {
		t.Errorf("Type = %q, want %q", event.Type, EventExecutePreflight)
	}
}

func TestBridgeExecuteLoopDetect(t *testing.T) {
	b := NewBridge()
	event, ok := b.MsgToRawEvent(workflow.ExecuteLoopDetectMsg{
		TaskID:   1,
		ToolName: "Bash",
		Count:    5,
	})
	if !ok {
		t.Fatal("Should convert ExecuteLoopDetectMsg")
	}
	if event.Type != EventExecuteLoopDetect {
		t.Errorf("Type = %q, want %q", event.Type, EventExecuteLoopDetect)
	}
}

func TestBridgeVerifyReport(t *testing.T) {
	b := NewBridge()
	event, ok := b.MsgToRawEvent(workflow.VerifyReportMsg{
		Report:   "all tests pass",
		PassRate: 100,
	})
	if !ok {
		t.Fatal("Should convert VerifyReportMsg")
	}
	if event.Type != EventVerifyReport {
		t.Errorf("Type = %q, want %q", event.Type, EventVerifyReport)
	}
}

func TestBridgeShipPreflight(t *testing.T) {
	b := NewBridge()
	event, ok := b.MsgToRawEvent(workflow.ShipPreflightMsg{
		Passed: true,
	})
	if !ok {
		t.Fatal("Should convert ShipPreflightMsg")
	}
	if event.Type != EventShipPreflight {
		t.Errorf("Type = %q, want %q", event.Type, EventShipPreflight)
	}
}

func TestBridgeShipChangelog(t *testing.T) {
	b := NewBridge()
	event, ok := b.MsgToRawEvent(workflow.ShipChangelogMsg{
		Content: "changelog content",
		Entries: 5,
	})
	if !ok {
		t.Fatal("Should convert ShipChangelogMsg")
	}
	if event.Type != EventShipChangelog {
		t.Errorf("Type = %q, want %q", event.Type, EventShipChangelog)
	}
}

func TestBridgeInitAnalysis(t *testing.T) {
	b := NewBridge()
	event, ok := b.MsgToRawEvent(workflow.InitAnalysisMsg{
		ProjectType: "go",
		Framework:   "bubbletea",
		FileCount:   100,
	})
	if !ok {
		t.Fatal("Should convert InitAnalysisMsg")
	}
	if event.Type != EventInitAnalysis {
		t.Errorf("Type = %q, want %q", event.Type, EventInitAnalysis)
	}
}

func TestBridgeInitPreflight(t *testing.T) {
	b := NewBridge()
	event, ok := b.MsgToRawEvent(workflow.InitPreflightMsg{
		Passed: true,
	})
	if !ok {
		t.Fatal("Should convert InitPreflightMsg")
	}
	if event.Type != EventInitPreflight {
		t.Errorf("Type = %q, want %q", event.Type, EventInitPreflight)
	}
}

func TestBridgeResearchProgress(t *testing.T) {
	b := NewBridge()
	event, ok := b.MsgToRawEvent(workflow.ResearchProgressMsg{
		Message:  "searching docs",
		Complete: false,
	})
	if !ok {
		t.Fatal("Should convert ResearchProgressMsg")
	}
	if event.Type != EventResearchProgress {
		t.Errorf("Type = %q, want %q", event.Type, EventResearchProgress)
	}
}

func TestBridgeCompactionComplete(t *testing.T) {
	b := NewBridge()
	event, ok := b.MsgToRawEvent(workflow.CompactionCompleteMsg{
		TokensBefore:    10000,
		TokensAfter:     5000,
		MessagesRemoved: 20,
	})
	if !ok {
		t.Fatal("Should convert CompactionCompleteMsg")
	}
	if event.Type != EventCompactionComplete {
		t.Errorf("Type = %q, want %q", event.Type, EventCompactionComplete)
	}
}

func TestBridgeRuntimeCheckComplete(t *testing.T) {
	b := NewBridge()
	event, ok := b.MsgToRawEvent(workflow.RuntimeCheckCompleteMsg{})
	if !ok {
		t.Fatal("Should convert RuntimeCheckCompleteMsg")
	}
	if event.Type != EventRuntimeCheck {
		t.Errorf("Type = %q, want %q", event.Type, EventRuntimeCheck)
	}
}

func TestBridgeTaskDiffSummary(t *testing.T) {
	b := NewBridge()
	event, ok := b.MsgToRawEvent(workflow.TaskDiffSummaryMsg{
		TaskID: 1,
		Summary: &m31types.DiffSummary{
			Additions: 50,
			Deletions: 10,
		},
	})
	if !ok {
		t.Fatal("Should convert TaskDiffSummaryMsg")
	}
	if event.Type != EventTaskDiff {
		t.Errorf("Type = %q, want %q", event.Type, EventTaskDiff)
	}
	if event.GetInt("additions") != 50 {
		t.Errorf("additions = %d, want 50", event.GetInt("additions"))
	}
}

func TestBridgeUnknownMessage(t *testing.T) {
	b := NewBridge()
	_, ok := b.MsgToRawEvent("some random string")
	if ok {
		t.Error("Should not convert unknown message type")
	}
}

func TestBridgeNilSummary(t *testing.T) {
	b := NewBridge()
	event, ok := b.MsgToRawEvent(workflow.TaskDiffSummaryMsg{
		TaskID:  1,
		Summary: nil,
	})
	if !ok {
		t.Fatal("Should convert TaskDiffSummaryMsg even with nil summary")
	}
	if event.GetInt("additions") != 0 {
		t.Errorf("additions = %d, want 0 for nil summary", event.GetInt("additions"))
	}
}

func TestBridgeEndToEnd(t *testing.T) {
	b := NewBridge()
	engine := NewEngine(DefaultEngineConfig())

	// Simulate a sequence of workflow messages
	msgs := []interface{}{
		workflow.PhaseTransitionStartMsg{From: "idle", To: "execute"},
		workflow.TaskStartMsg{Task: m31types.Task{ID: 1, Description: "fix auth"}},
		workflow.ToolStartMsg{ToolName: "FileRead", Description: "reading main.go"},
		workflow.ToolStartMsg{ToolName: "FileRead", Description: "reading config.go"},
		workflow.ToolCompleteMsg{ToolName: "FileRead", Success: true, DurationMs: 50},
		workflow.ToolCompleteMsg{ToolName: "FileRead", Success: true, DurationMs: 30},
		workflow.ToolStartMsg{ToolName: "Edit", Description: "editing main.go"},
		workflow.ToolCompleteMsg{ToolName: "Edit", Success: true, DurationMs: 100, FilePath: "main.go"},
		workflow.TaskUpdateMsg{Task: m31types.Task{ID: 1, Description: "fix auth"}, Status: "done"},
		workflow.PhaseTransitionCompleteMsg{From: "execute", To: "verify", Success: true},
	}

	var narratives []NarrativeObject
	for _, msg := range msgs {
		event, ok := b.MsgToRawEvent(msg)
		if !ok {
			continue
		}
		narratives = append(narratives, engine.ProcessEvent(event)...)
	}

	// Flush any pending groups
	narratives = append(narratives, engine.FlushGroups()...)

	if len(narratives) == 0 {
		t.Fatal("Expected at least one narrative from end-to-end flow")
	}

	// Verify that we got meaningful narratives
	for _, n := range narratives {
		if n.Text == "" {
			t.Errorf("Narrative %q has empty text", n.Type)
		}
	}
}
