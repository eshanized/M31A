package workflow

import (
	"testing"
)

func TestEngineMessages_TaskUpdateMsg(t *testing.T) {
	t.Parallel()
	msg := TaskUpdateMsg{
		Status: "running",
	}
	if msg.Status != "running" {
		t.Errorf("expected 'running', got %q", msg.Status)
	}
}

func TestEngineMessages_ToolStartMsg_Fields(t *testing.T) {
	t.Parallel()
	msg := ToolStartMsg{
		ToolName:    "Bash",
		Description: "running command",
	}
	if msg.ToolName != "Bash" {
		t.Errorf("expected 'Bash', got %q", msg.ToolName)
	}
	if msg.Description != "running command" {
		t.Errorf("expected 'running command', got %q", msg.Description)
	}
}

func TestEngineMessages_ToolCompleteMsg_Fields(t *testing.T) {
	t.Parallel()
	msg := ToolCompleteMsg{
		ToolName:   "Bash",
		Success:    true,
		DurationMs: 100,
		Error:      "",
	}
	if !msg.Success {
		t.Error("expected success to be true")
	}
	if msg.DurationMs != 100 {
		t.Errorf("expected 100, got %d", msg.DurationMs)
	}
}

func TestEngineMessages_SelfHealStartMsg_Fields(t *testing.T) {
	t.Parallel()
	msg := SelfHealStartMsg{
		TaskID:  1,
		Attempt: 2,
		Max:     3,
	}
	if msg.TaskID != 1 {
		t.Errorf("expected 1, got %d", msg.TaskID)
	}
	if msg.Attempt != 2 {
		t.Errorf("expected 2, got %d", msg.Attempt)
	}
}

func TestEngineMessages_SelfHealCompleteMsg_Fields(t *testing.T) {
	t.Parallel()
	msg := SelfHealCompleteMsg{
		TaskID:  1,
		Attempt: 2,
		Max:     3,
		Success: true,
	}
	if !msg.Success {
		t.Error("expected success to be true")
	}
}

func TestEngineMessages_PhaseTransitionMsgs(t *testing.T) {
	t.Parallel()
	start := PhaseTransitionStartMsg{
		From: "Initialize",
		To:   "Discuss",
	}
	if start.From != "Initialize" {
		t.Errorf("expected 'Initialize', got %q", start.From)
	}

	complete := PhaseTransitionCompleteMsg{
		From:    "Initialize",
		To:      "Discuss",
		Success: true,
	}
	if !complete.Success {
		t.Error("expected success to be true")
	}
}

func TestEngineMessages_IntermediateProgressMsg_Fields(t *testing.T) {
	t.Parallel()
	msg := IntermediateProgressMsg{
		Phase:   "Execute",
		Message: "Running tasks...",
	}
	if msg.Phase != "Execute" {
		t.Errorf("expected 'Execute', got %q", msg.Phase)
	}
	if msg.Message != "Running tasks..." {
		t.Errorf("expected 'Running tasks...', got %q", msg.Message)
	}
}

func TestEngineMessages_ThinkingMsgs(t *testing.T) {
	t.Parallel()
	start := ThinkingStartMsg{Context: "Planning"}
	if start.Context != "Planning" {
		t.Errorf("expected 'Planning', got %q", start.Context)
	}

	complete := ThinkingCompleteMsg{Context: "Planning"}
	if complete.Context != "Planning" {
		t.Errorf("expected 'Planning', got %q", complete.Context)
	}
}

func TestPhaseResult_Fields(t *testing.T) {
	t.Parallel()
	pr := PhaseResult{
		Success:    true,
		Error:      "",
		DurationMs: 1000,
	}
	if !pr.Success {
		t.Error("expected success to be true")
	}
	if pr.DurationMs != 1000 {
		t.Errorf("expected 1000, got %d", pr.DurationMs)
	}
}

func TestDiffStats_Fields(t *testing.T) {
	t.Parallel()
	ds := DiffStats{
		FilesAdded:    3,
		FilesModified: 5,
		FilesDeleted:  1,
		Insertions:    100,
		Deletions:     50,
	}
	if ds.FilesAdded != 3 {
		t.Errorf("expected 3, got %d", ds.FilesAdded)
	}
	if ds.Insertions != 100 {
		t.Errorf("expected 100, got %d", ds.Insertions)
	}
}

func TestVerificationResult_AllFields(t *testing.T) {
	t.Parallel()
	vr := VerificationResult{
		TaskID:     1,
		FilesExist: true,
		SyntaxOK:   true,
		TestsOK:    false,
		Errors:     []string{"test failed"},
	}
	if !vr.FilesExist {
		t.Error("expected FilesExist to be true")
	}
	if len(vr.Errors) != 1 {
		t.Errorf("expected 1 error, got %d", len(vr.Errors))
	}
}

func TestDiscussState_Fields(t *testing.T) {
	t.Parallel()
	ds := DiscussState{
		Questions: []string{"Q1", "Q2"},
		Answers:   map[int]string{0: "A1", 1: "A2"},
	}
	if len(ds.Questions) != 2 {
		t.Errorf("expected 2 questions, got %d", len(ds.Questions))
	}
	if ds.Answers[0] != "A1" {
		t.Errorf("expected 'A1', got %q", ds.Answers[0])
	}
}

func TestDemonstrationReadyMsg_Fields(t *testing.T) {
	t.Parallel()
	msg := DemonstrationReadyMsg{Content: "demo content"}
	if msg.Content != "demo content" {
		t.Errorf("expected 'demo content', got %q", msg.Content)
	}
}
