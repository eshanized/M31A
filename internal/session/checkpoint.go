package session

import (
	"encoding/json"
	"fmt"
	"log/slog"
	"os"
	"path/filepath"
	"sort"
	"time"

	m31errors "github.com/eshanized/M31A/internal/errors"
	"github.com/eshanized/M31A/internal/types"
)

// Checkpoint represents a point-in-time snapshot of workflow state, used by
// the /undo command to restore previous workflow phases.
type Checkpoint struct {
	Phase        types.WorkflowPhase `json:"phase"`
	Timestamp    time.Time           `json:"timestamp"`
	MessageCount int                 `json:"message_count"`
	TaskCount    int                 `json:"task_count"`
	Goal         string              `json:"goal,omitempty"`
	PlanVersion  int                 `json:"plan_version,omitempty"`
}

// SaveCheckpoint appends a checkpoint to checkpoint.json for the given session.
// If more than 2 checkpoints exist, the oldest are trimmed (only last 2 retained).
// Writes are atomic (temp + rename) per M-20.
func (m *Manager) SaveCheckpoint(sessionID string, cp Checkpoint) error {
	path := filepath.Join(m.projectDir(), "checkpoint.json")

	// Read existing checkpoints directly (not via LoadCheckpoints which sorts)
	existing, err := m.loadCheckpointsRaw(sessionID)
	if err != nil {
		return err
	}

	// Append new checkpoint
	existing = append(existing, cp)

	// Trim to max 2 (keep newest — last 2 in chronological order)
	if len(existing) > 2 {
		existing = existing[len(existing)-2:]
	}

	// Marshal and write atomically
	data, err := json.Marshal(existing)
	if err != nil {
		return fmt.Errorf("cannot marshal checkpoints: %w", err)
	}

	return m.atomicWrite(path, data)
}

// loadCheckpointsRaw reads checkpoints from disk in file order (chronological).
// Used internally by SaveCheckpoint for read-trim-write without sorting.
func (m *Manager) loadCheckpointsRaw(sessionID string) ([]Checkpoint, error) {
	path := filepath.Join(m.projectDir(), "checkpoint.json")
	data, err := readFileLimited(path, types.MaxSessionFileSize)
	if err != nil {
		if os.IsNotExist(err) {
			return []Checkpoint{}, nil
		}
		return nil, fmt.Errorf("cannot read checkpoint.json: %w", err)
	}

	var checkpoints []Checkpoint
	if err := json.Unmarshal(data, &checkpoints); err != nil {
		return nil, fmt.Errorf("cannot unmarshal checkpoints: %w", err)
	}

	return checkpoints, nil
}

// LoadCheckpoints reads all checkpoints from checkpoint.json for the given session.
// Returns an empty slice without error if the file does not exist.
// Prunes old checkpoints, keeping only the 2 most recent.
func (m *Manager) LoadCheckpoints(sessionID string) ([]Checkpoint, error) {
	path := filepath.Join(m.projectDir(), "checkpoint.json")
	data, err := readFileLimited(path, types.MaxSessionFileSize)
	if err != nil {
		if os.IsNotExist(err) {
			return []Checkpoint{}, nil
		}
		return nil, fmt.Errorf("cannot read checkpoint.json: %w", err)
	}

	var checkpoints []Checkpoint
	if err := json.Unmarshal(data, &checkpoints); err != nil {
		return nil, fmt.Errorf("cannot unmarshal checkpoints: %w", err)
	}

	// Sort by timestamp descending (newest first) and prune to max 2.
	sort.Slice(checkpoints, func(i, j int) bool {
		return checkpoints[i].Timestamp.After(checkpoints[j].Timestamp)
	})

	if len(checkpoints) > 2 {
		checkpoints = checkpoints[:2]
		// Rewrite the file to persist the pruned set
		data, err := json.Marshal(checkpoints)
		if err == nil {
			if writeErr := m.atomicWrite(path, data); writeErr != nil {
				slog.Warn("failed to persist pruned checkpoints", "error", writeErr)
			}
		}
	}

	return checkpoints, nil
}

// LatestCheckpoint returns the most recently saved checkpoint.
// Returns ErrCheckpointNotFound if no checkpoints exist.
func (m *Manager) LatestCheckpoint(sessionID string) (*Checkpoint, error) {
	checkpoints, err := m.LoadCheckpoints(sessionID)
	if err != nil {
		return nil, err
	}

	if len(checkpoints) == 0 {
		return nil, m31errors.ErrCheckpointNotFound
	}

	// LoadCheckpoints returns newest-first (sorted by timestamp descending)
	return &checkpoints[0], nil
}
