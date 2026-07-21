package workflow

import (
	"errors"
	"os"
	"path/filepath"
	"strings"
	"testing"
	"time"

	m31errors "github.com/eshanized/M31A/internal/errors"
	"github.com/eshanized/M31A/internal/ledger"
	"github.com/eshanized/M31A/internal/engine/session"
	"github.com/eshanized/M31A/internal/engine/taskrunner"
	"github.com/eshanized/M31A/tests/testutil/mocks"
	"github.com/eshanized/M31A/internal/types"
)

// --- Task 1: H-1 + M-3 ---

func TestParseToolCalls_RejectsMalformed(t *testing.T) {
	eng := &Engine{}
	// Input that looks like tool calls (has "name" field) but is malformed JSON
	_, err := eng.parseToolCalls(`{"name":"Bash","input":{not valid json}}`)
	if !errors.Is(err, m31errors.ErrToolExecution) {
		t.Errorf("expected ErrToolExecution, got %v", err)
	}
}

func TestParseToolCalls_AcceptsSingleObject(t *testing.T) {
	eng := &Engine{}
	input := `{"name":"Bash","input":{"command":"echo hello"}}`
	calls, err := eng.parseToolCalls(input)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if len(calls) != 1 {
		t.Fatalf("expected 1 tool call, got %d", len(calls))
	}
	if calls[0].Name != "Bash" {
		t.Errorf("expected Bash, got %s", calls[0].Name)
	}
}

func TestParseToolCalls_NoToolCalls_ReturnsNil(t *testing.T) {
	eng := &Engine{}
	// Plain text with no tool calls — should not error
	calls, err := eng.parseToolCalls("just some text response")
	if err != nil {
		t.Fatalf("unexpected error for plain text: %v", err)
	}
	if len(calls) != 0 {
		t.Errorf("expected 0 tool calls, got %d", len(calls))
	}
}

// --- Task 2: H-3, H-4, M-4, H-6 ---

func TestTaskRunner_CycleDetected_Typed(t *testing.T) {
	tasks := []types.Task{
		{ID: 1, Action: "A", Description: "a", Dependencies: []int{2}},
		{ID: 2, Action: "B", Description: "b", Dependencies: []int{1}},
	}
	r := taskrunner.New(tasks)
	_, err := r.Schedule()
	if !errors.Is(err, m31errors.ErrCircularDependency) {
		t.Errorf("expected ErrCircularDependency, got %v", err)
	}
}

func TestPlanValidator_RejectsDuplicateIDs(t *testing.T) {
	tasks := []types.Task{
		{ID: 1, Action: "Create", Description: "a", Dependencies: []int{}},
		{ID: 1, Action: "Create", Description: "b", Dependencies: []int{}},
	}
	errs := validateTasks(tasks)
	found := false
	for _, e := range errs {
		if strings.Contains(e, "duplicate") || strings.Contains(e, "Duplicate") {
			found = true
			break
		}
	}
	if !found {
		t.Errorf("expected duplicate ID error, got %v", errs)
	}
}

func TestPlanValidator_RejectsUnknownDep(t *testing.T) {
	tasks := []types.Task{
		{ID: 1, Action: "Create", Description: "a", Dependencies: []int{99}},
	}
	errs := validateTasks(tasks)
	found := false
	for _, e := range errs {
		if strings.Contains(e, "non-existent dependency 99") {
			found = true
			break
		}
	}
	if !found {
		t.Errorf("expected unknown dep error, got %v", errs)
	}
}

func TestPlanValidator_RejectsSelfRef(t *testing.T) {
	tasks := []types.Task{
		{ID: 1, Action: "Create", Description: "a", Dependencies: []int{1}},
	}
	errs := validateTasks(tasks)
	found := false
	for _, e := range errs {
		if strings.Contains(e, "self-reference") {
			found = true
			break
		}
	}
	if !found {
		t.Errorf("expected self-reference error, got %v", errs)
	}
}

func TestPhaseTransition_RejectsSkipping(t *testing.T) {
	engine, _ := setupTestEngine(t)
	// Set engine to Ship phase
	engine.stateMachine.SetPhase(types.PhaseShip)
	// Ship → Execute is not valid
	err := engine.Transition(t.Context(), types.PhaseShip, types.PhaseExecute)
	if !errors.Is(err, m31errors.ErrPhaseTransition) {
		t.Errorf("expected ErrPhaseTransition, got %v", err)
	}
}

func TestPhaseTransition_AllowsIdleExit(t *testing.T) {
	engine, _ := setupTestEngine(t)
	// Set engine to Execute phase first
	engine.stateMachine.SetPhase(types.PhaseExecute)
	// Execute → Idle should be allowed (abort/cancel)
	err := engine.Transition(t.Context(), types.PhaseExecute, types.PhaseIdle)
	if err != nil {
		t.Errorf("expected no error for Execute→Idle, got %v", err)
	}
}

func TestPhaseTransition_AllowsValidFlow(t *testing.T) {
	engine, _ := setupTestEngine(t)
	validTransitions := []struct {
		from, to types.WorkflowPhase
	}{
		{types.PhaseIdle, types.PhaseInitialize},
		{types.PhaseInitialize, types.PhaseDiscuss},
		{types.PhaseInitialize, types.PhaseExecute}, // Direct mode
		{types.PhaseDiscuss, types.PhasePlan},
		{types.PhaseDiscuss, types.PhaseExecute}, // Fast mode
		{types.PhasePlan, types.PhaseExecute},
		{types.PhaseExecute, types.PhaseVerify},
		{types.PhaseExecute, types.PhaseShip}, // Direct mode
		{types.PhaseVerify, types.PhaseShip},
		{types.PhaseShip, types.PhaseIdle},
	}
	for _, tr := range validTransitions {
		// Set the engine to the correct starting phase for this transition
		engine.stateMachine.SetPhase(tr.from)
		err := engine.Transition(t.Context(), tr.from, tr.to)
		if err != nil {
			t.Errorf("expected no error for %s→%s, got %v", tr.from, tr.to, err)
		}
	}
}

// --- Task 3: H-15, H-16, H-17, H-18 ---

func TestExecute_CheckpointBeforeEachTask(t *testing.T) {
	engine, _ := setupTestEngine(t)

	// Initialize session
	_, err := engine.RunPhase(t.Context(), types.PhaseInitialize, "Test")
	if err != nil {
		t.Fatalf("Initialize failed: %v", err)
	}

	// Save 2 tasks
	tasks := []types.Task{
		{ID: 1, Action: "Create", Description: "Task 1", Dependencies: []int{}, Files: []string{"a.go"}, AcceptanceCriteria: []string{"ok"}},
		{ID: 2, Action: "Create", Description: "Task 2", Dependencies: []int{1}, Files: []string{"b.go"}, AcceptanceCriteria: []string{"ok"}},
	}
	engine.sessionMgr.SaveTasks(engine.sessionID, tasks)

	// Mock provider returns simple content
	mp := engine.provider.(*mocks.MockProvider)
	mp.Response_ = "Done"

	// Run execute
	_, err = engine.RunPhase(t.Context(), types.PhaseExecute, "Test")
	if err != nil {
		t.Fatalf("Execute failed: %v", err)
	}

	// Verify checkpoints exist. SaveCheckpoint trims to max 2 (keeps newest).
	// 2 pre-task saves + 1 final = 3 total, but only last 2 retained.
	checkpoints, err := engine.sessionMgr.LoadCheckpoints(engine.sessionID)
	if err != nil {
		t.Fatalf("LoadCheckpoints failed: %v", err)
	}
	if len(checkpoints) < 2 {
		t.Errorf("expected at least 2 checkpoints (trim keeps newest 2), got %d", len(checkpoints))
	}
}

func TestLedger_AppendIsAtomic(t *testing.T) {
	dir := t.TempDir()
	ledgerPath := filepath.Join(dir, "LEDGER.md")
	l := ledger.New(ledgerPath)

	// Append first entry
	entry1 := ledger.LedgerEntry{
		SessionID:       "sess0001",
		Timestamp:       time.Now(),
		Model:           "test-model",
		ProjectType:     "go",
		TaskCount:       3,
		FailedTasks:     0,
		CostEstimate:    1.5,
		DurationMinutes: 10,
	}
	if err := l.Append(entry1); err != nil {
		t.Fatalf("first Append failed: %v", err)
	}

	// Read and verify the file is well-formed
	data, err := os.ReadFile(ledgerPath)
	if err != nil {
		t.Fatalf("ReadFile failed: %v", err)
	}
	content := string(data)
	if !strings.Contains(content, "sess0001") {
		t.Error("ledger file missing session entry")
	}
	if !strings.Contains(content, "# Cross-Session Learning Ledger") {
		t.Error("ledger file missing header")
	}

	// Append second entry
	entry2 := ledger.LedgerEntry{
		SessionID:       "sess0002",
		Timestamp:       time.Now(),
		Model:           "test-model",
		ProjectType:     "python",
		TaskCount:       5,
		FailedTasks:     1,
		CostEstimate:    2.0,
		DurationMinutes: 20,
	}
	if appendErr := l.Append(entry2); appendErr != nil {
		t.Fatalf("second Append failed: %v", appendErr)
	}

	// Verify both entries present
	data, err = os.ReadFile(ledgerPath)
	if err != nil {
		t.Fatalf("ReadFile failed: %v", err)
	}
	content = string(data)
	if !strings.Contains(content, "sess0001") || !strings.Contains(content, "sess0002") {
		t.Error("ledger file missing entries after second append")
	}
}

func TestLedger_RejectsDuplicateSessionID(t *testing.T) {
	dir := t.TempDir()
	ledgerPath := filepath.Join(dir, "LEDGER.md")
	l := ledger.New(ledgerPath)

	entry := ledger.LedgerEntry{
		SessionID:       "sess0001",
		Timestamp:       time.Now(),
		Model:           "test-model",
		ProjectType:     "go",
		TaskCount:       3,
		FailedTasks:     0,
		CostEstimate:    1.5,
		DurationMinutes: 10,
	}

	// First append should succeed
	if err := l.Append(entry); err != nil {
		t.Fatalf("first Append failed: %v", err)
	}

	// Second append with same SessionID should return error
	err := l.Append(entry)
	if !errors.Is(err, m31errors.ErrTaskFailed) {
		t.Errorf("expected ErrTaskFailed for duplicate session, got %v", err)
	}
}

func TestSession_Load_RejectsMissingID(t *testing.T) {
	dir := t.TempDir()
	mgr := session.NewManager(dir, dir, session.ManagerOpts{})

	// Create a session with empty ID
	projectDir := filepath.Join(dir, ".m31a")
	os.MkdirAll(projectDir, 0755)
	sessionData := []byte(`{"id":"","model":"test","provider":"test","started_at":"2024-01-01T00:00:00Z","workflow_phase":"idle"}`)
	os.WriteFile(filepath.Join(projectDir, "session.json"), sessionData, 0644)

	_, err := mgr.LoadSession("aabbccdf")
	if !errors.Is(err, m31errors.ErrSessionCorrupted) {
		t.Errorf("expected ErrSessionCorrupted for missing ID, got %v", err)
	}
}

func TestSession_Load_RejectsZeroStartedAt(t *testing.T) {
	dir := t.TempDir()
	mgr := session.NewManager(dir, dir, session.ManagerOpts{})

	// Create a session with zero StartedAt
	projectDir := filepath.Join(dir, ".m31a")
	os.MkdirAll(projectDir, 0755)
	sessionData := []byte(`{"id":"aabbccdd","model":"test","provider":"test","started_at":"0001-01-01T00:00:00Z","workflow_phase":"idle"}`)
	os.WriteFile(filepath.Join(projectDir, "session.json"), sessionData, 0644)

	_, err := mgr.LoadSession("aabbccdd")
	if !errors.Is(err, m31errors.ErrSessionCorrupted) {
		t.Errorf("expected ErrSessionCorrupted for zero StartedAt, got %v", err)
	}
}

func TestSession_Load_RejectsUnknownPhase(t *testing.T) {
	dir := t.TempDir()
	mgr := session.NewManager(dir, dir, session.ManagerOpts{})

	// Create a session with unknown workflow phase
	projectDir := filepath.Join(dir, ".m31a")
	os.MkdirAll(projectDir, 0755)
	sessionData := []byte(`{"id":"aabbccde","model":"test","provider":"test","started_at":"2024-01-01T00:00:00Z","workflow_phase":"bogus"}`)
	os.WriteFile(filepath.Join(projectDir, "session.json"), sessionData, 0644)

	_, err := mgr.LoadSession("aabbccde")
	if !errors.Is(err, m31errors.ErrSessionCorrupted) {
		t.Errorf("expected ErrSessionCorrupted for unknown phase, got %v", err)
	}
}

func TestSession_Load_AcceptsEmptyPhase(t *testing.T) {
	dir := t.TempDir()
	mgr := session.NewManager(dir, dir, session.ManagerOpts{})

	// Create a session with empty workflow phase (legacy)
	projectDir := filepath.Join(dir, ".m31a")
	os.MkdirAll(projectDir, 0755)
	sessionData := []byte(`{"id":"abc0def0","model":"test","provider":"test","started_at":"2024-01-01T00:00:00Z"}`)
	os.WriteFile(filepath.Join(projectDir, "session.json"), sessionData, 0644)

	sess, err := mgr.LoadSession("abc0def0")
	if err != nil {
		t.Fatalf("expected no error for empty phase, got %v", err)
	}
	if sess.WorkflowPhase != types.PhaseIdle {
		t.Errorf("expected PhaseIdle for empty phase, got %s", sess.WorkflowPhase)
	}
}
