package workflow

import (
	"testing"
)

func TestParsePlan_TasksFromJSON(t *testing.T) {
	t.Parallel()
	md := `# Plan

## Task List

` + "```json" + `
[{"id":1,"action":"Create","description":"test task","dependencies":[],"files":["test.go"],"acceptance_criteria":[]}]
` + "```"
	tasks, err := extractTasksFromPlan(md)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if len(tasks) != 1 {
		t.Fatalf("expected 1 task, got %d", len(tasks))
	}
	if tasks[0].Description != "test task" {
		t.Errorf("expected 'test task', got %q", tasks[0].Description)
	}
}

func TestParsePlan_RawMarkdownPreserved(t *testing.T) {
	t.Parallel()
	md := "# Plan\n\nSome content here"
	plan, err := ParsePlan(md)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if plan.RawMarkdown != md {
		t.Error("expected raw markdown to be preserved")
	}
}
