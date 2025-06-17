package session

import (
	"os"
	"testing"

	"github.com/eshanized/M31A/internal/types"
)

// TestSession_SetWorkflowState_RoundTrip verifies that the in-memory
// setter and getter on a Session struct are inverses — i.e. calling
// SetWorkflowState then WorkflowState returns the same values. This
// is the unit-level guarantee that the typed accessors work.
func TestSession_SetWorkflowState_RoundTrip(t *testing.T) {
	s := NewSession("test-id", "gpt-4o", "openrouter")
	s.SetWorkflowState("build a REST API", types.PhasePlan, []string{"Q1", "Q2"})

	goal, phase, questions := s.WorkflowState()
	if goal != "build a REST API" {
		t.Errorf("expected goal 'build a REST API', got %q", goal)
	}
	if phase != types.PhasePlan {
		t.Errorf("expected phase Plan, got %s", phase)
	}
	if len(questions) != 2 || questions[0] != "Q1" || questions[1] != "Q2" {
		t.Errorf("expected questions [Q1 Q2], got %v", questions)
	}
}

// TestSession_SetWorkflowState_ZeroGoal verifies that an empty goal
// is correctly preserved (i.e. the setter does not strip empty
// values, which would be wrong for a 'clear workflow' operation).
func TestSession_SetWorkflowState_ZeroGoal(t *testing.T) {
	s := NewSession("test-id", "gpt-4o", "openrouter")
	s.SetWorkflowState("some goal", types.PhasePlan, []string{"Q1"})
	s.SetWorkflowState("", types.PhaseIdle, nil)

	goal, phase, questions := s.WorkflowState()
	if goal != "" {
		t.Errorf("expected empty goal, got %q", goal)
	}
	if phase != types.PhaseIdle {
		t.Errorf("expected phase Idle, got %s", phase)
	}
	if len(questions) != 0 {
		t.Errorf("expected empty questions, got %v", questions)
	}
}

// TestSession_NewSession_InitializesWorkflowFields verifies that a
// freshly created Session has the new workflow state fields
// initialized to zero values (DiscussQuestions is an empty slice,
// not nil, so JSON encoding produces [] not null).
func TestSession_NewSession_InitializesWorkflowFields(t *testing.T) {
	s := NewSession("test-id", "gpt-4o", "openrouter")

	if s.WorkflowGoal != "" {
		t.Errorf("expected empty WorkflowGoal, got %q", s.WorkflowGoal)
	}
	if s.DiscussQuestions == nil {
		t.Error("expected DiscussQuestions to be initialized to empty slice, got nil")
	}
	if len(s.DiscussQuestions) != 0 {
		t.Errorf("expected empty DiscussQuestions, got %v", s.DiscussQuestions)
	}
	if s.WorkflowPhase != types.PhaseIdle {
		t.Errorf("expected PhaseIdle, got %s", s.WorkflowPhase)
	}
}

// TestManager_UpdateWorkflowState_PersistsAndLoads verifies the
// end-to-end persistence flow: UpdateWorkflowState writes the
// state to session.json and LoadWorkflowState reads it back
// verbatim. This is the core D-06 guarantee.
func TestManager_UpdateWorkflowState_PersistsAndLoads(t *testing.T) {
	dir, err := os.MkdirTemp("", "m31a-update-wf-*")
	if err != nil {
		t.Fatalf("Failed to create temp dir: %v", err)
	}
	defer os.RemoveAll(dir)

	mgr := NewManager(dir, ManagerOpts{})

	// Create a session
	s, err := mgr.NewSession("gpt-4o", "openrouter")
	if err != nil {
		t.Fatalf("NewSession failed: %v", err)
	}

	// Update workflow state
	err = mgr.UpdateWorkflowState(s.ID, "build a REST API", types.PhasePlan, []string{"Q1"})
	if err != nil {
		t.Fatalf("UpdateWorkflowState failed: %v", err)
	}

	// Load it back via LoadWorkflowState
	goal, phase, questions, err := mgr.LoadWorkflowState(s.ID)
	if err != nil {
		t.Fatalf("LoadWorkflowState failed: %v", err)
	}
	if goal != "build a REST API" {
		t.Errorf("expected goal preserved, got %q", goal)
	}
	if phase != types.PhasePlan {
		t.Errorf("expected phase Plan preserved, got %s", phase)
	}
	if len(questions) != 1 || questions[0] != "Q1" {
		t.Errorf("expected questions preserved, got %v", questions)
	}
}

// TestManager_UpdateWorkflowState_OverwritesPrevious verifies that
// calling UpdateWorkflowState twice on the same session replaces the
// first call's values with the second call's values (no append
// behavior, no merge). This matches the setter semantics on the
// Session struct.
func TestManager_UpdateWorkflowState_OverwritesPrevious(t *testing.T) {
	dir, err := os.MkdirTemp("", "m31a-update-wf2-*")
	if err != nil {
		t.Fatalf("Failed to create temp dir: %v", err)
	}
	defer os.RemoveAll(dir)

	mgr := NewManager(dir, ManagerOpts{})
	s, err := mgr.NewSession("gpt-4o", "openrouter")
	if err != nil {
		t.Fatalf("NewSession failed: %v", err)
	}

	// First update
	if err := mgr.UpdateWorkflowState(s.ID, "first goal", types.PhasePlan, []string{"Q1"}); err != nil {
		t.Fatalf("first UpdateWorkflowState failed: %v", err)
	}

	// Second update overwrites
	if err := mgr.UpdateWorkflowState(s.ID, "second goal", types.PhaseExecute, []string{"Q2", "Q3"}); err != nil {
		t.Fatalf("second UpdateWorkflowState failed: %v", err)
	}

	goal, phase, questions, err := mgr.LoadWorkflowState(s.ID)
	if err != nil {
		t.Fatalf("LoadWorkflowState failed: %v", err)
	}
	if goal != "second goal" {
		t.Errorf("expected goal 'second goal', got %q", goal)
	}
	if phase != types.PhaseExecute {
		t.Errorf("expected phase Execute, got %s", phase)
	}
	if len(questions) != 2 || questions[0] != "Q2" || questions[1] != "Q3" {
		t.Errorf("expected questions [Q2 Q3], got %v", questions)
	}
}

// TestManager_LoadWorkflowState_ReturnsZeroForUnset verifies that
// loading the workflow state for a session that was never explicitly
// set returns zero values (PhaseIdle, empty goal, empty questions)
// without an error. This matches the contract documented in the
// 14-04 plan: missing sessions return zero state, not an error.
func TestManager_LoadWorkflowState_ReturnsZeroForUnset(t *testing.T) {
	dir, err := os.MkdirTemp("", "m31a-load-wf-unset-*")
	if err != nil {
		t.Fatalf("Failed to create temp dir: %v", err)
	}
	defer os.RemoveAll(dir)

	mgr := NewManager(dir, ManagerOpts{})

	// No session created — should return zero values with no error
	goal, phase, questions, err := mgr.LoadWorkflowState("nonexistent-id")
	if err != nil {
		t.Fatalf("expected nil error for nonexistent session, got %v", err)
	}
	if goal != "" {
		t.Errorf("expected empty goal, got %q", goal)
	}
	if phase != types.PhaseIdle {
		t.Errorf("expected phase Idle, got %s", phase)
	}
	if len(questions) != 0 {
		t.Errorf("expected empty questions, got %v", questions)
	}
}

// TestManager_LoadWorkflowState_ReturnsZeroForUnsetSession verifies
// the same zero-value contract when the session EXISTS on disk but
// has never had its workflow state explicitly set (e.g. immediately
// after NewSession, before any phase transition).
func TestManager_LoadWorkflowState_ReturnsZeroForUnsetSession(t *testing.T) {
	dir, err := os.MkdirTemp("", "m31a-load-wf-unset2-*")
	if err != nil {
		t.Fatalf("Failed to create temp dir: %v", err)
	}
	defer os.RemoveAll(dir)

	mgr := NewManager(dir, ManagerOpts{})
	s, err := mgr.NewSession("gpt-4o", "openrouter")
	if err != nil {
		t.Fatalf("NewSession failed: %v", err)
	}

	// Session exists but workflow state was never set
	goal, phase, questions, err := mgr.LoadWorkflowState(s.ID)
	if err != nil {
		t.Fatalf("expected nil error, got %v", err)
	}
	if goal != "" {
		t.Errorf("expected empty goal, got %q", goal)
	}
	if phase != types.PhaseIdle {
		t.Errorf("expected phase Idle, got %s", phase)
	}
	if len(questions) != 0 {
		t.Errorf("expected empty questions, got %v", questions)
	}
}

// TestManager_UpdateWorkflowState_AfterReset verifies the post-Ship
// reset flow: after UpdateWorkflowState is called with empty
// values, LoadWorkflowState returns zero values (the persisted
// state has been cleared).
func TestManager_UpdateWorkflowState_AfterReset(t *testing.T) {
	dir, err := os.MkdirTemp("", "m31a-reset-wf-*")
	if err != nil {
		t.Fatalf("Failed to create temp dir: %v", err)
	}
	defer os.RemoveAll(dir)

	mgr := NewManager(dir, ManagerOpts{})
	s, err := mgr.NewSession("gpt-4o", "openrouter")
	if err != nil {
		t.Fatalf("NewSession failed: %v", err)
	}

	// Set state
	if err := mgr.UpdateWorkflowState(s.ID, "build it", types.PhaseExecute, []string{"Q1"}); err != nil {
		t.Fatalf("UpdateWorkflowState failed: %v", err)
	}

	// Reset to idle (post-Ship)
	if err := mgr.UpdateWorkflowState(s.ID, "", types.PhaseIdle, nil); err != nil {
		t.Fatalf("reset UpdateWorkflowState failed: %v", err)
	}

	// Load back
	goal, phase, _, err := mgr.LoadWorkflowState(s.ID)
	if err != nil {
		t.Fatalf("LoadWorkflowState failed: %v", err)
	}
	if goal != "" {
		t.Errorf("expected empty goal after reset, got %q", goal)
	}
	if phase != types.PhaseIdle {
		t.Errorf("expected phase Idle after reset, got %s", phase)
	}
}

// TestManager_UpdateWorkflowState_NonexistentSession verifies that
// the method returns an error (not a panic) when the session
// doesn't exist on disk. This is the failure mode that the
// /workflow resume error path relies on.
func TestManager_UpdateWorkflowState_NonexistentSession(t *testing.T) {
	dir, err := os.MkdirTemp("", "m31a-update-wf-missing-*")
	if err != nil {
		t.Fatalf("Failed to create temp dir: %v", err)
	}
	defer os.RemoveAll(dir)

	mgr := NewManager(dir, ManagerOpts{})

	// Try to update a session that doesn't exist
	err = mgr.UpdateWorkflowState("nonexistent", "goal", types.PhasePlan, nil)
	if err == nil {
		t.Error("expected error for nonexistent session, got nil")
	}
}
