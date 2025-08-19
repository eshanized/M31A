package tui

import (
	"testing"
	"time"

	"github.com/eshanized/M31A/internal/tui/theme"
	"github.com/eshanized/M31A/internal/types"
	"github.com/eshanized/M31A/internal/workflow"
)

func TestWorkflowPhaseSync(t *testing.T) {
	app := &AppState{}

	tests := []struct {
		phase           types.WorkflowPhase
		expectRunning   bool
		expectPhase     types.WorkflowPhase
	}{
		{types.PhaseIdle, false, types.PhaseIdle},
		{types.PhaseInitialize, true, types.PhaseInitialize},
		{types.PhaseDiscuss, true, types.PhaseDiscuss},
		{types.PhasePlan, true, types.PhasePlan},
		{types.PhaseExecute, true, types.PhaseExecute},
		{types.PhaseVerify, true, types.PhaseVerify},
		{types.PhaseShip, true, types.PhaseShip},
	}

	for _, tt := range tests {
		t.Run(string(tt.phase), func(t *testing.T) {
			app.setWorkflowPhase(tt.phase)
			if app.workflowRunning != tt.expectRunning {
				t.Errorf("workflowRunning = %v, want %v for phase %s", app.workflowRunning, tt.expectRunning, tt.phase)
			}
			if app.currentPhase != tt.expectPhase {
				t.Errorf("currentPhase = %v, want %v", app.currentPhase, tt.expectPhase)
			}
			if app.headerCacheValid {
				t.Errorf("headerCacheValid should be false after phase change")
			}
		})
	}
}

func TestSessionIDPropagation(t *testing.T) {
	rp := NewReplModel(theme.Dark(), "test")
	rp.SetSessionID("abc123")
	if rp.sessionID != "abc123" {
		t.Errorf("ReplModel.sessionID = %q, want %q", rp.sessionID, "abc123")
	}

	rp.SetSessionID("def456")
	if rp.sessionID != "def456" {
		t.Errorf("ReplModel.sessionID after update = %q, want %q", rp.sessionID, "def456")
	}
}

func TestPlanModelUpdate(t *testing.T) {
	tasks := []types.Task{
		{ID: 1, Description: "Task 1", Status: types.StatusPending},
		{ID: 2, Description: "Task 2", Status: types.StatusPending},
		{ID: 3, Description: "Task 3", Status: types.StatusPending},
	}
	pm := NewPlanModel(tasks, theme.Dark(), "model", "Model", "provider", 0, "", 80, 24)
	pm.selected = 2

	newTasks := []types.Task{
		{ID: 1, Description: "Updated Task 1", Status: types.StatusDone},
	}
	pm.UpdateTasks(newTasks)

	if len(pm.tasks) != 1 {
		t.Errorf("tasks length = %d, want 1", len(pm.tasks))
	}
	if pm.selected != 0 {
		t.Errorf("selected = %d, want 0 (adjusted after task removal)", pm.selected)
	}

	pm.SetDimensions(120, 40)
	if pm.width != 120 || pm.height != 40 {
		t.Errorf("dimensions = (%d, %d), want (120, 40)", pm.width, pm.height)
	}
}

func TestVerifyModelResults(t *testing.T) {
	tasks := []types.Task{
		{ID: 1, Description: "Task 1", Status: types.StatusDone},
		{ID: 2, Description: "Task 2", Status: types.StatusFailed},
	}
	results := make(map[int]workflow.VerificationResult)
	vm := NewVerifyModel(tasks, results, theme.Dark(), 80, 24)

	newResults := map[int]workflow.VerificationResult{
		1: {FilesExist: true, SyntaxOK: true, TestsOK: true},
		2: {FilesExist: false, SyntaxOK: false, TestsOK: false},
	}
	vm.UpdateResults(newResults)

	if len(vm.results) != 2 {
		t.Errorf("results length = %d, want 2", len(vm.results))
	}
	if !vm.results[1].FilesExist {
		t.Error("task 1 FilesExist should be true")
	}
	if vm.results[2].FilesExist {
		t.Error("task 2 FilesExist should be false")
	}
}

func TestShipModelSummary(t *testing.T) {
	summary := ShipSummary{
		TaskDone:  5,
		TaskTotal: 6,
		Model:     "claude-3-opus",
		Provider:  "openrouter",
		SessionID: "abc123",
		Duration:  "2m 30s",
	}
	sm := NewShipModel(summary, theme.Dark(), 80, 24)

	if sm.summary.Model != "claude-3-opus" {
		t.Errorf("Model = %q, want %q", sm.summary.Model, "claude-3-opus")
	}
	if sm.summary.Provider != "openrouter" {
		t.Errorf("Provider = %q, want %q", sm.summary.Provider, "openrouter")
	}
}

func TestMessageHistoryBounded(t *testing.T) {
	rp := NewReplModel(theme.Dark(), "test")
	rp.width = 80
	rp.height = 24
	rp.msgRenderer = nil

	for i := 0; i < MaxMessageHistory+100; i++ {
		rp.AddMessage(types.Message{
			Role:      "assistant",
			Content:   "test",
			CreatedAt: time.Now(),
		})
	}

	if len(rp.messages) > MaxMessageHistory {
		t.Errorf("messages length = %d, should be <= %d (MaxMessageHistory)", len(rp.messages), MaxMessageHistory)
	}
}

func TestPermissionRequestQueue(t *testing.T) {
	app := &AppState{
		permissionModalActive: true,
		pendingPermissionRequests: nil,
	}

	msg1 := PermissionRequestMsg{}
	msg2 := PermissionRequestMsg{}

	app.pendingPermissionRequests = append(app.pendingPermissionRequests, msg1)
	app.pendingPermissionRequests = append(app.pendingPermissionRequests, msg2)

	if len(app.pendingPermissionRequests) != 2 {
		t.Errorf("pending queue length = %d, want 2", len(app.pendingPermissionRequests))
	}

	next := app.pendingPermissionRequests[0]
	app.pendingPermissionRequests = app.pendingPermissionRequests[1:]
	_ = next

	if len(app.pendingPermissionRequests) != 1 {
		t.Errorf("pending queue length after dequeue = %d, want 1", len(app.pendingPermissionRequests))
	}
}

func TestStreamMsgPhaseTransition(t *testing.T) {
	app := &AppState{}
	app.setWorkflowPhase(types.PhaseExecute)

	chunk := &types.StreamChunk{Type: "content", Delta: "test"}

	if !app.workflowRunning {
		t.Error("workflow should be running during execute phase")
	}
	if app.currentPhase == types.PhaseDiscuss {
		t.Error("current phase should not be discuss during execute")
	}

	app.pendingStreamChunks = append(app.pendingStreamChunks, chunk)
	if len(app.pendingStreamChunks) != 1 {
		t.Errorf("pending chunks = %d, want 1", len(app.pendingStreamChunks))
	}
}

func TestWorkflowStartTime(t *testing.T) {
	app := &AppState{}
	app.workflowStartTime = time.Now()
	time.Sleep(10 * time.Millisecond)

	elapsed := time.Since(app.workflowStartTime)
	if elapsed < 10*time.Millisecond {
		t.Errorf("workflow start time elapsed = %v, should be >= 10ms", elapsed)
	}
}

func TestFlushPendingStreamChunks(t *testing.T) {
	rp := NewReplModel(theme.Dark(), "test")
	rp.width = 80
	rp.height = 24

	app := &AppState{
		replModel: &rp,
		pendingStreamChunks: []*types.StreamChunk{
			{Type: "content", Delta: "hello "},
			{Type: "content", Delta: "world"},
		},
	}

	app.flushPendingStreamChunks()

	if len(app.pendingStreamChunks) != 0 {
		t.Errorf("pending chunks after flush = %d, want 0", len(app.pendingStreamChunks))
	}
}
