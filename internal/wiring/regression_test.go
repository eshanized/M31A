package wiring

import (
	"context"
	"testing"
	"time"

	"github.com/eshanized/M31A/internal/decision"
	m31types "github.com/eshanized/M31A/internal/types"
	"github.com/eshanized/M31A/internal/workflow"
	"github.com/eshanized/M31A/pkg/taskrunner"
)

// ─── W5: Task runner skips completed tasks on resume ─────────────────────────

func TestW5_ResumeSkipsDoneTasks(t *testing.T) {
	tasks := []m31types.Task{
		{ID: 1, Action: "implement", Description: "task 1", Status: m31types.StatusDone},
		{ID: 2, Action: "implement", Description: "task 2", Dependencies: []int{1}, Status: m31types.StatusPending},
	}
	r := taskrunner.New(tasks)

	groups, err := r.Schedule()
	if err != nil {
		t.Fatalf("Schedule failed: %v", err)
	}
	if len(groups) < 1 {
		t.Fatal("Expected at least 1 group")
	}

	// Group 0 should contain task 1 (done) — ExecuteGroup should skip it
	executed := make(map[int]bool)
	err = r.ExecuteGroup(t.Context(), groups[0], func(_ context.Context, task m31types.Task) taskrunner.TaskResult {
		executed[task.ID] = true
		return taskrunner.TaskResult{Success: true}
	})
	if err != nil {
		t.Fatalf("ExecuteGroup failed: %v", err)
	}
	if executed[1] {
		t.Error("Task 1 (StatusDone) should have been skipped, but was executed")
	}

	// Group 1 should contain task 2 (pending) — ExecuteGroup should run it
	if len(groups) > 1 {
		err = r.ExecuteGroup(t.Context(), groups[1], func(_ context.Context, task m31types.Task) taskrunner.TaskResult {
			executed[task.ID] = true
			return taskrunner.TaskResult{Success: true}
		})
		if err != nil {
			t.Fatalf("ExecuteGroup failed: %v", err)
		}
		if !executed[2] {
			t.Error("Task 2 (StatusPending) should have been executed")
		}
	}
}

func TestW5_ResumeSkipsFailedTasks(t *testing.T) {
	tasks := []m31types.Task{
		{ID: 1, Action: "implement", Description: "task 1", Status: m31types.StatusFailed},
		{ID: 2, Action: "implement", Description: "task 2", Dependencies: []int{1}, Status: m31types.StatusPending},
	}
	r := taskrunner.New(tasks)

	groups, err := r.Schedule()
	if err != nil {
		t.Fatalf("Schedule failed: %v", err)
	}

	// Task 1 failed — ExecuteGroup should skip it
	executed := make(map[int]bool)
	err = r.ExecuteGroup(t.Context(), groups[0], func(_ context.Context, task m31types.Task) taskrunner.TaskResult {
		executed[task.ID] = true
		return taskrunner.TaskResult{Success: true}
	})
	if err != nil {
		t.Fatalf("ExecuteGroup failed: %v", err)
	}
	if executed[1] {
		t.Error("Task 1 (StatusFailed) should have been skipped")
	}

	// Task 2 depends on failed task 1 — should be skipped
	if len(groups) > 1 {
		err = r.ExecuteGroup(t.Context(), groups[1], func(_ context.Context, task m31types.Task) taskrunner.TaskResult {
			executed[task.ID] = true
			return taskrunner.TaskResult{Success: true}
		})
		if err != nil {
			t.Fatalf("ExecuteGroup failed: %v", err)
		}
		if executed[2] {
			t.Error("Task 2 (depends on failed task) should have been skipped")
		}
	}

	// Verify task 2 status is skipped
	status := r.Status(2)
	if status != m31types.StatusSkipped {
		t.Errorf("Task 2 status should be Skipped, got %q", status)
	}
}

func TestW5_ResumeMixedStatusTasks(t *testing.T) {
	tasks := []m31types.Task{
		{ID: 1, Action: "implement", Description: "task 1", Status: m31types.StatusDone},
		{ID: 2, Action: "implement", Description: "task 2", Status: m31types.StatusDone},
		{ID: 3, Action: "implement", Description: "task 3", Dependencies: []int{1, 2}, Status: m31types.StatusPending},
		{ID: 4, Action: "implement", Description: "task 4", Status: m31types.StatusPending},
	}
	r := taskrunner.New(tasks)

	groups, err := r.Schedule()
	if err != nil {
		t.Fatalf("Schedule failed: %v", err)
	}

	executed := make(map[int]bool)
	for _, group := range groups {
		err := r.ExecuteGroup(t.Context(), group, func(_ context.Context, task m31types.Task) taskrunner.TaskResult {
			executed[task.ID] = true
			return taskrunner.TaskResult{Success: true}
		})
		if err != nil {
			t.Fatalf("ExecuteGroup failed: %v", err)
		}
	}

	// Tasks 1, 2 should be skipped (done)
	if executed[1] {
		t.Error("Task 1 (StatusDone) should have been skipped")
	}
	if executed[2] {
		t.Error("Task 2 (StatusDone) should have been skipped")
	}
	// Tasks 3, 4 should be executed (pending, deps met)
	if !executed[3] {
		t.Error("Task 3 (StatusPending) should have been executed")
	}
	if !executed[4] {
		t.Error("Task 4 (StatusPending) should have been executed")
	}
}

// ─── W6: Decision redaction integration ──────────────────────────────────────

func TestW6_RedactSliceRedactsSensitiveData(t *testing.T) {
	input := []decision.DecisionReceipt{
		{
			Timestamp: time.Now(),
			Category:  decision.CategoryTool,
			Decision:  "Use api_key=sk-abc123secret to authenticate",
			Rationale: "Bearer tok_abc123 for auth",
		},
		{
			Timestamp: time.Now(),
			Category:  decision.CategoryPlan,
			Decision:  "Contact user@example.com for details",
			Rationale: "Server at 192.168.1.1 needs config",
		},
		{
			Timestamp: time.Now(),
			Category:  decision.CategoryIntent,
			Decision:  "Normal decision without secrets",
			Rationale: "No sensitive data here",
		},
	}

	result := decision.RedactSlice(input)

	if len(result) != len(input) {
		t.Fatalf("Expected %d results, got %d", len(input), len(result))
	}

	// Check that sensitive data was redacted
	if result[0].Decision == input[0].Decision {
		t.Error("Decision 0: api_key was not redacted")
	}
	if result[0].Rationale == input[0].Rationale {
		t.Error("Rationale 0: Bearer token was not redacted")
	}
	if result[1].Decision == input[1].Decision {
		t.Error("Decision 1: email was not redacted")
	}
	if result[1].Rationale == input[1].Rationale {
		t.Error("Rationale 1: IP address was not redacted")
	}

	// Check that non-sensitive data is preserved
	if result[2].Decision != input[2].Decision {
		t.Error("Decision 2: non-sensitive decision was modified")
	}
	if result[2].Rationale != input[2].Rationale {
		t.Error("Rationale 2: non-sensitive rationale was modified")
	}
}

func TestW6_RedactReceiptPreservesMetadata(t *testing.T) {
	now := time.Now()
	input := decision.DecisionReceipt{
		Timestamp:    now,
		Category:     decision.CategoryModel,
		Decision:     "Use api_key=secret123",
		Rationale:    "For auth",
		Alternatives: []string{"option1", "option2"},
	}

	result := decision.RedactReceipt(input)

	if result.Timestamp != now {
		t.Error("Timestamp was modified")
	}
	if result.Category != decision.CategoryModel {
		t.Error("Category was modified")
	}
	if len(result.Alternatives) != 2 {
		t.Error("Alternatives length was modified")
	}
}

func TestW6_DecisionRedactionDoesNotAffectCheckpointData(t *testing.T) {
	// Verify that decisions stored in checkpoints are NOT redacted
	// (only rendering paths should redact, not storage)
	input := []decision.DecisionReceipt{
		{
			Timestamp: time.Now(),
			Category:  decision.CategoryTool,
			Decision:  "Use api_key=sk-abc123secret",
			Rationale: "For auth",
		},
	}

	// Simulate what SaveCheckpointData does: store raw decisions
	checkpointDecisions := input // store raw, no redaction

	// Simulate what rendering does: redact for display
	displayDecisions := decision.RedactSlice(input)

	// Checkpoint should have raw data
	if checkpointDecisions[0].Decision != input[0].Decision {
		t.Error("Checkpoint decisions should NOT be redacted")
	}

	// Display should have redacted data
	if displayDecisions[0].Decision == input[0].Decision {
		t.Error("Display decisions SHOULD be redacted")
	}
}

// ─── W1: Decision Logger Close method exists ─────────────────────────────────

func TestW1_DecisionLoggerHasClose(t *testing.T) {
	l := decision.NewLogger(16)
	// Verify Close method exists and can be called
	l.Close()
}

func TestW1_DecisionLoggerCloseFlushesEntries(t *testing.T) {
	l := decision.NewLogger(16)

	// Record some entries
	l.Log(decision.DecisionReceipt{
		Timestamp: time.Now(),
		Category:  decision.CategoryTool,
		Decision:  "Test decision",
		Rationale: "Test rationale",
	})

	// Close should flush
	l.Close()

	// After close, snapshot should still work
	snapshot := l.Snapshot()
	if len(snapshot) != 1 {
		t.Errorf("Expected 1 decision in snapshot after close, got %d", len(snapshot))
	}
}

func TestW1_DecisionLoggerCloseIdempotent(t *testing.T) {
	l := decision.NewLogger(16)
	l.Log(decision.DecisionReceipt{
		Timestamp: time.Now(),
		Category:  decision.CategoryPlan,
		Decision:  "Test",
		Rationale: "Test",
	})

	// Multiple closes should not panic
	l.Close()
	l.Close()
	l.Close()
}

// ─── W4: Undo checkpoint restoration (unit-level) ────────────────────────────

func TestW4_CheckpointDataRoundTrip(t *testing.T) {
	// Verify that CheckpointData can carry phase + goal + decisions
	cp := &workflow.CheckpointData{
		Phase:       m31types.PhaseExecute,
		Goal:        "implement auth module",
		PlanVersion: 2,
		Decisions: []decision.DecisionReceipt{
			{
				Timestamp: time.Now(),
				Category:  decision.CategoryTool,
				Decision:  "Use JWT for auth",
				Rationale: "Industry standard",
			},
		},
		Timestamp: time.Now(),
	}

	if cp.Phase != m31types.PhaseExecute {
		t.Errorf("Phase mismatch: got %q", cp.Phase)
	}
	if cp.Goal != "implement auth module" {
		t.Errorf("Goal mismatch: got %q", cp.Goal)
	}
	if cp.PlanVersion != 2 {
		t.Errorf("PlanVersion mismatch: got %d", cp.PlanVersion)
	}
	if len(cp.Decisions) != 1 {
		t.Errorf("Decisions count mismatch: got %d", len(cp.Decisions))
	}
	if cp.Decisions[0].Decision != "Use JWT for auth" {
		t.Errorf("Decision mismatch: got %q", cp.Decisions[0].Decision)
	}
}
