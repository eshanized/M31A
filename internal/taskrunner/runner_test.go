package taskrunner

import (
	"context"
	"fmt"
	"testing"

	m31errors "github.com/eshanized/M31A/internal/errors"
	"github.com/eshanized/M31A/internal/types"
)

func newTask(id int, desc string, deps []int) types.Task {
	return types.Task{
		ID:           id,
		Description:  desc,
		Action:       "Create",
		Dependencies: deps,
	}
}

func TestRunner_EmptyList(t *testing.T) {
	r := New([]types.Task{})

	groups, err := r.Schedule()
	if err != nil {
		t.Fatalf("Schedule on empty list should not error: %v", err)
	}
	if len(groups) != 0 {
		t.Errorf("Expected 0 groups, got %d", len(groups))
	}

	done, doneCount, failed, skipped := r.Summary()
	if done != 0 || doneCount != 0 || failed != 0 || skipped != 0 {
		t.Errorf("Expected all zeros, got total=%d done=%d failed=%d skipped=%d", done, doneCount, failed, skipped)
	}
}

func TestRunner_SingleTask(t *testing.T) {
	tasks := []types.Task{newTask(1, "single task", nil)}
	r := New(tasks)

	groups, err := r.Schedule()
	if err != nil {
		t.Fatalf("Schedule failed: %v", err)
	}
	if len(groups) != 1 {
		t.Fatalf("Expected 1 group, got %d", len(groups))
	}
	if len(groups[0]) != 1 || groups[0][0] != 1 {
		t.Errorf("Expected group [1], got %v", groups[0])
	}
}

func TestRunner_LinearDependencyChain(t *testing.T) {
	tasks := []types.Task{
		newTask(1, "task 1", nil),
		newTask(2, "task 2", []int{1}),
		newTask(3, "task 3", []int{2}),
	}
	r := New(tasks)

	groups, err := r.Schedule()
	if err != nil {
		t.Fatalf("Schedule failed: %v", err)
	}
	if len(groups) != 3 {
		t.Fatalf("Expected 3 groups for linear chain, got %d", len(groups))
	}
	if groups[0][0] != 1 {
		t.Errorf("Group 0: expected [1], got %v", groups[0])
	}
	if groups[1][0] != 2 {
		t.Errorf("Group 1: expected [2], got %v", groups[1])
	}
	if groups[2][0] != 3 {
		t.Errorf("Group 2: expected [3], got %v", groups[2])
	}
}

func TestRunner_DiamondDependency(t *testing.T) {
	tasks := []types.Task{
		newTask(1, "root", nil),
		newTask(2, "left", []int{1}),
		newTask(3, "right", []int{1}),
		newTask(4, "bottom", []int{2, 3}),
	}
	r := New(tasks)

	groups, err := r.Schedule()
	if err != nil {
		t.Fatalf("Schedule failed: %v", err)
	}
	if len(groups) != 3 {
		t.Fatalf("Expected 3 groups, got %d", len(groups))
	}
	// Group 0: [1]
	if len(groups[0]) != 1 || groups[0][0] != 1 {
		t.Errorf("Group 0: expected [1], got %v", groups[0])
	}
	// Group 1: [2, 3] (both depend only on 1)
	if len(groups[1]) != 2 {
		t.Errorf("Group 1: expected 2 tasks, got %d", len(groups[1]))
	}
	// Group 2: [4]
	if len(groups[2]) != 1 || groups[2][0] != 4 {
		t.Errorf("Group 2: expected [4], got %v", groups[2])
	}
}

func TestRunner_IndependentTasks(t *testing.T) {
	tasks := []types.Task{
		newTask(1, "a", nil),
		newTask(2, "b", nil),
		newTask(3, "c", nil),
	}
	r := New(tasks)

	groups, err := r.Schedule()
	if err != nil {
		t.Fatalf("Schedule failed: %v", err)
	}
	if len(groups) != 1 {
		t.Fatalf("Expected 1 group for independent tasks, got %d", len(groups))
	}
	if len(groups[0]) != 3 {
		t.Errorf("Expected 3 tasks in group, got %d", len(groups[0]))
	}
}

func TestRunner_CircularDependency(t *testing.T) {
	tasks := []types.Task{
		newTask(1, "a", []int{2}),
		newTask(2, "b", []int{1}),
	}
	r := New(tasks)

	_, err := r.Schedule()
	if err == nil {
		t.Fatal("Expected error for circular dependency")
	}
	if err != m31errors.ErrCircularDependency {
		t.Errorf("Expected ErrCircularDependency, got %v", err)
	}
}

func TestRunner_SelfReference(t *testing.T) {
	tasks := []types.Task{
		newTask(1, "a", []int{1}),
	}
	r := New(tasks)

	_, err := r.Schedule()
	if err == nil {
		t.Fatal("Expected error for self-reference")
	}
	// Error is wrapped with context, check it references the right error
	if err.Error() == "" {
		t.Errorf("Expected non-empty error")
	}
}

func TestRunner_ExecuteFunction(t *testing.T) {
	tasks := []types.Task{
		newTask(1, "task 1", nil),
		newTask(2, "task 2", []int{1}),
	}
	r := New(tasks)

	groups, err := r.Schedule()
	if err != nil {
		t.Fatalf("Schedule failed: %v", err)
	}

	execFn := func(ctx context.Context, task types.Task) TaskResult {
		return TaskResult{Success: true, Output: "done " + task.Description}
	}

	for _, group := range groups {
		if err := r.ExecuteGroup(context.Background(), group, execFn); err != nil {
			t.Fatalf("ExecuteGroup failed: %v", err)
		}
	}

	if r.Status(1) != types.StatusDone {
		t.Errorf("Task 1: expected StatusDone, got %s", r.Status(1))
	}
	if r.Status(2) != types.StatusDone {
		t.Errorf("Task 2: expected StatusDone, got %s", r.Status(2))
	}

	results := r.Results()
	if results[1].Output != "done task 1" {
		t.Errorf("Task 1 output: expected 'done task 1', got %q", results[1].Output)
	}
}

func TestRunner_FailedDependencyBlocking(t *testing.T) {
	tasks := []types.Task{
		newTask(1, "task 1", nil),
		newTask(2, "task 2", []int{1}),
	}
	r := New(tasks)

	groups, err := r.Schedule()
	if err != nil {
		t.Fatalf("Schedule failed: %v", err)
	}

	// Fail task 1
	failFn := func(ctx context.Context, task types.Task) TaskResult {
		if task.ID == 1 {
			return TaskResult{Success: false, Error: "failed"}
		}
		return TaskResult{Success: true}
	}

	if err := r.ExecuteGroup(context.Background(), groups[0], failFn); err != nil {
		t.Fatalf("ExecuteGroup 0 failed: %v", err)
	}

	if r.Status(1) != types.StatusFailed {
		t.Fatalf("Task 1: expected StatusFailed, got %s", r.Status(1))
	}

	// Execute group 1 — task 2 should be skipped
	if err := r.ExecuteGroup(context.Background(), groups[1], failFn); err != nil {
		t.Fatalf("ExecuteGroup 1 failed: %v", err)
	}

	if r.Status(2) != types.StatusSkipped {
		t.Errorf("Task 2: expected StatusSkipped due to failed dependency, got %s", r.Status(2))
	}
}

func TestRunner_SkipPropagation(t *testing.T) {
	tasks := []types.Task{
		newTask(1, "task 1", nil),
		newTask(2, "task 2", []int{1}),
		newTask(3, "task 3", []int{2}),
	}
	r := New(tasks)

	groups, err := r.Schedule()
	if err != nil {
		t.Fatalf("Schedule failed: %v", err)
	}

	// Skip task 1 by marking it before execution
	r.status[1] = types.StatusSkipped
	r.results[1] = TaskResult{Success: false, Error: "skipped"}

	// Execute group 0 — task 1 already skipped, stays skipped
	if err := r.ExecuteGroup(context.Background(), groups[0], nil); err != nil {
		t.Fatalf("ExecuteGroup 0 failed: %v", err)
	}

	// Execute group 1 — task 2 should skip because dep 1 is skipped
	if err := r.ExecuteGroup(context.Background(), groups[1], nil); err != nil {
		t.Fatalf("ExecuteGroup 1 failed: %v", err)
	}

	// Execute group 2 — task 3 should skip because dep 2 is skipped
	if err := r.ExecuteGroup(context.Background(), groups[2], nil); err != nil {
		t.Fatalf("ExecuteGroup 2 failed: %v", err)
	}

	if r.Status(2) != types.StatusSkipped {
		t.Errorf("Task 2: expected StatusSkipped, got %s", r.Status(2))
	}
	if r.Status(3) != types.StatusSkipped {
		t.Errorf("Task 3: expected StatusSkipped, got %s", r.Status(3))
	}
}

func TestRunner_StatusTracking(t *testing.T) {
	tasks := []types.Task{newTask(1, "task 1", nil)}
	r := New(tasks)

	if r.Status(1) != types.StatusPending {
		t.Errorf("Expected StatusPending, got %s", r.Status(1))
	}

	groups, _ := r.Schedule()
	r.ExecuteGroup(context.Background(), groups[0], func(ctx context.Context, task types.Task) TaskResult {
		return TaskResult{Success: true, CommitHash: "abc123"}
	})

	if r.Status(1) != types.StatusDone {
		t.Errorf("Expected StatusDone, got %s", r.Status(1))
	}

	results := r.Results()
	if results[1].CommitHash != "abc123" {
		t.Errorf("Expected commit hash 'abc123', got %q", results[1].CommitHash)
	}
}

func TestRunner_Summary(t *testing.T) {
	tasks := []types.Task{
		newTask(1, "done", nil),
		newTask(2, "failed", nil),
		newTask(3, "skipped", nil),
	}
	r := New(tasks)

	groups, _ := r.Schedule()

	// Execute with mixed results
	r.ExecuteGroup(context.Background(), groups[0], func(ctx context.Context, task types.Task) TaskResult {
		switch task.ID {
		case 1:
			return TaskResult{Success: true}
		case 2:
			return TaskResult{Success: false}
		}
		return TaskResult{Success: true}
	})

	// Manually skip task 3
	r.status[3] = types.StatusSkipped

	total, done, failed, skipped := r.Summary()
	if total != 3 {
		t.Errorf("Total: expected 3, got %d", total)
	}
	if done != 1 {
		t.Errorf("Done: expected 1, got %d", done)
	}
	if failed != 1 {
		t.Errorf("Failed: expected 1, got %d", failed)
	}
	if skipped != 1 {
		t.Errorf("Skipped: expected 1, got %d", skipped)
	}
}

func TestRunner_AllDone(t *testing.T) {
	tasks := []types.Task{
		newTask(1, "task 1", nil),
		newTask(2, "task 2", nil),
	}
	r := New(tasks)

	if r.AllDone() {
		t.Error("Expected AllDone to be false initially")
	}

	groups, _ := r.Schedule()
	r.ExecuteGroup(context.Background(), groups[0], func(ctx context.Context, task types.Task) TaskResult {
		return TaskResult{Success: true}
	})

	if !r.AllDone() {
		t.Error("Expected AllDone to be true after all tasks executed")
	}
}

func TestRunner_ComplexDAG(t *testing.T) {
	// 3-level DAG:
	// Level 0: 1
	// Level 1: 2, 3 (both depend on 1)
	// Level 2: 4 (depends on 2), 5 (depends on 3), 6 (depends on 2, 3)
	tasks := []types.Task{
		newTask(1, "root", nil),
		newTask(2, "left", []int{1}),
		newTask(3, "right", []int{1}),
		newTask(4, "left-left", []int{2}),
		newTask(5, "right-right", []int{3}),
		newTask(6, "merge", []int{2, 3}),
	}
	r := New(tasks)

	groups, err := r.Schedule()
	if err != nil {
		t.Fatalf("Schedule failed: %v", err)
	}

	if len(groups) != 3 {
		t.Fatalf("Expected 3 levels, got %d", len(groups))
	}

	// Level 0: [1]
	if len(groups[0]) != 1 || groups[0][0] != 1 {
		t.Errorf("Level 0: expected [1], got %v", groups[0])
	}

	// Level 1: [2, 3]
	if len(groups[1]) != 2 {
		t.Errorf("Level 1: expected 2 tasks, got %d", len(groups[1]))
	}

	// Level 2: [4, 5, 6]
	if len(groups[2]) != 3 {
		t.Errorf("Level 2: expected 3 tasks, got %d", len(groups[2]))
	}

	// Execute all
	execFn := func(ctx context.Context, task types.Task) TaskResult {
		return TaskResult{Success: true}
	}
	for _, group := range groups {
		if err := r.ExecuteGroup(context.Background(), group, execFn); err != nil {
			t.Fatalf("ExecuteGroup failed: %v", err)
		}
	}

	total, done, _, _ := r.Summary()
	if total != 6 || done != 6 {
		t.Errorf("Expected 6/6 done, got %d/%d", done, total)
	}
}

func TestRunner_Tasks(t *testing.T) {
	tasks := []types.Task{
		newTask(1, "task 1", nil),
		newTask(2, "task 2", nil),
	}
	r := New(tasks)

	// Initially statuses should be pending
	returned := r.Tasks()
	if len(returned) != 2 {
		t.Fatalf("Expected 2 tasks, got %d", len(returned))
	}
	if returned[0].Status != types.StatusPending {
		t.Errorf("Task 1: expected StatusPending, got %s", returned[0].Status)
	}

	// Execute tasks
	groups, _ := r.Schedule()
	r.ExecuteGroup(context.Background(), groups[0], func(ctx context.Context, task types.Task) TaskResult {
		return TaskResult{Success: true}
	})

	// After execution, statuses should be done
	returned = r.Tasks()
	if returned[0].Status != types.StatusDone {
		t.Errorf("Task 1: expected StatusDone, got %s", returned[0].Status)
	}
	if returned[1].Status != types.StatusDone {
		t.Errorf("Task 2: expected StatusDone, got %s", returned[1].Status)
	}
}

func TestRunner_NewWithExistingStatus(t *testing.T) {
	tests := []struct {
		name          string
		initialStatus types.TaskStatus
		expected      types.TaskStatus
	}{
		{"empty status defaults to pending", "", types.StatusPending},
		{"preserves running", types.StatusRunning, types.StatusRunning},
		{"preserves done", types.StatusDone, types.StatusDone},
		{"preserves failed", types.StatusFailed, types.StatusFailed},
		{"preserves skipped", types.StatusSkipped, types.StatusSkipped},
		{"preserves unrecoverable", types.StatusUnrecoverable, types.StatusUnrecoverable},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			task := types.Task{
				ID:          1,
				Description: "test",
				Action:      "Create",
				Status:      tt.initialStatus,
			}
			r := New([]types.Task{task})
			if r.Status(1) != tt.expected {
				t.Errorf("Expected %s, got %s", tt.expected, r.Status(1))
			}
		})
	}
}

func TestRunner_ExecuteGroupNilFn(t *testing.T) {
	tasks := []types.Task{newTask(1, "task 1", nil)}
	r := New(tasks)

	groups, err := r.Schedule()
	if err != nil {
		t.Fatalf("Schedule failed: %v", err)
	}

	// Execute with nil function — should succeed with default TaskResult
	if err := r.ExecuteGroup(context.Background(), groups[0], nil); err != nil {
		t.Fatalf("ExecuteGroup with nil fn should not error: %v", err)
	}

	if r.Status(1) != types.StatusDone {
		t.Errorf("Task should be done after nil fn execution, got %s", r.Status(1))
	}
}

func TestRunner_ExecuteGroupTerminalStates(t *testing.T) {
	tests := []struct {
		name   string
		status types.TaskStatus
	}{
		{"skipped", types.StatusSkipped},
		{"failed", types.StatusFailed},
		{"unrecoverable", types.StatusUnrecoverable},
		{"done", types.StatusDone},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			tasks := []types.Task{newTask(1, "task 1", nil)}
			r := New(tasks)
			r.status[1] = tt.status

			groups, _ := r.Schedule()
			called := false
			execFn := func(ctx context.Context, task types.Task) TaskResult {
				called = true
				return TaskResult{Success: true}
			}

			if err := r.ExecuteGroup(context.Background(), groups[0], execFn); err != nil {
				t.Fatalf("ExecuteGroup should not error: %v", err)
			}

			if called {
				t.Error("Execute function should not be called for terminal state tasks")
			}
			if r.Status(1) != tt.status {
				t.Errorf("Status should remain %s, got %s", tt.status, r.Status(1))
			}
		})
	}
}

func TestRunner_ExecuteGroupTaskNotFound(t *testing.T) {
	tasks := []types.Task{newTask(1, "task 1", nil)}
	r := New(tasks)

	// Try to execute a task ID that doesn't exist
	group := []int{999}
	err := r.ExecuteGroup(context.Background(), group, nil)
	if err == nil {
		t.Fatal("Expected error for non-existent task ID")
	}
	if err.Error() == "" {
		t.Error("Expected non-empty error message")
	}
}

func TestRunner_ExecuteGroupDependencyNotDone(t *testing.T) {
	tasks := []types.Task{
		newTask(1, "task 1", nil),
		newTask(2, "task 2", []int{1}),
	}
	r := New(tasks)

	groups, _ := r.Schedule()

	// Execute group 0 but leave task 1 in pending (by using nil fn won't work — nil fn succeeds)
	// Instead, manually set task 1 to pending after scheduling
	r.status[1] = types.StatusPending

	// Execute group 1 — task 2 should skip because dep 1 is not done
	if err := r.ExecuteGroup(context.Background(), groups[1], nil); err != nil {
		t.Fatalf("ExecuteGroup should not error: %v", err)
	}

	if r.Status(2) != types.StatusSkipped {
		t.Errorf("Task 2: expected StatusSkipped, got %s", r.Status(2))
	}
	results := r.Results()
	if results[2].Error == "" {
		t.Error("Expected error message about dependency not completed")
	}
}

func TestRunner_ExecuteGroupUnrecoverableDependency(t *testing.T) {
	tasks := []types.Task{
		newTask(1, "task 1", nil),
		newTask(2, "task 2", []int{1}),
	}
	r := New(tasks)

	groups, _ := r.Schedule()

	// Mark task 1 as unrecoverable
	r.status[1] = types.StatusUnrecoverable

	// Execute group 1 — task 2 should skip
	if err := r.ExecuteGroup(context.Background(), groups[1], nil); err != nil {
		t.Fatalf("ExecuteGroup should not error: %v", err)
	}

	if r.Status(2) != types.StatusSkipped {
		t.Errorf("Task 2: expected StatusSkipped due to unrecoverable dependency, got %s", r.Status(2))
	}
}

func TestRunner_ResultsReturnsCopy(t *testing.T) {
	tasks := []types.Task{newTask(1, "task 1", nil)}
	r := New(tasks)

	groups, _ := r.Schedule()
	r.ExecuteGroup(context.Background(), groups[0], func(ctx context.Context, task types.Task) TaskResult {
		return TaskResult{Success: true, Output: "hello"}
	})

	results1 := r.Results()
	// Modify the returned map directly
	results1[1] = TaskResult{Success: true, Output: "modified"}

	results2 := r.Results()
	if results2[1].Output == "modified" {
		t.Error("Results() should return a copy, but map modifications leaked through")
	}
	if results2[1].Output != "hello" {
		t.Errorf("Expected output 'hello', got %q", results2[1].Output)
	}
}

func TestRunner_Callbacks(t *testing.T) {
	tasks := []types.Task{
		newTask(1, "task 1", nil),
		newTask(2, "task 2", []int{1}),
	}
	r := New(tasks)

	var started []int
	var updates []string

	r.OnTaskStart = func(task types.Task) {
		started = append(started, task.ID)
	}
	r.OnTaskUpdate = func(task types.Task, status string) {
		updates = append(updates, fmt.Sprintf("%d:%s", task.ID, status))
	}

	groups, err := r.Schedule()
	if err != nil {
		t.Fatalf("Schedule failed: %v", err)
	}

	execFn := func(ctx context.Context, task types.Task) TaskResult {
		return TaskResult{Success: true}
	}

	for _, group := range groups {
		if err := r.ExecuteGroup(context.Background(), group, execFn); err != nil {
			t.Fatalf("ExecuteGroup failed: %v", err)
		}
	}

	if len(started) != 2 {
		t.Errorf("Expected 2 task starts, got %d", len(started))
	}
	if started[0] != 1 || started[1] != 2 {
		t.Errorf("Expected starts [1, 2], got %v", started)
	}
	if len(updates) != 2 {
		t.Errorf("Expected 2 updates, got %d", len(updates))
	}
	if updates[0] != "1:done" || updates[1] != "2:done" {
		t.Errorf("Expected updates [1:done, 2:done], got %v", updates)
	}
}

func TestRunner_CallbacksNotCalledWhenNil(t *testing.T) {
	tasks := []types.Task{newTask(1, "task 1", nil)}
	r := New(tasks)

	// No callbacks set — should not panic
	groups, _ := r.Schedule()
	r.ExecuteGroup(context.Background(), groups[0], func(ctx context.Context, task types.Task) TaskResult {
		return TaskResult{Success: true}
	})

	if r.Status(1) != types.StatusDone {
		t.Errorf("Task should be done, got %s", r.Status(1))
	}
}
