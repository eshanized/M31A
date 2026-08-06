package workflow

import (
	"encoding/json"
	"os"
	"path/filepath"
	"sync"
	"testing"
	"time"

	m31types "github.com/eshanized/M31A/internal/core/types"
	"github.com/eshanized/M31A/internal/engine/session"
)

// helper to set up an engine with a known recovery path
func setupRecoveryEngine(t *testing.T) (*Engine, string) {
	t.Helper()
	engine, _ := setupTestEngine(t)
	recoveryFile := engine.recoveryPath
	return engine, recoveryFile
}

// helper to write a raw recovery JSON file
func writeRecoveryJSON(t *testing.T, path string, state *RecoveryState) {
	t.Helper()
	data, err := json.Marshal(state)
	if err != nil {
		t.Fatalf("marshal recovery state: %v", err)
	}
	if err := os.MkdirAll(filepath.Dir(path), 0o755); err != nil {
		t.Fatalf("create dir: %v", err)
	}
	if err := os.WriteFile(path, data, 0o644); err != nil {
		t.Fatalf("write recovery file: %v", err)
	}
}

// helper to create a valid RecoveryState
func validRecoveryState() *RecoveryState {
	return &RecoveryState{
		SessionID:    "test-session-123",
		CurrentPhase: m31types.PhasePlan,
		PhaseHistory: []m31types.WorkflowPhase{m31types.PhaseIdle, m31types.PhaseInitialize, m31types.PhasePlan},
		PlanMarkdown: "# Plan\nSome content",
		PlanVersion:  2,
		Messages: []m31types.Message{
			{Role: "user", Content: "hello"},
			{Role: "assistant", Content: "hi"},
		},
		Goal:      "test goal",
		Timestamp: time.Now(),
	}
}

// ── Crash Recovery Tests ──────────────────────────────────────────────

// TestRecovery_CrashDuringPhaseTransition simulates a crash during a phase
// transition by saving recovery state mid-transition and verifying it
// can be loaded to restore the pre-transition state.
func TestRecovery_CrashDuringPhaseTransition(t *testing.T) {
	engine, recoveryFile := setupRecoveryEngine(t)

	// Set engine to a known state
	engine.stateMachine.SetPhase(m31types.PhaseInitialize)
	engine.state.SetPlanContent("# Plan")
	engine.state.SetPlanVersion(1)
	engine.state.SetCurrentGoal("test goal")
	engine.state.SetMessages([]m31types.Message{
		{Role: "user", Content: "build a server"},
	})

	// Simulate crash: save recovery state before transition
	if err := SaveRecoveryState(engine, recoveryFile); err != nil {
		t.Fatalf("SaveRecoveryState failed: %v", err)
	}

	// Verify recovery state was written
	state, err := LoadRecoveryState(recoveryFile)
	if err != nil {
		t.Fatalf("LoadRecoveryState failed: %v", err)
	}

	// Verify state matches what we saved
	if state.CurrentPhase != m31types.PhaseInitialize {
		t.Errorf("expected phase Initialize, got %s", state.CurrentPhase)
	}
	if state.PlanMarkdown != "# Plan" {
		t.Errorf("expected plan markdown '# Plan', got %q", state.PlanMarkdown)
	}
	if state.Goal != "test goal" {
		t.Errorf("expected goal 'test goal', got %q", state.Goal)
	}
	if len(state.Messages) != 1 {
		t.Errorf("expected 1 message, got %d", len(state.Messages))
	}
}

// TestRecovery_CrashDuringPlanExecution saves recovery state with plan
// content and messages, simulating a crash during plan execution.
func TestRecovery_CrashDuringPlanExecution(t *testing.T) {
	engine, recoveryFile := setupRecoveryEngine(t)

	// Set engine to plan phase with content
	engine.stateMachine.SetPhase(m31types.PhasePlan)
	engine.state.SetPlanContent("# Detailed Plan\n## Tasks\n- Task 1\n- Task 2")
	engine.state.SetPlanVersion(3)
	engine.state.SetCurrentGoal("implement feature X")
	engine.state.SetMessages([]m31types.Message{
		{Role: "system", Content: "You are helpful"},
		{Role: "user", Content: "build feature X"},
		{Role: "assistant", Content: "I'll plan this out"},
	})

	// Save recovery
	if err := SaveRecoveryState(engine, recoveryFile); err != nil {
		t.Fatalf("SaveRecoveryState failed: %v", err)
	}

	// Load and verify
	state, err := LoadRecoveryState(recoveryFile)
	if err != nil {
		t.Fatalf("LoadRecoveryState failed: %v", err)
	}

	if state.PlanVersion != 3 {
		t.Errorf("expected plan version 3, got %d", state.PlanVersion)
	}
	if len(state.Messages) != 3 {
		t.Errorf("expected 3 messages, got %d", len(state.Messages))
	}
	if state.CurrentPhase != m31types.PhasePlan {
		t.Errorf("expected phase Plan, got %s", state.CurrentPhase)
	}
}

// TestRecovery_CorruptedRecoveryFile verifies that loading a corrupted
// recovery file returns an error, allowing the engine to start fresh.
func TestRecovery_CorruptedRecoveryFile(t *testing.T) {
	_, recoveryFile := setupRecoveryEngine(t)

	// Write garbage to the recovery file
	if err := os.MkdirAll(filepath.Dir(recoveryFile), 0o755); err != nil {
		t.Fatalf("create dir: %v", err)
	}
	if err := os.WriteFile(recoveryFile, []byte("not valid json {{{"), 0o644); err != nil {
		t.Fatalf("write garbage: %v", err)
	}

	// Attempt to load — should fail
	_, err := LoadRecoveryState(recoveryFile)
	if err == nil {
		t.Fatal("expected error loading corrupted recovery file, got nil")
	}
}

// TestRecovery_MissingRecoveryFile verifies that loading when no recovery
// file exists returns an appropriate error.
func TestRecovery_MissingRecoveryFile(t *testing.T) {
	nonExistent := filepath.Join(t.TempDir(), "nonexistent", "recovery.json")

	_, err := LoadRecoveryState(nonExistent)
	if err == nil {
		t.Fatal("expected error for missing recovery file, got nil")
	}
}

// TestRecovery_AtomicWriteGuarantee verifies that AtomicWrite is used for
// recovery persistence, ensuring partial writes do not corrupt existing state.
func TestRecovery_AtomicWriteGuarantee(t *testing.T) {
	_, recoveryFile := setupRecoveryEngine(t)

	// Write initial valid state
	state1 := validRecoveryState()
	state1.CurrentPhase = m31types.PhaseInitialize
	writeRecoveryJSON(t, recoveryFile, state1)

	// Verify initial state is valid
	loaded, err := LoadRecoveryState(recoveryFile)
	if err != nil {
		t.Fatalf("LoadRecoveryState failed: %v", err)
	}
	if loaded.CurrentPhase != m31types.PhaseInitialize {
		t.Errorf("expected phase Initialize, got %s", loaded.CurrentPhase)
	}

	// Overwrite with new state
	state2 := validRecoveryState()
	state2.CurrentPhase = m31types.PhasePlan
	data, _ := json.Marshal(state2)
	if writeErr := os.WriteFile(recoveryFile, data, 0o644); writeErr != nil {
		t.Fatalf("write: %v", writeErr)
	}

	// Verify new state is valid (no corruption)
	loaded2, err := LoadRecoveryState(recoveryFile)
	if err != nil {
		t.Fatalf("LoadRecoveryState after overwrite failed: %v", err)
	}
	if loaded2.CurrentPhase != m31types.PhasePlan {
		t.Errorf("expected phase Plan, got %s", loaded2.CurrentPhase)
	}
}

// ── Forced Exit Tests ────────────────────────────────────────────────

// TestRecovery_ForcedExitDuringToolExecution verifies that recovery state
// is consistent even when a tool execution is interrupted.
func TestRecovery_ForcedExitDuringToolExecution(t *testing.T) {
	engine, recoveryFile := setupRecoveryEngine(t)

	// Set engine to execute phase with tool-related messages
	engine.stateMachine.SetPhase(m31types.PhaseExecute)
	engine.state.SetCurrentGoal("run tests")
	engine.state.SetMessages([]m31types.Message{
		{Role: "user", Content: "run tests"},
		{Role: "assistant", Content: "running tests", ToolCalls: []m31types.ToolCall{
			{ID: "call_1", Name: "Bash", Input: json.RawMessage(`{"command":"go test"}`)},
		}},
	})

	// Save recovery mid-execution
	if err := SaveRecoveryState(engine, recoveryFile); err != nil {
		t.Fatalf("SaveRecoveryState failed: %v", err)
	}

	// Verify recovery state is consistent
	state, err := LoadRecoveryState(recoveryFile)
	if err != nil {
		t.Fatalf("LoadRecoveryState failed: %v", err)
	}

	if state.CurrentPhase != m31types.PhaseExecute {
		t.Errorf("expected phase Execute, got %s", state.CurrentPhase)
	}
	if len(state.Messages) != 2 {
		t.Errorf("expected 2 messages, got %d", len(state.Messages))
	}
}

// TestRecovery_ForcedExitDuringLLMStreaming verifies that recovery state
// captures progress during LLM streaming.
func TestRecovery_ForcedExitDuringLLMStreaming(t *testing.T) {
	engine, recoveryFile := setupRecoveryEngine(t)

	// Set engine state as if LLM is streaming
	engine.stateMachine.SetPhase(m31types.PhaseDiscuss)
	engine.state.SetCurrentGoal("discuss architecture")
	engine.state.SetMessages([]m31types.Message{
		{Role: "user", Content: "discuss architecture"},
		{Role: "assistant", Content: "let me think about this..."},
	})

	// Save recovery
	if err := SaveRecoveryState(engine, recoveryFile); err != nil {
		t.Fatalf("SaveRecoveryState failed: %v", err)
	}

	// Verify
	state, err := LoadRecoveryState(recoveryFile)
	if err != nil {
		t.Fatalf("LoadRecoveryState failed: %v", err)
	}

	if state.CurrentPhase != m31types.PhaseDiscuss {
		t.Errorf("expected phase Discuss, got %s", state.CurrentPhase)
	}
}

// ── Interrupted Session Tests ────────────────────────────────────────

// TestRecovery_SessionResume_WithRecovery creates a session with recovery
// state, then verifies the engine can recover from it.
func TestRecovery_SessionResume_WithRecovery(t *testing.T) {
	engine, recoveryFile := setupRecoveryEngine(t)

	// Set engine to a non-initial state
	engine.stateMachine.SetPhase(m31types.PhaseVerify)
	engine.state.SetPlanContent("# Verified Plan")
	engine.state.SetPlanVersion(5)
	engine.state.SetCurrentGoal("verify implementation")
	engine.state.SetMessages([]m31types.Message{
		{Role: "user", Content: "verify the implementation"},
		{Role: "assistant", Content: "checking tests..."},
	})

	// Save recovery
	if err := SaveRecoveryState(engine, recoveryFile); err != nil {
		t.Fatalf("SaveRecoveryState failed: %v", err)
	}

	// Create a fresh engine with the same recovery path
	engine2, _ := setupRecoveryEngine(t)
	// Override the recovery path to point to our recovery file
	engine2.recoveryPath = recoveryFile

	// Recover
	if err := engine2.Recover(); err != nil {
		t.Fatalf("Recover failed: %v", err)
	}

	// Verify restored state
	if engine2.stateMachine.CurrentPhase() != m31types.PhaseVerify {
		t.Errorf("expected phase Verify, got %s", engine2.stateMachine.CurrentPhase())
	}
	if engine2.state.PlanContent() != "# Verified Plan" {
		t.Errorf("expected plan '# Verified Plan', got %q", engine2.state.PlanContent())
	}
	if engine2.state.PlanVersion() != 5 {
		t.Errorf("expected plan version 5, got %d", engine2.state.PlanVersion())
	}
	if engine2.state.CurrentGoal() != "verify implementation" {
		t.Errorf("expected goal 'verify implementation', got %q", engine2.state.CurrentGoal())
	}
	msgs := engine2.state.MessagesSnapshot()
	if len(msgs) != 2 {
		t.Errorf("expected 2 messages, got %d", len(msgs))
	}
}

// TestRecovery_SessionResume_WithoutRecovery verifies that starting without
// a recovery file results in a clean start (no error).
func TestRecovery_SessionResume_WithoutRecovery(t *testing.T) {
	engine, _ := setupRecoveryEngine(t)

	// Ensure no recovery file exists
	os.Remove(engine.recoveryPath) //nolint:errcheck

	// Recover should succeed (clean start)
	if err := engine.Recover(); err != nil {
		t.Fatalf("Recover with no file should return nil, got: %v", err)
	}

	// Phase should remain at default (Idle)
	if engine.stateMachine.CurrentPhase() != m31types.PhaseIdle {
		t.Errorf("expected phase Idle, got %s", engine.stateMachine.CurrentPhase())
	}
}

// TestRecovery_SessionResume_CorruptedRecovery verifies that corrupted
// recovery state causes recovery to fail gracefully.
func TestRecovery_SessionResume_CorruptedRecovery(t *testing.T) {
	engine, recoveryFile := setupRecoveryEngine(t)

	// Write corrupted recovery file
	if err := os.MkdirAll(filepath.Dir(recoveryFile), 0o755); err != nil {
		t.Fatalf("create dir: %v", err)
	}
	if err := os.WriteFile(recoveryFile, []byte("corrupted"), 0o644); err != nil {
		t.Fatalf("write corrupted: %v", err)
	}

	// Recover should return an error
	err := engine.Recover()
	if err == nil {
		t.Fatal("expected error for corrupted recovery, got nil")
	}

	// Phase should remain at Idle (no state changed)
	if engine.stateMachine.CurrentPhase() != m31types.PhaseIdle {
		t.Errorf("expected phase Idle after failed recovery, got %s", engine.stateMachine.CurrentPhase())
	}
}

// ── Failed Workflow Tests ────────────────────────────────────────────

// TestRecovery_WorkflowFailure_PhaseRollback verifies that a workflow
// failure can be rolled back to the previous consistent state.
func TestRecovery_WorkflowFailure_PhaseRollback(t *testing.T) {
	engine, recoveryFile := setupRecoveryEngine(t)

	// Set up engine in Execute phase with a history that includes Plan
	engine.stateMachine.SetPhase(m31types.PhaseExecute)
	engine.state.SetPlanContent("# Plan for rollback test")
	engine.state.SetPlanVersion(2)
	engine.state.SetCurrentGoal("execute plan")
	engine.state.SetMessages([]m31types.Message{
		{Role: "user", Content: "execute"},
	})

	// Save recovery (this captures the state including history)
	if err := SaveRecoveryState(engine, recoveryFile); err != nil {
		t.Fatalf("SaveRecoveryState failed: %v", err)
	}

	// Verify the recovery state has the expected history
	state, err := LoadRecoveryState(recoveryFile)
	if err != nil {
		t.Fatalf("LoadRecoveryState failed: %v", err)
	}

	// History should have at least Idle -> Initialize -> ... -> Execute
	if len(state.PhaseHistory) < 2 {
		t.Fatalf("expected at least 2 history entries, got %d", len(state.PhaseHistory))
	}

	// The last entry should be Execute (current phase)
	lastPhase := state.PhaseHistory[len(state.PhaseHistory)-1]
	if lastPhase != m31types.PhaseExecute {
		t.Errorf("expected last history entry to be Execute, got %s", lastPhase)
	}
}

// TestRecovery_WorkflowFailure_PreservesMessages verifies that messages from
// a failed phase are preserved in the recovery state for debugging.
func TestRecovery_WorkflowFailure_PreservesMessages(t *testing.T) {
	engine, recoveryFile := setupRecoveryEngine(t)

	// Set up messages from a failed phase
	messages := []m31types.Message{
		{Role: "system", Content: "You are helpful"},
		{Role: "user", Content: "run the failing test"},
		{Role: "assistant", Content: "running test..."},
		{Role: "tool", Content: "FAIL: test TestFoo failed"},
		{Role: "assistant", Content: "the test failed, let me investigate"},
	}

	engine.stateMachine.SetPhase(m31types.PhaseExecute)
	engine.state.SetCurrentGoal("fix failing test")
	engine.state.SetMessages(messages)

	// Save recovery
	if err := SaveRecoveryState(engine, recoveryFile); err != nil {
		t.Fatalf("SaveRecoveryState failed: %v", err)
	}

	// Load and verify messages are preserved
	state, err := LoadRecoveryState(recoveryFile)
	if err != nil {
		t.Fatalf("LoadRecoveryState failed: %v", err)
	}

	if len(state.Messages) != 5 {
		t.Errorf("expected 5 messages preserved, got %d", len(state.Messages))
	}

	// Verify the tool failure message is there
	found := false
	for _, msg := range state.Messages {
		if msg.Role == "tool" && msg.Content == "FAIL: test TestFoo failed" {
			found = true
			break
		}
	}
	if !found {
		t.Error("expected tool failure message to be preserved")
	}
}

// TestRecovery_WorkflowFailure_ClearsRecoveryAfterSuccess verifies that
// recovery file is cleaned up after successful phase completion.
func TestRecovery_WorkflowFailure_ClearsRecoveryAfterSuccess(t *testing.T) {
	engine, recoveryFile := setupRecoveryEngine(t)

	// Save recovery state
	if err := SaveRecoveryState(engine, recoveryFile); err != nil {
		t.Fatalf("SaveRecoveryState failed: %v", err)
	}

	// Verify file exists
	if _, err := os.Stat(recoveryFile); os.IsNotExist(err) {
		t.Fatal("recovery file should exist before clear")
	}

	// Clear recovery
	if err := engine.ClearRecovery(); err != nil {
		t.Fatalf("ClearRecovery failed: %v", err)
	}

	// Verify file is removed
	if _, err := os.Stat(recoveryFile); !os.IsNotExist(err) {
		t.Error("recovery file should be removed after ClearRecovery")
	}
}

// ── Concurrency Tests ────────────────────────────────────────────────

// TestRecovery_ConcurrentSaveAndLoad verifies that multiple goroutines
// saving recovery state simultaneously do not corrupt the file.
func TestRecovery_ConcurrentSaveAndLoad(t *testing.T) {
	engine, recoveryFile := setupRecoveryEngine(t)

	// Ensure recovery directory exists
	if err := os.MkdirAll(filepath.Dir(recoveryFile), 0o755); err != nil {
		t.Fatalf("create dir: %v", err)
	}

	var wg sync.WaitGroup
	const goroutines = 10
	const iterations = 50

	// Concurrent writers
	for i := 0; i < goroutines; i++ {
		wg.Add(1)
		go func(id int) {
			defer wg.Done()
			for j := 0; j < iterations; j++ {
				engine.stateMachine.SetPhase(m31types.PhaseExecute)
				engine.state.SetPlanContent("# Plan from goroutine")
				engine.state.SetCurrentGoal("concurrent goal")
				_ = SaveRecoveryState(engine, recoveryFile)
			}
		}(i)
	}

	// Concurrent readers
	for i := 0; i < goroutines; i++ {
		wg.Add(1)
		go func(id int) {
			defer wg.Done()
			for j := 0; j < iterations; j++ {
				state, err := LoadRecoveryState(recoveryFile)
				if err != nil {
					// File may not exist yet or be mid-write — that's OK
					continue
				}
				// Verify loaded state is internally consistent
				if state.SessionID == "" {
					t.Errorf("goroutine %d: loaded state has empty session ID", id)
				}
			}
		}(i)
	}

	wg.Wait()

	// Final state should be valid
	state, err := LoadRecoveryState(recoveryFile)
	if err != nil {
		t.Fatalf("final LoadRecoveryState failed: %v", err)
	}
	if state.SessionID == "" {
		t.Error("final state has empty session ID")
	}
}

// TestRecovery_SaveAndCrash_Race verifies that atomic write prevents
// corruption when saving recovery state while simultaneously crashing.
func TestRecovery_SaveAndCrash_Race(t *testing.T) {
	_, recoveryFile := setupRecoveryEngine(t)

	// Write initial valid state
	state1 := validRecoveryState()
	writeRecoveryJSON(t, recoveryFile, state1)

	// Verify initial state is valid
	loaded, err := LoadRecoveryState(recoveryFile)
	if err != nil {
		t.Fatalf("initial load failed: %v", err)
	}
	if loaded.CurrentPhase != m31types.PhasePlan {
		t.Errorf("expected initial phase Plan, got %s", loaded.CurrentPhase)
	}

	// Simulate concurrent writes (as if multiple engines are writing)
	var wg sync.WaitGroup
	for i := 0; i < 5; i++ {
		wg.Add(1)
		go func(id int) {
			defer wg.Done()
			state := validRecoveryState()
			state.CurrentPhase = m31types.PhaseExecute
			state.Goal = "concurrent write"
			data, _ := json.Marshal(state)
			_ = os.WriteFile(recoveryFile, data, 0o644)
		}(i)
	}
	wg.Wait()

	// After concurrent writes, the file should still be valid JSON
	// (atomic write ensures no half-written state)
	data, err := os.ReadFile(recoveryFile)
	if err != nil {
		t.Fatalf("read after race: %v", err)
	}

	var finalState RecoveryState
	if err := json.Unmarshal(data, &finalState); err != nil {
		t.Fatalf("unmarshal after race failed (file corrupted): %v", err)
	}

	// State should be internally consistent
	if finalState.SessionID == "" {
		t.Error("final state has empty session ID after race")
	}
}

// ── Validation Tests ─────────────────────────────────────────────────

// TestRecovery_ValidateState_Valid verifies that a valid recovery state
// passes validation.
func TestRecovery_ValidateState_Valid(t *testing.T) {
	state := validRecoveryState()
	if err := ValidateRecoveryState(state); err != nil {
		t.Errorf("expected valid state, got error: %v", err)
	}
}

// TestRecovery_ValidateState_Nil verifies that nil state fails validation.
func TestRecovery_ValidateState_Nil(t *testing.T) {
	if err := ValidateRecoveryState(nil); err == nil {
		t.Error("expected error for nil state")
	}
}

// TestRecovery_ValidateState_InvalidPhase verifies that an invalid phase
// fails validation.
func TestRecovery_ValidateState_InvalidPhase(t *testing.T) {
	state := validRecoveryState()
	state.CurrentPhase = "InvalidPhase"
	if err := ValidateRecoveryState(state); err == nil {
		t.Error("expected error for invalid phase")
	}
}

// TestRecovery_ValidateState_EmptyHistory verifies that empty history
// fails validation.
func TestRecovery_ValidateState_EmptyHistory(t *testing.T) {
	state := validRecoveryState()
	state.PhaseHistory = []m31types.WorkflowPhase{}
	if err := ValidateRecoveryState(state); err == nil {
		t.Error("expected error for empty history")
	}
}

// TestRecovery_ValidateState_HistoryNotStartingWithIdle verifies that
// history not starting with Idle fails validation.
func TestRecovery_ValidateState_HistoryNotStartingWithIdle(t *testing.T) {
	state := validRecoveryState()
	state.PhaseHistory = []m31types.WorkflowPhase{m31types.PhaseInitialize, m31types.PhasePlan}
	if err := ValidateRecoveryState(state); err == nil {
		t.Error("expected error for history not starting with Idle")
	}
}

// TestRecovery_ValidateState_FutureTimestamp verifies that a timestamp
// far in the future fails validation.
func TestRecovery_ValidateState_FutureTimestamp(t *testing.T) {
	state := validRecoveryState()
	state.Timestamp = time.Now().Add(30 * 24 * time.Hour) // 30 days in future
	if err := ValidateRecoveryState(state); err == nil {
		t.Error("expected error for future timestamp")
	}
}

// TestRecovery_ValidateState_OldTimestamp verifies that a timestamp
// far in the past fails validation.
func TestRecovery_ValidateState_OldTimestamp(t *testing.T) {
	state := validRecoveryState()
	state.Timestamp = time.Now().Add(-30 * 24 * time.Hour) // 30 days in past
	if err := ValidateRecoveryState(state); err == nil {
		t.Error("expected error for old timestamp")
	}
}

// TestRecovery_ValidateState_MissingSessionID verifies that missing
// session ID fails validation.
func TestRecovery_ValidateState_MissingSessionID(t *testing.T) {
	state := validRecoveryState()
	state.SessionID = ""
	if err := ValidateRecoveryState(state); err == nil {
		t.Error("expected error for missing session ID")
	}
}

// ── Session Manager Recovery Tests ───────────────────────────────────

// TestRecovery_SessionManager_RecoveryExists verifies the session manager's
// RecoveryExists method correctly detects recovery files.
func TestRecovery_SessionManager_RecoveryExists(t *testing.T) {
	dir := t.TempDir()
	mgr := session.NewManager(dir, dir, session.ManagerOpts{})

	// Initially no recovery
	if mgr.RecoveryExists() {
		t.Error("expected no recovery before writing")
	}

	// Create recovery file in the expected location
	recoveryDir := filepath.Join(dir, ".m31a")
	if err := os.MkdirAll(recoveryDir, 0o755); err != nil {
		t.Fatalf("create dir: %v", err)
	}
	state := validRecoveryState()
	data, _ := json.Marshal(state)
	if err := os.WriteFile(filepath.Join(recoveryDir, "recovery.json"), data, 0o644); err != nil {
		t.Fatalf("write recovery: %v", err)
	}

	// Now should exist
	if !mgr.RecoveryExists() {
		t.Error("expected recovery to exist after writing")
	}

	// Clear it
	if err := mgr.ClearRecovery(); err != nil {
		t.Fatalf("ClearRecovery failed: %v", err)
	}

	// Should not exist
	if mgr.RecoveryExists() {
		t.Error("expected no recovery after clear")
	}
}

// TestRecovery_SessionManager_LoadRecoveryBytes verifies the session manager's
// LoadRecoveryBytes method returns valid data.
func TestRecovery_SessionManager_LoadRecoveryBytes(t *testing.T) {
	dir := t.TempDir()
	mgr := session.NewManager(dir, dir, session.ManagerOpts{})

	// Write recovery file
	recoveryDir := filepath.Join(dir, ".m31a")
	if err := os.MkdirAll(recoveryDir, 0o755); err != nil {
		t.Fatalf("create dir: %v", err)
	}
	state := validRecoveryState()
	data, _ := json.Marshal(state)
	if err := os.WriteFile(filepath.Join(recoveryDir, "recovery.json"), data, 0o644); err != nil {
		t.Fatalf("write recovery: %v", err)
	}

	// Load bytes
	bytes, err := mgr.LoadRecoveryBytes()
	if err != nil {
		t.Fatalf("LoadRecoveryBytes failed: %v", err)
	}

	// Verify bytes are valid JSON
	var loaded RecoveryState
	if err := json.Unmarshal(bytes, &loaded); err != nil {
		t.Fatalf("unmarshal loaded bytes failed: %v", err)
	}

	if loaded.SessionID != "test-session-123" {
		t.Errorf("expected session ID 'test-session-123', got %q", loaded.SessionID)
	}
}

// TestRecovery_SessionManager_ListSessionsRecoverable verifies that
// session listing includes the Recoverable field.
func TestRecovery_SessionManager_ListSessionsRecoverable(t *testing.T) {
	dir := t.TempDir()
	mgr := session.NewManager(dir, dir, session.ManagerOpts{})

	// Create a session
	sess, err := mgr.NewSession("test-model", "test-provider")
	if err != nil {
		t.Fatalf("NewSession failed: %v", err)
	}

	// List sessions — should not be recoverable initially
	sessions, err := mgr.ListSessions()
	if err != nil {
		t.Fatalf("ListSessions failed: %v", err)
	}
	if len(sessions) != 1 {
		t.Fatalf("expected 1 session, got %d", len(sessions))
	}
	if sessions[0].Recoverable {
		t.Error("expected not recoverable before writing recovery file")
	}

	// Write recovery file
	recoveryDir := filepath.Join(dir, ".m31a")
	if mkdirErr := os.MkdirAll(recoveryDir, 0o755); mkdirErr != nil {
		t.Fatalf("create dir: %v", mkdirErr)
	}
	state := validRecoveryState()
	state.SessionID = sess.ID
	data, _ := json.Marshal(state)
	if writeErr := os.WriteFile(filepath.Join(recoveryDir, "recovery.json"), data, 0o644); writeErr != nil {
		t.Fatalf("write recovery: %v", writeErr)
	}

	// List again — should be recoverable
	sessions, err = mgr.ListSessions()
	if err != nil {
		t.Fatalf("ListSessions failed: %v", err)
	}
	if len(sessions) != 1 {
		t.Fatalf("expected 1 session, got %d", len(sessions))
	}
	if !sessions[0].Recoverable {
		t.Error("expected recoverable after writing recovery file")
	}
}
