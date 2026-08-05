package workflow

import (
	"encoding/json"
	"fmt"
	"log/slog"
	"os"
	"path/filepath"
	"time"

	"github.com/eshanized/M31A/internal/core/types"
	"github.com/eshanized/M31A/internal/infrastructure/fileutil"
)

// RecoveryState contains a snapshot of workflow state for crash recovery.
// It captures all state needed to resume a workflow after a crash or forced exit.
type RecoveryState struct {
	SessionID    string                   `json:"session_id"`
	CurrentPhase types.WorkflowPhase      `json:"current_phase"`
	PhaseHistory []types.WorkflowPhase    `json:"phase_history"`
	PlanMarkdown string                   `json:"plan_markdown"`
	PlanVersion  int                      `json:"plan_version"`
	Messages     []types.Message          `json:"messages"`
	Checkpoint   *CheckpointData          `json:"checkpoint,omitempty"`
	Goal         string                   `json:"goal"`
	Timestamp    time.Time                `json:"timestamp"`
}

// recoveryPath returns the path to the recovery file for the given session directory.
func recoveryPath(sessionDir string) string {
	return filepath.Join(sessionDir, "recovery.json")
}

// SaveRecoveryState snapshots the engine's workflow state and writes it atomically
// to the recovery file. This is crash-safe: a concurrent crash mid-write leaves
// the existing recovery file intact (via AtomicWrite temp-file + rename).
func SaveRecoveryState(engine *Engine, path string) error {
	if path == "" {
		return fmt.Errorf("recovery path is empty: %w", os.ErrInvalid)
	}

	// Snapshot state under appropriate locks
	state := &RecoveryState{
		SessionID:    engine.sessionID,
		CurrentPhase: engine.stateMachine.CurrentPhase(),
		PhaseHistory: engine.stateMachine.History(),
		PlanMarkdown: engine.state.PlanContent(),
		PlanVersion:  engine.state.PlanVersion(),
		Messages:     engine.state.MessagesSnapshot(),
		Checkpoint:   engine.state.CheckpointData(),
		Goal:         engine.state.CurrentGoal(),
		Timestamp:    time.Now(),
	}

	data, err := json.Marshal(state)
	if err != nil {
		return fmt.Errorf("marshal recovery state: %w", err)
	}

	// Ensure the parent directory exists
	dir := filepath.Dir(path)
	if err := os.MkdirAll(dir, 0o755); err != nil {
		return fmt.Errorf("create recovery directory: %w", err)
	}

	if err := fileutil.AtomicWrite(path, data); err != nil {
		return fmt.Errorf("write recovery file: %w", err)
	}

	return nil
}

// LoadRecoveryState reads and deserializes a recovery state file.
// Returns an error if the file is missing, corrupted, or fails validation.
func LoadRecoveryState(path string) (*RecoveryState, error) {
	if path == "" {
		return nil, fmt.Errorf("recovery path is empty: %w", os.ErrInvalid)
	}

	data, err := os.ReadFile(path)
	if err != nil {
		if os.IsNotExist(err) {
			return nil, fmt.Errorf("recovery file not found: %w", err)
		}
		return nil, fmt.Errorf("read recovery file: %w", err)
	}

	var state RecoveryState
	if err := json.Unmarshal(data, &state); err != nil {
		return nil, fmt.Errorf("unmarshal recovery state: %w", err)
	}

	if err := ValidateRecoveryState(&state); err != nil {
		return nil, fmt.Errorf("validate recovery state: %w", err)
	}

	return &state, nil
}

// ValidateRecoveryState checks that a recovery state is internally consistent.
// It verifies the phase is valid, history is non-empty and starts with Idle,
// and the timestamp is within reasonable bounds.
func ValidateRecoveryState(state *RecoveryState) error {
	if state == nil {
		return fmt.Errorf("recovery state is nil")
	}

	if state.SessionID == "" {
		return fmt.Errorf("recovery state missing session ID")
	}

	// Verify CurrentPhase is a valid phase
	validPhases := map[types.WorkflowPhase]bool{
		types.PhaseIdle:       true,
		types.PhaseInitialize: true,
		types.PhaseDiscuss:    true,
		types.PhasePlan:       true,
		types.PhaseExecute:    true,
		types.PhaseVerify:     true,
		types.PhaseRuntime:    true,
		types.PhaseShip:       true,
	}
	if !validPhases[state.CurrentPhase] {
		return fmt.Errorf("invalid recovery phase %q", state.CurrentPhase)
	}

	// Verify PhaseHistory is non-empty and starts with Idle
	if len(state.PhaseHistory) == 0 {
		return fmt.Errorf("recovery state has empty phase history")
	}
	if state.PhaseHistory[0] != types.PhaseIdle {
		return fmt.Errorf("recovery phase history does not start with Idle, starts with %q", state.PhaseHistory[0])
	}

	// Verify timestamp is within reasonable bounds (not far future/past)
	now := time.Now()
	maxDrift := 7 * 24 * time.Hour // 7 days
	if state.Timestamp.After(now.Add(maxDrift)) {
		return fmt.Errorf("recovery timestamp is in the future: %v", state.Timestamp)
	}
	if state.Timestamp.Before(now.Add(-maxDrift)) {
		return fmt.Errorf("recovery timestamp is too old: %v", state.Timestamp)
	}

	return nil
}

// RollbackToLastCheckpoint loads the last recovery state and restores the engine
// to that state. This transitions the state machine back and restores plan/messages.
// Returns an error if no valid recovery state exists or rollback is not possible.
func RollbackToLastCheckpoint(engine *Engine) error {
	if engine.recoveryPath == "" {
		return fmt.Errorf("recovery path not configured")
	}

	state, err := LoadRecoveryState(engine.recoveryPath)
	if err != nil {
		return fmt.Errorf("load recovery state for rollback: %w", err)
	}

	// Verify we can actually go back — must have a valid phase in history
	if len(state.PhaseHistory) < 2 {
		return fmt.Errorf("cannot rollback: recovery history has only %d entries", len(state.PhaseHistory))
	}

	// The previous phase is the second-to-last in history
	previousPhase := state.PhaseHistory[len(state.PhaseHistory)-2]

	// Restore engine state
	engine.state.SetPlanContent(state.PlanMarkdown)
	engine.state.SetPlanVersion(state.PlanVersion)
	engine.state.SetMessages(state.Messages)
	engine.state.SetCurrentGoal(state.Goal)
	engine.state.SetCheckpointData(state.Checkpoint)

	// Transition state machine to the recovered phase
	engine.stateMachine.SetPhase(state.CurrentPhase)

	slog.Info("rolled back to last checkpoint",
		"phase", state.CurrentPhase,
		"previous_phase", previousPhase,
		"goal", truncateForLog(state.Goal, 100),
	)

	return nil
}
