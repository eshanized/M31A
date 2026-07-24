package workflow

import (
	"context"
	"log/slog"
	"os"
	"path/filepath"
	"testing"
	"time"

	m31types "github.com/eshanized/M31A/internal/core/types"
	"github.com/eshanized/M31A/internal/engine/session"
	"github.com/eshanized/M31A/tests/testutil/mocks"
)

// mockEmitFn captures emitted messages for testing.
type mockEmitFn struct {
	messages []any
}

func (m *mockEmitFn) emit(msg any) {
	m.messages = append(m.messages, msg)
}

func setupTestPhaseCoordinator(t *testing.T) (*PhaseCoordinator, *mockEmitFn) {
	t.Helper()
	sm := NewStateMachine()
	cache := NewWorkflowCache()
	tmpDir := t.TempDir()
	sessionBaseDir := filepath.Join(tmpDir, "sessions")
	os.MkdirAll(sessionBaseDir, 0755)
	// Create .m31a directory that session manager expects
	os.MkdirAll(filepath.Join(tmpDir, ".m31a"), 0755)
	sessionMgr := session.NewManager(sessionBaseDir, tmpDir, session.ManagerOpts{})
	costTracker := NewCostTracker(0)
	emitter := &mockEmitFn{}
	logger := slog.Default()

	pc := NewPhaseCoordinator(sm, cache, sessionMgr, "test-session", costTracker, nil, &mocks.WorkflowDispatcher{}, logger, emitter.emit)
	return pc, emitter
}

func TestNewPhaseCoordinator(t *testing.T) {
	pc, _ := setupTestPhaseCoordinator(t)

	if pc == nil {
		t.Fatal("expected non-nil PhaseCoordinator")
	}
	if pc.stateMachine == nil {
		t.Error("expected stateMachine to be set")
	}
	if pc.cache == nil {
		t.Error("expected cache to be set")
	}
}

func TestPhaseCoordinator_PrePhaseSetup_BudgetCheck(t *testing.T) {
	sm := NewStateMachine()
	cache := NewWorkflowCache()
	tmpDir := t.TempDir()
	sessionBaseDir := filepath.Join(tmpDir, "sessions")
	os.MkdirAll(sessionBaseDir, 0755)
	sessionMgr := session.NewManager(sessionBaseDir, sessionBaseDir, session.ManagerOpts{})
	costTracker := NewCostTracker(1.0)
	costTracker.RecordCost(0.5)
	emitter := &mockEmitFn{}
	logger := slog.Default()

	pc := NewPhaseCoordinator(sm, cache, sessionMgr, "test-session", costTracker, nil, &mocks.WorkflowDispatcher{}, logger, emitter.emit)

	// Test with budget under limit
	cfg := &testConfig{budgetLimit: 1.0}
	messages := []m31types.Message{{Role: "user", Content: "test"}}
	result, err := pc.PrePhaseSetup(context.TODO(), m31types.PhaseInitialize, cfg, messages, nil)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if len(result) != 1 {
		t.Errorf("expected 1 message, got %d", len(result))
	}

	// Test with budget exceeded
	costTracker.RecordCost(0.6) // Now total is 1.1, exceeding limit
	_, err = pc.PrePhaseSetup(context.TODO(), m31types.PhaseInitialize, cfg, messages, nil)
	if err == nil {
		t.Error("expected error for budget exceeded")
	}
}

func TestPhaseCoordinator_PrePhaseSetup_BatchApprovalRevocation(t *testing.T) {
	pc, _ := setupTestPhaseCoordinator(t)
	pc.dispatcher = &mocks.WorkflowDispatcher{}

	messages := []m31types.Message{{Role: "user", Content: "test"}}
	_, err := pc.PrePhaseSetup(context.TODO(), m31types.PhaseInitialize, nil, messages, nil)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}

	if !pc.dispatcher.(*mocks.WorkflowDispatcher).Revoked {
		t.Error("expected batch approvals to be revoked")
	}
}

func TestPhaseCoordinator_PostPhaseExecution(t *testing.T) {
	pc, _ := setupTestPhaseCoordinator(t)

	start := time.Now()
	result := &PhaseResult{
		Success: true,
		Cost:    0.1,
	}

	pc.PostPhaseExecution(m31types.PhaseInitialize, result, start)

	if result.Phase != m31types.PhaseInitialize {
		t.Errorf("expected phase to be set to PhaseInitialize, got %s", result.Phase)
	}
	if result.DurationMs < 0 {
		t.Error("expected non-negative duration")
	}
}

func TestPhaseCoordinator_CoordinateTransition(t *testing.T) {
	pc, emitter := setupTestPhaseCoordinator(t)

	err := pc.CoordinateTransition(context.TODO(), m31types.PhaseIdle, m31types.PhaseInitialize, "test goal", 1)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}

	// Verify checkpoint was saved
	checkpoints, err := pc.sessionMgr.LoadCheckpoints("test-session")
	if err != nil {
		t.Fatalf("unexpected error loading checkpoints: %v", err)
	}
	if len(checkpoints) != 1 {
		t.Errorf("expected 1 checkpoint, got %d", len(checkpoints))
	}
	if checkpoints[0].Phase != m31types.PhaseInitialize {
		t.Errorf("expected checkpoint phase to be PhaseInitialize, got %s", checkpoints[0].Phase)
	}

	// Verify messages were emitted
	if len(emitter.messages) != 2 {
		t.Errorf("expected 2 messages, got %d", len(emitter.messages))
	}
}

type testConfig struct {
	budgetLimit float64
}

func (c *testConfig) GetBudgetLimit() float64 {
	return c.budgetLimit
}

func TestPhaseCoordinator_ModelForPhase(t *testing.T) {
	pc, _ := setupTestPhaseCoordinator(t)

	// Should return empty string (placeholder implementation)
	model := pc.ModelForPhase(m31types.PhaseExecute)
	if model != "" {
		t.Errorf("ModelForPhase should return empty string, got %q", model)
	}
}

func TestPhaseCoordinator_ProviderForPhase(t *testing.T) {
	pc, _ := setupTestPhaseCoordinator(t)

	// Should return nil (placeholder implementation)
	provider := pc.ProviderForPhase(m31types.PhaseExecute)
	if provider != nil {
		t.Error("ProviderForPhase should return nil")
	}
}

func TestPhaseCoordinator_Emit(t *testing.T) {
	pc, emitter := setupTestPhaseCoordinator(t)

	// Emit a message
	pc.Emit("test message")

	if len(emitter.messages) != 1 {
		t.Errorf("expected 1 emitted message, got %d", len(emitter.messages))
	}
}

func TestPhaseCoordinator_Logger(t *testing.T) {
	pc, _ := setupTestPhaseCoordinator(t)

	logger := pc.Logger()
	if logger == nil {
		t.Error("Logger should return non-nil logger")
	}
}
