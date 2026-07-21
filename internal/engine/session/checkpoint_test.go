package session

import (
	"os"
	"testing"
	"time"

	m31errors "github.com/eshanized/M31A/internal/errors"
	"github.com/eshanized/M31A/internal/types"
)

func TestCheckpoint_SaveAndLoad(t *testing.T) {
	mgr, dir := newTestManager(t)
	defer os.RemoveAll(dir)

	s, err := mgr.NewSession("test-model", "test-provider")
	if err != nil {
		t.Fatalf("NewSession failed: %v", err)
	}

	cp := Checkpoint{
		Phase:        types.PhasePlan,
		Timestamp:    time.Now().Truncate(time.Second), // truncate for comparison
		MessageCount: 42,
		TaskCount:    7,
	}

	if saveErr := mgr.SaveCheckpoint(s.ID, cp); saveErr != nil {
		t.Fatalf("SaveCheckpoint failed: %v", saveErr)
	}

	loaded, err := mgr.LoadCheckpoints(s.ID)
	if err != nil {
		t.Fatalf("LoadCheckpoints failed: %v", err)
	}
	if len(loaded) != 1 {
		t.Fatalf("Expected 1 checkpoint, got %d", len(loaded))
	}

	if loaded[0].Phase != cp.Phase {
		t.Errorf("Phase: expected %q, got %q", cp.Phase, loaded[0].Phase)
	}
	if !loaded[0].Timestamp.Equal(cp.Timestamp) {
		t.Errorf("Timestamp: expected %v, got %v", cp.Timestamp, loaded[0].Timestamp)
	}
	if loaded[0].MessageCount != cp.MessageCount {
		t.Errorf("MessageCount: expected %d, got %d", cp.MessageCount, loaded[0].MessageCount)
	}
	if loaded[0].TaskCount != cp.TaskCount {
		t.Errorf("TaskCount: expected %d, got %d", cp.TaskCount, loaded[0].TaskCount)
	}
}

func TestCheckpoint_MaxRetention(t *testing.T) {
	mgr, dir := newTestManager(t)
	defer os.RemoveAll(dir)

	s, err := mgr.NewSession("test-model", "test-provider")
	if err != nil {
		t.Fatalf("NewSession failed: %v", err)
	}

	// Save 3 checkpoints in sequence with different phases
	phases := []types.WorkflowPhase{
		types.PhaseIdle,
		types.PhaseDiscuss,
		types.PhasePlan,
	}
	for i := 0; i < 3; i++ {
		cp := Checkpoint{
			Phase:        phases[i],
			Timestamp:    time.Now().Add(time.Duration(i) * time.Second),
			MessageCount: i * 10,
			TaskCount:    i,
		}
		if saveErr := mgr.SaveCheckpoint(s.ID, cp); saveErr != nil {
			t.Fatalf("SaveCheckpoint %d failed: %v", i, saveErr)
		}
	}

	loaded, err := mgr.LoadCheckpoints(s.ID)
	if err != nil {
		t.Fatalf("LoadCheckpoints failed: %v", err)
	}
	if len(loaded) != 2 {
		t.Fatalf("Expected 2 checkpoints (trimmed from 3), got %d", len(loaded))
	}

	// Should have kept the 2 newest (Discuss and Plan), newest-first
	expectedPhases := []types.WorkflowPhase{types.PhasePlan, types.PhaseDiscuss}
	for i, cp := range loaded {
		if cp.Phase != expectedPhases[i] {
			t.Errorf("Checkpoint %d Phase: expected %q, got %q", i, expectedPhases[i], cp.Phase)
		}
	}

	// Verify order: newest first (Plan=2, Discuss=1)
	if loaded[0].TaskCount != 2 || loaded[1].TaskCount != 1 {
		t.Errorf("Expected newest [plan] first, oldest [discuss] last. Got TaskCounts: %d, %d",
			loaded[0].TaskCount, loaded[1].TaskCount)
	}
}

func TestCheckpoint_Latest(t *testing.T) {
	mgr, dir := newTestManager(t)
	defer os.RemoveAll(dir)

	s, err := mgr.NewSession("test-model", "test-provider")
	if err != nil {
		t.Fatalf("NewSession failed: %v", err)
	}

	// Save two checkpoints
	cp1 := Checkpoint{
		Phase:        types.PhaseDiscuss,
		Timestamp:    time.Now().Add(-time.Hour),
		MessageCount: 10,
		TaskCount:    1,
	}
	cp2 := Checkpoint{
		Phase:        types.PhasePlan,
		Timestamp:    time.Now(),
		MessageCount: 25,
		TaskCount:    5,
	}

	if saveErr := mgr.SaveCheckpoint(s.ID, cp1); saveErr != nil {
		t.Fatalf("SaveCheckpoint 1 failed: %v", saveErr)
	}
	if saveErr := mgr.SaveCheckpoint(s.ID, cp2); saveErr != nil {
		t.Fatalf("SaveCheckpoint 2 failed: %v", saveErr)
	}

	latest, err := mgr.LatestCheckpoint(s.ID)
	if err != nil {
		t.Fatalf("LatestCheckpoint failed: %v", err)
	}
	if latest == nil {
		t.Fatal("Expected non-nil checkpoint")
	}

	if latest.Phase != types.PhasePlan {
		t.Errorf("Expected PhasePlan (latest), got %q", latest.Phase)
	}
	if latest.MessageCount != 25 {
		t.Errorf("Expected MessageCount 25, got %d", latest.MessageCount)
	}
	if latest.TaskCount != 5 {
		t.Errorf("Expected TaskCount 5, got %d", latest.TaskCount)
	}
}

func TestCheckpoint_EmptyState(t *testing.T) {
	mgr, dir := newTestManager(t)
	defer os.RemoveAll(dir)

	s, err := mgr.NewSession("test-model", "test-provider")
	if err != nil {
		t.Fatalf("NewSession failed: %v", err)
	}

	// No checkpoints saved yet
	_, err = mgr.LatestCheckpoint(s.ID)
	if err != m31errors.ErrCheckpointNotFound {
		t.Errorf("Expected ErrCheckpointNotFound, got %v", err)
	}

	// LoadCheckpoints should return empty slice
	cps, err := mgr.LoadCheckpoints(s.ID)
	if err != nil {
		t.Fatalf("LoadCheckpoints on empty state should not error, got: %v", err)
	}
	if cps == nil {
		t.Error("Expected empty slice, got nil")
	}
	if len(cps) != 0 {
		t.Errorf("Expected 0 checkpoints, got %d", len(cps))
	}
}
