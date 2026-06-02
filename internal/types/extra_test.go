package types

import (
	"context"
	"encoding/json"
	"testing"
)

// ---------------------------------------------------------------------------
// Task.ClampHealsAttempted tests
// ---------------------------------------------------------------------------

func TestTask_ClampHealsAttempted_UnderLimit(t *testing.T) {
	task := &Task{HealsAttempted: 1}
	task.ClampHealsAttempted()
	if task.HealsAttempted != 1 {
		t.Errorf("expected 1, got %d", task.HealsAttempted)
	}
}

func TestTask_ClampHealsAttempted_AtLimit(t *testing.T) {
	task := &Task{HealsAttempted: MaxHealAttempts}
	task.ClampHealsAttempted()
	if task.HealsAttempted != MaxHealAttempts {
		t.Errorf("expected %d, got %d", MaxHealAttempts, task.HealsAttempted)
	}
}

func TestTask_ClampHealsAttempted_OverLimit(t *testing.T) {
	task := &Task{HealsAttempted: 10}
	task.ClampHealsAttempted()
	if task.HealsAttempted != MaxHealAttempts {
		t.Errorf("expected %d, got %d", MaxHealAttempts, task.HealsAttempted)
	}
}

func TestTask_ClampHealsAttempted_Zero(t *testing.T) {
	task := &Task{HealsAttempted: 0}
	task.ClampHealsAttempted()
	if task.HealsAttempted != 0 {
		t.Errorf("expected 0, got %d", task.HealsAttempted)
	}
}

// ---------------------------------------------------------------------------
// Message.MarshalJSON tests
// ---------------------------------------------------------------------------

func TestMessage_MarshalJSON_NilSegments(t *testing.T) {
	msg := Message{
		Role:    "user",
		Content: "hello",
	}

	data, err := json.Marshal(msg)
	if err != nil {
		t.Fatalf("MarshalJSON failed: %v", err)
	}

	var result map[string]interface{}
	if err := json.Unmarshal(data, &result); err != nil {
		t.Fatalf("Unmarshal failed: %v", err)
	}

	segments, ok := result["segments"]
	if !ok {
		t.Fatal("expected 'segments' key in JSON")
	}
	// Should be [] not null
	segSlice, ok := segments.([]interface{})
	if !ok {
		t.Fatalf("expected segments to be array, got %T", segments)
	}
	if len(segSlice) != 0 {
		t.Errorf("expected empty segments array, got %d elements", len(segSlice))
	}
}

func TestMessage_MarshalJSON_EmptySegments(t *testing.T) {
	msg := Message{
		Role:     "user",
		Content:  "hello",
		Segments: []MessageSegment{},
	}

	data, err := json.Marshal(msg)
	if err != nil {
		t.Fatalf("MarshalJSON failed: %v", err)
	}

	var result map[string]interface{}
	if err := json.Unmarshal(data, &result); err != nil {
		t.Fatalf("Unmarshal failed: %v", err)
	}

	segSlice, ok := result["segments"].([]interface{})
	if !ok {
		t.Fatalf("expected segments to be array, got %T", result["segments"])
	}
	if len(segSlice) != 0 {
		t.Errorf("expected empty segments, got %d", len(segSlice))
	}
}

func TestMessage_MarshalJSON_WithSegments(t *testing.T) {
	msg := Message{
		Role:    "system",
		Content: "summary",
		Segments: []MessageSegment{
			{Type: "memory", Content: "mem", Visible: false},
		},
	}

	data, err := json.Marshal(msg)
	if err != nil {
		t.Fatalf("MarshalJSON failed: %v", err)
	}

	var result map[string]interface{}
	if err := json.Unmarshal(data, &result); err != nil {
		t.Fatalf("Unmarshal failed: %v", err)
	}

	segSlice := result["segments"].([]interface{})
	if len(segSlice) != 1 {
		t.Errorf("expected 1 segment, got %d", len(segSlice))
	}
}

// ---------------------------------------------------------------------------
// Message round-trip JSON test
// ---------------------------------------------------------------------------

func TestMessage_JSON_RoundTrip(t *testing.T) {
	original := Message{
		Role:    "assistant",
		Content: "test response",
		Segments: []MessageSegment{
			{Type: "text", Content: "text content", DurationMs: 100, Visible: true},
			{Type: "memory", Content: "mem content", Visible: false},
		},
		ToolCalls: []ToolCall{
			{ID: "call1", Name: "read_file", Input: json.RawMessage(`{"path":"/tmp/test"}`)},
		},
		ToolCallID: "tc_123",
	}

	data, err := json.Marshal(original)
	if err != nil {
		t.Fatalf("Marshal failed: %v", err)
	}

	var decoded Message
	if err := json.Unmarshal(data, &decoded); err != nil {
		t.Fatalf("Unmarshal failed: %v", err)
	}

	if decoded.Role != original.Role {
		t.Errorf("Role mismatch: %q vs %q", decoded.Role, original.Role)
	}
	if decoded.Content != original.Content {
		t.Errorf("Content mismatch: %q vs %q", decoded.Content, original.Content)
	}
	if len(decoded.Segments) != 2 {
		t.Errorf("Segments mismatch: %d vs 2", len(decoded.Segments))
	}
	if decoded.ToolCalls[0].Name != "read_file" {
		t.Errorf("ToolCall name mismatch: %q", decoded.ToolCalls[0].Name)
	}
}

// ---------------------------------------------------------------------------
// RiskLevel tests
// ---------------------------------------------------------------------------

func TestRiskLevel_Values(t *testing.T) {
	tests := []struct {
		level    RiskLevel
		expected string
	}{
		{RiskSafe, "safe"},
		{RiskMedium, "medium"},
		{RiskDangerous, "dangerous"},
		{RiskDestructive, "destructive"},
	}
	for _, tt := range tests {
		if string(tt.level) != tt.expected {
			t.Errorf("RiskLevel = %q, want %q", string(tt.level), tt.expected)
		}
	}
}

// ---------------------------------------------------------------------------
// WorkflowPhase tests
// ---------------------------------------------------------------------------

func TestWorkflowPhase_Values(t *testing.T) {
	phases := []WorkflowPhase{
		PhaseIdle, PhaseInitialize, PhaseDiscuss,
		PhasePlan, PhaseExecute, PhaseVerify, PhaseShip,
	}
	seen := make(map[WorkflowPhase]bool)
	for _, p := range phases {
		if seen[p] {
			t.Errorf("duplicate WorkflowPhase: %q", p)
		}
		seen[p] = true
		if string(p) == "" {
			t.Error("WorkflowPhase should not be empty")
		}
	}
}

// ---------------------------------------------------------------------------
// TaskStatus tests
// ---------------------------------------------------------------------------

func TestTaskStatus_Values(t *testing.T) {
	statuses := []TaskStatus{
		StatusPending, StatusRunning, StatusDone,
		StatusFailed, StatusSkipped, StatusUnrecoverable,
	}
	seen := make(map[TaskStatus]bool)
	for _, s := range statuses {
		if seen[s] {
			t.Errorf("duplicate TaskStatus: %q", s)
		}
		seen[s] = true
		if string(s) == "" {
			t.Error("TaskStatus should not be empty")
		}
	}
}

// ---------------------------------------------------------------------------
// Usage struct tests
// ---------------------------------------------------------------------------

func TestUsage_JSON(t *testing.T) {
	u := Usage{
		PromptTokens:     100,
		CompletionTokens: 50,
		TotalTokens:      150,
	}
	data, err := json.Marshal(u)
	if err != nil {
		t.Fatalf("Marshal failed: %v", err)
	}

	var decoded Usage
	if err := json.Unmarshal(data, &decoded); err != nil {
		t.Fatalf("Unmarshal failed: %v", err)
	}
	if decoded.TotalTokens != 150 {
		t.Errorf("expected 150, got %d", decoded.TotalTokens)
	}
}

// ---------------------------------------------------------------------------
// CapFlags struct tests
// ---------------------------------------------------------------------------

func TestCapFlags_JSON(t *testing.T) {
	c := CapFlags{Tools: true, Reasoning: false, Vision: true}
	data, err := json.Marshal(c)
	if err != nil {
		t.Fatalf("Marshal failed: %v", err)
	}

	var decoded CapFlags
	if err := json.Unmarshal(data, &decoded); err != nil {
		t.Fatalf("Unmarshal failed: %v", err)
	}
	if !decoded.Tools {
		t.Error("expected Tools=true")
	}
	if decoded.Reasoning {
		t.Error("expected Reasoning=false")
	}
}

// ---------------------------------------------------------------------------
// Pricing struct tests
// ---------------------------------------------------------------------------

func TestPricing_JSON(t *testing.T) {
	p := Pricing{InputPerMToken: 0.5, OutputPerMToken: 1.5}
	data, err := json.Marshal(p)
	if err != nil {
		t.Fatalf("Marshal failed: %v", err)
	}

	var decoded Pricing
	if err := json.Unmarshal(data, &decoded); err != nil {
		t.Fatalf("Unmarshal failed: %v", err)
	}
	if decoded.InputPerMToken != 0.5 {
		t.Errorf("expected 0.5, got %f", decoded.InputPerMToken)
	}
}

// ---------------------------------------------------------------------------
// ModelInfo struct tests
// ---------------------------------------------------------------------------

func TestModelInfo_JSON(t *testing.T) {
	variant := "thinking"
	m := ModelInfo{
		ID:            "test-model",
		Provider:      "test-provider",
		Name:          "Test Model",
		Description:   "A test model",
		ContextLength: 128000,
		Pricing:       Pricing{InputPerMToken: 0.5, OutputPerMToken: 1.5},
		Architecture:  ArchInfo{TokenizerFamily: "cl100k"},
		TopProvider:   "test",
		Capabilities:  CapFlags{Tools: true},
		Variant:       &variant,
	}
	data, err := json.Marshal(m)
	if err != nil {
		t.Fatalf("Marshal failed: %v", err)
	}

	var decoded ModelInfo
	if err := json.Unmarshal(data, &decoded); err != nil {
		t.Fatalf("Unmarshal failed: %v", err)
	}
	if decoded.Variant == nil || *decoded.Variant != "thinking" {
		t.Error("expected Variant='thinking'")
	}
}

func TestModelInfo_JSON_NilVariant(t *testing.T) {
	m := ModelInfo{
		ID:       "test",
		Provider: "test",
	}
	data, err := json.Marshal(m)
	if err != nil {
		t.Fatalf("Marshal failed: %v", err)
	}

	// variant should be omitted
	if string(data) == "" {
		t.Fatal("unexpected empty JSON")
	}
}

// ---------------------------------------------------------------------------
// ToolInput / ToolResult tests
// ---------------------------------------------------------------------------

func TestToolInput_JSON(t *testing.T) {
	ti := ToolInput{
		Name:   "read_file",
		Params: map[string]any{"path": "/tmp/test"},
	}
	data, err := json.Marshal(ti)
	if err != nil {
		t.Fatalf("Marshal failed: %v", err)
	}

	var decoded ToolInput
	if err := json.Unmarshal(data, &decoded); err != nil {
		t.Fatalf("Unmarshal failed: %v", err)
	}
	if decoded.Name != "read_file" {
		t.Errorf("expected 'read_file', got %q", decoded.Name)
	}
}

func TestToolResult_JSON(t *testing.T) {
	tr := ToolResult{
		ToolCallID: "tc_123",
		Output:     "file content",
		Error:      "",
		DurationMs: 100,
		Truncated:  false,
	}
	data, err := json.Marshal(tr)
	if err != nil {
		t.Fatalf("Marshal failed: %v", err)
	}

	var decoded ToolResult
	if err := json.Unmarshal(data, &decoded); err != nil {
		t.Fatalf("Unmarshal failed: %v", err)
	}
	if decoded.ToolCallID != "tc_123" {
		t.Errorf("expected 'tc_123', got %q", decoded.ToolCallID)
	}
}

// ---------------------------------------------------------------------------
// StreamChunk tests
// ---------------------------------------------------------------------------

func TestStreamChunk_JSON(t *testing.T) {
	sc := StreamChunk{
		Type:             "delta",
		Delta:            "Hello",
		ThinkingDuration: 50,
		ToolCallID:       "tc_1",
		ToolName:         "read_file",
		ToolInput:        `{}`,
		Index:            0,
	}
	data, err := json.Marshal(sc)
	if err != nil {
		t.Fatalf("Marshal failed: %v", err)
	}

	var decoded StreamChunk
	if err := json.Unmarshal(data, &decoded); err != nil {
		t.Fatalf("Unmarshal failed: %v", err)
	}
	if decoded.Delta != "Hello" {
		t.Errorf("expected 'Hello', got %q", decoded.Delta)
	}
}

// ---------------------------------------------------------------------------
// ProjectState tests
// ---------------------------------------------------------------------------

func TestProjectState_JSON(t *testing.T) {
	ps := ProjectState{
		Goal:        "build a web app",
		ProjectType: "node",
		Framework:   "express",
		Answers:     map[string]string{"key": "value"},
	}
	data, err := json.Marshal(ps)
	if err != nil {
		t.Fatalf("Marshal failed: %v", err)
	}

	var decoded ProjectState
	if err := json.Unmarshal(data, &decoded); err != nil {
		t.Fatalf("Unmarshal failed: %v", err)
	}
	if decoded.Goal != "build a web app" {
		t.Errorf("expected goal, got %q", decoded.Goal)
	}
}

// ---------------------------------------------------------------------------
// Session tests
// ---------------------------------------------------------------------------

func TestSession_JSON(t *testing.T) {
	s := Session{
		ID:            "sess001",
		ParentID:      "parent001",
		ChildrenIDs:   []string{"child1", "child2"},
		Label:         "test session",
		Tags:          []string{"test", "dev"},
		Model:         "m1",
		Provider:      "p1",
		MessageCount:  10,
		WorkflowPhase: PhaseExecute,
	}
	data, err := json.Marshal(s)
	if err != nil {
		t.Fatalf("Marshal failed: %v", err)
	}

	var decoded Session
	if err := json.Unmarshal(data, &decoded); err != nil {
		t.Fatalf("Unmarshal failed: %v", err)
	}
	if decoded.ID != "sess001" {
		t.Errorf("expected sess001, got %q", decoded.ID)
	}
	if decoded.ParentID != "parent001" {
		t.Errorf("expected parent001, got %q", decoded.ParentID)
	}
	if len(decoded.ChildrenIDs) != 2 {
		t.Errorf("expected 2 children, got %d", len(decoded.ChildrenIDs))
	}
}

func TestSession_NilProject(t *testing.T) {
	s := Session{ID: "sess002"}
	data, err := json.Marshal(s)
	if err != nil {
		t.Fatalf("Marshal failed: %v", err)
	}

	var decoded Session
	if err := json.Unmarshal(data, &decoded); err != nil {
		t.Fatalf("Unmarshal failed: %v", err)
	}
	if decoded.Project != nil {
		t.Error("expected nil Project")
	}
}

// ---------------------------------------------------------------------------
// HealthStatus tests
// ---------------------------------------------------------------------------

func TestHealthStatus_JSON(t *testing.T) {
	h := HealthStatus{
		Status:    HealthStatusLive,
		LatencyMs: 100,
		Error:     "",
	}
	data, err := json.Marshal(h)
	if err != nil {
		t.Fatalf("Marshal failed: %v", err)
	}

	var decoded HealthStatus
	if err := json.Unmarshal(data, &decoded); err != nil {
		t.Fatalf("Unmarshal failed: %v", err)
	}
	if decoded.Status != HealthStatusLive {
		t.Errorf("expected 'live', got %q", decoded.Status)
	}
}

// ---------------------------------------------------------------------------
// Constants sanity checks
// ---------------------------------------------------------------------------

func TestConstants_Values(t *testing.T) {
	if MaxHealAttempts != 2 {
		t.Errorf("MaxHealAttempts = %d, want 2", MaxHealAttempts)
	}
	if MaxPlanRetries != 3 {
		t.Errorf("MaxPlanRetries = %d, want 3", MaxPlanRetries)
	}
	if SessionIDLength != 8 {
		t.Errorf("SessionIDLength = %d, want 8", SessionIDLength)
	}
	if AutoDreamThreshold != 0.60 {
		t.Errorf("AutoDreamThreshold = %f, want 0.60", AutoDreamThreshold)
	}
	if MaxToolOutputChars != 10_000 {
		t.Errorf("MaxToolOutputChars = %d, want 10000", MaxToolOutputChars)
	}
}

func TestHealthStatus_Constants(t *testing.T) {
	statuses := []string{
		HealthStatusLive, HealthStatusSlow,
		HealthStatusOffline, HealthStatusDegraded,
	}
	for _, s := range statuses {
		if s == "" {
			t.Error("HealthStatus constant should not be empty")
		}
	}
}

func TestFileAction_Constants(t *testing.T) {
	if FileActionCreate != "create" {
		t.Errorf("FileActionCreate = %q", FileActionCreate)
	}
	if FileActionModify != "modify" {
		t.Errorf("FileActionModify = %q", FileActionModify)
	}
	if FileActionDelete != "delete" {
		t.Errorf("FileActionDelete = %q", FileActionDelete)
	}
}

// ---------------------------------------------------------------------------
// StreamIterator tests
// ---------------------------------------------------------------------------

func TestStreamIterator_Fields(t *testing.T) {
	called := false
	si := StreamIterator{
		Next: func() (*StreamChunk, error) {
			called = true
			return nil, nil
		},
		Close: func() error {
			return nil
		},
	}

	_, err := si.Next()
	if err != nil {
		t.Fatalf("Next failed: %v", err)
	}
	if !called {
		t.Error("Next function not called")
	}

	if err := si.Close(); err != nil {
		t.Fatalf("Close failed: %v", err)
	}
}

// ---------------------------------------------------------------------------
// Tool interface test
// ---------------------------------------------------------------------------

type mockTool struct {
	name        string
	description string
	risk        RiskLevel
}

func (m *mockTool) Name() string         { return m.name }
func (m *mockTool) Description() string  { return m.description }
func (m *mockTool) RiskLevel() RiskLevel { return m.risk }
func (m *mockTool) Execute(_ context.Context, _ ToolInput) (ToolResult, error) {
	return ToolResult{}, nil
}

func TestTool_InterfaceCompliance(t *testing.T) {
	var _ Tool = (*mockTool)(nil)
}

// ---------------------------------------------------------------------------
// MessageSegment tests
// ---------------------------------------------------------------------------

func TestMessageSegment_JSON(t *testing.T) {
	ms := MessageSegment{
		Type:       "text",
		Content:    "hello",
		DurationMs: 50,
		Visible:    true,
	}
	data, err := json.Marshal(ms)
	if err != nil {
		t.Fatalf("Marshal failed: %v", err)
	}

	var decoded MessageSegment
	if err := json.Unmarshal(data, &decoded); err != nil {
		t.Fatalf("Unmarshal failed: %v", err)
	}
	if decoded.Content != "hello" {
		t.Errorf("expected 'hello', got %q", decoded.Content)
	}
}

// ---------------------------------------------------------------------------
// Plan struct tests
// ---------------------------------------------------------------------------

func TestPlan_JSON(t *testing.T) {
	p := Plan{
		Title:   "Test Plan",
		Summary: "A test plan",
		Tasks: []Task{
			{ID: 1, Description: "task 1"},
		},
		Version:     1,
		RawMarkdown: "# Plan\n\nContent here",
	}
	data, err := json.Marshal(p)
	if err != nil {
		t.Fatalf("Marshal failed: %v", err)
	}

	var decoded Plan
	if err := json.Unmarshal(data, &decoded); err != nil {
		t.Fatalf("Unmarshal failed: %v", err)
	}
	if decoded.Title != "Test Plan" {
		t.Errorf("expected 'Test Plan', got %q", decoded.Title)
	}
	if len(decoded.Tasks) != 1 {
		t.Errorf("expected 1 task, got %d", len(decoded.Tasks))
	}
}

// ---------------------------------------------------------------------------
// ReviewNote, OpenQuestion, ProposedChangeGroup tests
// ---------------------------------------------------------------------------

func TestReviewNote_JSON(t *testing.T) {
	rn := ReviewNote{Level: "IMPORTANT", Text: "check this"}
	data, err := json.Marshal(rn)
	if err != nil {
		t.Fatalf("Marshal failed: %v", err)
	}
	var decoded ReviewNote
	if err := json.Unmarshal(data, &decoded); err != nil {
		t.Fatalf("Unmarshal failed: %v", err)
	}
	if decoded.Text != "check this" {
		t.Errorf("expected 'check this', got %q", decoded.Text)
	}
}

func TestOpenQuestion_JSON(t *testing.T) {
	oq := OpenQuestion{Question: "What framework?", Suggestion: "React"}
	data, err := json.Marshal(oq)
	if err != nil {
		t.Fatalf("Marshal failed: %v", err)
	}
	var decoded OpenQuestion
	if err := json.Unmarshal(data, &decoded); err != nil {
		t.Fatalf("Unmarshal failed: %v", err)
	}
	if decoded.Question != "What framework?" {
		t.Errorf("expected question, got %q", decoded.Question)
	}
}

func TestProposedChangeGroup_JSON(t *testing.T) {
	pcg := ProposedChangeGroup{
		Category: "Setup",
		Changes: []ProposedChange{
			{Action: "NEW", File: "new.go", Description: "create file"},
			{Action: "MODIFY", File: "old.go", Description: "modify file"},
		},
	}
	data, err := json.Marshal(pcg)
	if err != nil {
		t.Fatalf("Marshal failed: %v", err)
	}
	var decoded ProposedChangeGroup
	if err := json.Unmarshal(data, &decoded); err != nil {
		t.Fatalf("Unmarshal failed: %v", err)
	}
	if decoded.Category != "Setup" {
		t.Errorf("expected 'Setup', got %q", decoded.Category)
	}
	if len(decoded.Changes) != 2 {
		t.Errorf("expected 2 changes, got %d", len(decoded.Changes))
	}
}

func TestVerificationPlan_JSON(t *testing.T) {
	vp := VerificationPlan{
		Automated: []string{"unit tests", "integration tests"},
		Manual:    []string{"manual QA"},
	}
	data, err := json.Marshal(vp)
	if err != nil {
		t.Fatalf("Marshal failed: %v", err)
	}
	var decoded VerificationPlan
	if err := json.Unmarshal(data, &decoded); err != nil {
		t.Fatalf("Unmarshal failed: %v", err)
	}
	if len(decoded.Automated) != 2 {
		t.Errorf("expected 2 automated steps, got %d", len(decoded.Automated))
	}
}

// ---------------------------------------------------------------------------
// ToolCall JSON tests
// ---------------------------------------------------------------------------

func TestToolCall_JSON(t *testing.T) {
	tc := ToolCall{
		ID:    "tc_001",
		Name:  "bash",
		Input: json.RawMessage(`{"command":"ls"}`),
	}
	data, err := json.Marshal(tc)
	if err != nil {
		t.Fatalf("Marshal failed: %v", err)
	}
	var decoded ToolCall
	if err := json.Unmarshal(data, &decoded); err != nil {
		t.Fatalf("Unmarshal failed: %v", err)
	}
	if decoded.Name != "bash" {
		t.Errorf("expected 'bash', got %q", decoded.Name)
	}
}

// ---------------------------------------------------------------------------
// SkipDirs tests
// ---------------------------------------------------------------------------

func TestSkipDirs_NotEmpty(t *testing.T) {
	if len(SkipDirs) == 0 {
		t.Error("SkipDirs should not be empty")
	}
}

func TestSkipDirsMap_AllEntriesPresent(t *testing.T) {
	m := SkipDirsMap()
	for _, d := range SkipDirs {
		if !m[d] {
			t.Errorf("SkipDirsMap missing %q", d)
		}
	}
}
