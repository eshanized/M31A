package types

import (
	"testing"
)

func TestPlan_Fields(t *testing.T) {
	t.Parallel()
	plan := Plan{
		Title:   "Test Plan",
		Summary: "A test plan",
		Version: 1,
		Tasks: []Task{
			{ID: 1, Description: "Task 1"},
		},
	}
	if plan.Title != "Test Plan" {
		t.Errorf("expected 'Test Plan', got %q", plan.Title)
	}
	if plan.Version != 1 {
		t.Errorf("expected version 1, got %d", plan.Version)
	}
	if len(plan.Tasks) != 1 {
		t.Errorf("expected 1 task, got %d", len(plan.Tasks))
	}
}

func TestReviewNote_Fields(t *testing.T) {
	t.Parallel()
	rn := ReviewNote{
		Level: "IMPORTANT",
		Text:  "Review this carefully",
	}
	if rn.Level != "IMPORTANT" {
		t.Errorf("expected 'IMPORTANT', got %q", rn.Level)
	}
}

func TestOpenQuestion_Fields(t *testing.T) {
	t.Parallel()
	oq := OpenQuestion{
		Question:   "What framework?",
		Suggestion: "Bubble Tea",
	}
	if oq.Question != "What framework?" {
		t.Errorf("expected question, got %q", oq.Question)
	}
	if oq.Suggestion != "Bubble Tea" {
		t.Errorf("expected suggestion, got %q", oq.Suggestion)
	}
}

func TestProposedChange_Fields(t *testing.T) {
	t.Parallel()
	pc := ProposedChange{
		Action:      "NEW",
		File:        "main.go",
		Description: "Entry point",
	}
	if pc.Action != "NEW" {
		t.Errorf("expected 'NEW', got %q", pc.Action)
	}
}

func TestVerificationPlan_Fields(t *testing.T) {
	t.Parallel()
	vp := VerificationPlan{
		Automated: []string{"go test ./..."},
		Manual:    []string{"test in browser"},
	}
	if len(vp.Automated) != 1 {
		t.Errorf("expected 1 automated step, got %d", len(vp.Automated))
	}
}
