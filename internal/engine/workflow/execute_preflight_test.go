package workflow

import (
	"testing"

	m31types "github.com/eshanized/M31A/internal/core/types"
)

func TestRunExecutePreflight(t *testing.T) {
	dir := t.TempDir()
	e := &Engine{workDir: dir}

	tasks := []m31types.Task{
		{ID: 1, Action: "Create", Description: "Setup", Dependencies: []int{}, Files: []string{"setup.go"}, AcceptanceCriteria: []string{"build ok"}},
		{ID: 2, Action: "Modify", Description: "Update", Dependencies: []int{1}, Files: []string{"main.go"}, AcceptanceCriteria: []string{"tests pass"}},
		{ID: 3, Action: "Create", Description: "Bad deps", Dependencies: []int{99}, Files: []string{"bad.go"}, AcceptanceCriteria: []string{"ok"}},
		{ID: 4, Action: "Create", Description: "Empty task", Dependencies: []int{}, Files: []string{}, AcceptanceCriteria: []string{}},
	}

	result := e.runExecutePreflight(tasks)

	// Task 3 depends on non-existent task 99
	hasBadDep := false
	for _, issue := range result.Issues {
		if containsAny(issue, []string{"non-existent task 99"}) {
			hasBadDep = true
		}
	}
	if !hasBadDep {
		t.Error("expected preflight to detect bad dependency in task 3")
	}

	// Task 4 has no files and no criteria
	hasEmptyTask := false
	for _, issue := range result.Issues {
		if containsAny(issue, []string{"no files and no acceptance criteria"}) {
			hasEmptyTask = true
		}
	}
	if !hasEmptyTask {
		t.Error("expected preflight to detect empty task 4")
	}
}

func TestToolCallTracker(t *testing.T) {
	tracker := NewToolCallTracker(3)

	// Different tool calls — no loop
	if tracker.Record("Bash", "ls") {
		t.Error("should not detect loop after 1 call")
	}
	if tracker.Record("FileRead", "main.go") {
		t.Error("should not detect loop after different call")
	}

	// Same tool call 3 times — loop detected
	tracker.Record("Bash", "ls")
	tracker.Record("Bash", "ls")
	if !tracker.Record("Bash", "ls") {
		t.Error("should detect loop after 3 identical calls")
	}

	if tracker.LastLoopTool() != "Bash" {
		t.Errorf("LastLoopTool() = %q, want Bash", tracker.LastLoopTool())
	}

	if tracker.Count() != 5 {
		t.Errorf("Count() = %d, want 5", tracker.Count())
	}
}

func TestToolCallTrackerDifferentArgs(t *testing.T) {
	tracker := NewToolCallTracker(3)

	// Same tool but different args — no loop
	tracker.Record("Edit", "file1.go")
	tracker.Record("Edit", "file2.go")
	if tracker.Record("Edit", "file3.go") {
		t.Error("different args should not trigger loop detection")
	}
}

func TestMakeSignature(t *testing.T) {
	sig := makeSignature("Bash", `{"command": "ls -la"}`)
	if sig.Name != "Bash" {
		t.Errorf("Name = %q, want Bash", sig.Name)
	}

	// Long input should be truncated
	longInput := string(make([]byte, 300))
	sig = makeSignature("Test", longInput)
	if len(sig.Args) > 200 {
		t.Errorf("Args length = %d, should be <= 200", len(sig.Args))
	}
}
