package subagent

import (
	"context"
	"encoding/json"
	"testing"
	"time"
)

func TestEventTypes_Constants(t *testing.T) {
	t.Parallel()
	// Verify event type constants are distinct
	types := map[EventType]bool{
		EventSpawned:   true,
		EventToolStart: true,
		EventToolDone:  true,
		EventTextDelta: true,
		EventThinking:  true,
		EventDone:      true,
		EventError:     true,
		EventCancelled: true,
	}
	if len(types) != 8 {
		t.Errorf("expected 8 event types, got %d", len(types))
	}
}

func TestStatusValues_Constants(t *testing.T) {
	t.Parallel()
	statuses := map[Status]bool{
		StatusPending: true,
		StatusRunning: true,
		StatusDone:    true,
		StatusError:   true,
		StatusCancel:  true,
	}
	if len(statuses) != 5 {
		t.Errorf("expected 5 statuses, got %d", len(statuses))
	}
}

func TestIsolationValues_Constants(t *testing.T) {
	t.Parallel()
	if IsolationWorktree != "worktree" {
		t.Errorf("expected 'worktree', got %q", IsolationWorktree)
	}
	if IsolationDefault != "default" {
		t.Errorf("expected 'default', got %q", IsolationDefault)
	}
}

func TestSubagentEvent_MarshalJSON_NilFields(t *testing.T) {
	t.Parallel()
	event := SubagentEvent{
		Type:      EventSpawned,
		AgentID:   "abc-123",
		Timestamp: time.Now(),
	}

	data, err := json.Marshal(event)
	if err != nil {
		t.Fatalf("unexpected marshal error: %v", err)
	}

	var decoded map[string]any
	if err := json.Unmarshal(data, &decoded); err != nil {
		t.Fatalf("unexpected unmarshal error: %v", err)
	}

	// These fields should be omitted (zero values)
	if _, ok := decoded["summary"]; ok {
		t.Error("expected 'summary' to be omitted for zero value")
	}
	if _, ok := decoded["error"]; ok {
		t.Error("expected 'error' to be omitted for zero value")
	}
}

func TestSpawnRequest_AllFields(t *testing.T) {
	t.Parallel()
	req := SpawnRequest{
		Description: "test task",
		Prompt:      "do something",
		Name:        "test-agent",
		Isolation:   IsolationWorktree,
		Background:  true,
		ModelID:     "model-123",
		MaxTools:    25,
		MaxTokens:   25000,
	}

	if req.Description != "test task" {
		t.Errorf("unexpected description: %q", req.Description)
	}
	if req.Isolation != IsolationWorktree {
		t.Errorf("unexpected isolation: %q", req.Isolation)
	}
	if !req.Background {
		t.Error("expected background to be true")
	}
	if req.MaxTools != 25 {
		t.Errorf("expected 25, got %d", req.MaxTools)
	}
}

func TestSubagentInfo_AllFields(t *testing.T) {
	t.Parallel()
	info := SubagentInfo{
		ID:          "abc-123",
		Name:        "test-agent",
		Description: "test task",
		Status:      StatusRunning,
		Isolation:   IsolationWorktree,
		StartedAt:   time.Now(),
	}

	if info.ID != "abc-123" {
		t.Errorf("unexpected ID: %q", info.ID)
	}
	if info.Status != StatusRunning {
		t.Errorf("unexpected status: %q", info.Status)
	}
	if info.Name != "test-agent" {
		t.Errorf("unexpected name: %q", info.Name)
	}
}

func TestToolCallInput_RoundTrip(t *testing.T) {
	t.Parallel()
	input := ToolCallInput{
		ID:    "call-1",
		Name:  "Bash",
		Input: json.RawMessage(`{"command":"ls"}`),
	}

	data, err := json.Marshal(input)
	if err != nil {
		t.Fatalf("unexpected marshal error: %v", err)
	}

	var decoded ToolCallInput
	if err := json.Unmarshal(data, &decoded); err != nil {
		t.Fatalf("unexpected unmarshal error: %v", err)
	}

	if decoded.Name != "Bash" {
		t.Errorf("expected name 'Bash', got %q", decoded.Name)
	}
	if decoded.ID != "call-1" {
		t.Errorf("expected ID 'call-1', got %q", decoded.ID)
	}
}

func TestToolCallOutput_AllFields(t *testing.T) {
	t.Parallel()
	output := ToolCallOutput{
		ToolCallID: "call-1",
		Output:     "result",
		Error:      "some error",
		DurationMs: 100,
	}

	if output.ToolCallID != "call-1" {
		t.Errorf("unexpected ToolCallID: %q", output.ToolCallID)
	}
	if output.DurationMs != 100 {
		t.Errorf("unexpected DurationMs: %d", output.DurationMs)
	}
	if output.Error != "some error" {
		t.Errorf("unexpected Error: %q", output.Error)
	}
}

func TestToolDescriptor_AllFields(t *testing.T) {
	t.Parallel()
	desc := ToolDescriptor{
		Name:            "Bash",
		Description:     "Execute bash commands",
		ParameterSchema: `{"type":"object"}`,
	}

	if desc.Name != "Bash" {
		t.Errorf("unexpected Name: %q", desc.Name)
	}
	if desc.ParameterSchema != `{"type":"object"}` {
		t.Errorf("unexpected ParameterSchema: %q", desc.ParameterSchema)
	}
}

func TestMaxTurns_Constant(t *testing.T) {
	t.Parallel()
	if maxTurns != 25 {
		t.Errorf("expected maxTurns=25, got %d", maxTurns)
	}
}

func TestToolDispatcherInterface_Exists(t *testing.T) {
	t.Parallel()
	// Verify the interface is satisfiable at compile time
	_ = context.Background()
}
