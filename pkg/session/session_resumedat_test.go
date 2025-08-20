package session

import (
	"encoding/json"
	"errors"
	"os"
	"path/filepath"
	"sync"
	"testing"
	"time"

	m31errors "github.com/eshanized/M31A/internal/errors"
	"github.com/eshanized/M31A/internal/types"
)

// ---------------------------------------------------------------------------
// M-6: ResumedAt field
// ---------------------------------------------------------------------------

// TestSession_ResumedAt_FirstLoadSetsNow verifies that LoadSession sets
// ResumedAt to a recent timestamp on the first load.
func TestSession_ResumedAt_FirstLoadSetsNow(t *testing.T) {
	mgr, dir := newTestManager(t)
	defer os.RemoveAll(dir)

	s, err := mgr.NewSession("gpt-4o", "openrouter")
	if err != nil {
		t.Fatalf("NewSession failed: %v", err)
	}

	before := time.Now()
	loaded, err := mgr.LoadSession(s.ID)
	if err != nil {
		t.Fatalf("LoadSession failed: %v", err)
	}
	after := time.Now()

	if loaded.ResumedAt == nil {
		t.Fatal("Expected ResumedAt to be set on first load")
	}
	if loaded.ResumedAt.Before(before) || loaded.ResumedAt.After(after) {
		t.Errorf("ResumedAt %v should be between %v and %v", loaded.ResumedAt, before, after)
	}
}

// TestSession_ResumedAt_UpdatesOnSecondLoad verifies that loading the same
// session twice produces a ResumedAt that advances.
func TestSession_ResumedAt_UpdatesOnSecondLoad(t *testing.T) {
	mgr, dir := newTestManager(t)
	defer os.RemoveAll(dir)

	s, err := mgr.NewSession("gpt-4o", "openrouter")
	if err != nil {
		t.Fatalf("NewSession failed: %v", err)
	}

	loaded1, err := mgr.LoadSession(s.ID)
	if err != nil {
		t.Fatalf("LoadSession 1 failed: %v", err)
	}

	time.Sleep(100 * time.Millisecond)

	loaded2, err := mgr.LoadSession(s.ID)
	if err != nil {
		t.Fatalf("LoadSession 2 failed: %v", err)
	}

	if loaded1.ResumedAt == nil || loaded2.ResumedAt == nil {
		t.Fatal("Expected ResumedAt to be set on both loads")
	}
	if !loaded2.ResumedAt.After(*loaded1.ResumedAt) {
		t.Errorf("Expected second ResumedAt %v to be after first %v", loaded2.ResumedAt, loaded1.ResumedAt)
	}
}

// TestSession_ResumedAt_Persisted verifies that ResumedAt survives a
// save-load round trip.
func TestSession_ResumedAt_Persisted(t *testing.T) {
	mgr, dir := newTestManager(t)
	defer os.RemoveAll(dir)

	s, err := mgr.NewSession("gpt-4o", "openrouter")
	if err != nil {
		t.Fatalf("NewSession failed: %v", err)
	}

	// Load to set ResumedAt
	loaded, err := mgr.LoadSession(s.ID)
	if err != nil {
		t.Fatalf("LoadSession failed: %v", err)
	}

	// Save the session (ResumedAt should be persisted)
	if err := mgr.SaveSession(loaded); err != nil {
		t.Fatalf("SaveSession failed: %v", err)
	}

	// Reload and check ResumedAt is still present
	reloaded, err := mgr.LoadSession(s.ID)
	if err != nil {
		t.Fatalf("LoadSession after save failed: %v", err)
	}

	if reloaded.ResumedAt == nil {
		t.Fatal("Expected ResumedAt to survive save-load round trip")
	}
}

// TestSession_ResumedAt_OmittedOnNewSession verifies that a freshly created
// session has nil ResumedAt (omitempty in JSON).
func TestSession_ResumedAt_OmittedOnNewSession(t *testing.T) {
	s := NewSession("test-id", "gpt-4o", "openrouter")
	if s.ResumedAt != nil {
		t.Error("Expected ResumedAt to be nil on new session")
	}

	// Verify JSON omitempty behavior
	data, err := json.Marshal(s)
	if err != nil {
		t.Fatalf("Marshal failed: %v", err)
	}
	var decoded map[string]interface{}
	if err := json.Unmarshal(data, &decoded); err != nil {
		t.Fatalf("Unmarshal failed: %v", err)
	}
	if _, exists := decoded["resumed_at"]; exists {
		t.Error("Expected resumed_at to be omitted from JSON when nil")
	}
}

// ---------------------------------------------------------------------------
// L-13: Session ID validation
// ---------------------------------------------------------------------------

// TestSession_IDValidation_RejectsBadFormat verifies that invalid session IDs
// are rejected by validateSessionID.
func TestSession_IDValidation_RejectsBadFormat(t *testing.T) {
	badIDs := []struct {
		name string
		id   string
	}{
		{"too short", "abc"},
		{"too long", "abcdefghijklmnop"},
		{"uppercase", "ABCDEF12"},
		{"special chars", "abc-def!"},
		{"spaces", "abc defg"},
		{"empty", ""},
		{"unicode", "café1234"},
	}

	for _, tt := range badIDs {
		t.Run(tt.name, func(t *testing.T) {
			err := validateSessionID(tt.id, types.SessionIDLength)
			if err == nil {
				t.Errorf("validateSessionID(%q) should return error, got nil", tt.id)
			}
		})
	}
}

// TestSession_IDValidation_AcceptsValidFormat verifies that valid hex session
// IDs are accepted.
func TestSession_IDValidation_AcceptsValidFormat(t *testing.T) {
	validIDs := []string{
		"abcdef12",
		"00000000",
		"ffffffff",
		"deadbeef",
		"12345678",
	}

	for _, id := range validIDs {
		t.Run(id, func(t *testing.T) {
			if err := validateSessionID(id, types.SessionIDLength); err != nil {
				t.Errorf("validateSessionID(%q) should succeed, got %v", id, err)
			}
		})
	}
}

// TestSession_LoadRejectsBadID verifies that LoadSession rejects invalid IDs.
func TestSession_LoadRejectsBadID(t *testing.T) {
	mgr, dir := newTestManager(t)
	defer os.RemoveAll(dir)

	_, err := mgr.LoadSession("BadID!")
	if err == nil {
		t.Error("Expected LoadSession to reject invalid ID format")
	}
}

// ---------------------------------------------------------------------------
// Atomic writes (M-18, M-19, M-20) — concurrency safety
// ---------------------------------------------------------------------------

// TestSession_AtomicWrites verifies that concurrent saves and reads don't
// produce partial JSON.
func TestSession_AtomicWrites(t *testing.T) {
	mgr, dir := newTestManager(t)
	defer os.RemoveAll(dir)

	s, err := mgr.NewSession("gpt-4o", "openrouter")
	if err != nil {
		t.Fatalf("NewSession failed: %v", err)
	}

	var wg sync.WaitGroup
	errs := make(chan error, 20)

	// Concurrent writers — each goroutine creates its own copy to avoid races
	for i := 0; i < 5; i++ {
		wg.Add(1)
		go func(n int) {
			defer wg.Done()
			// Load a fresh copy to avoid data race on shared struct
			loaded, loadErr := mgr.LoadSession(s.ID)
			if loadErr != nil {
				errs <- loadErr
				return
			}
			loaded.MessageCount = n
			if saveErr := mgr.SaveSession(loaded); saveErr != nil {
				errs <- saveErr
			}
		}(i)
	}

	// Concurrent readers
	for i := 0; i < 5; i++ {
		wg.Add(1)
		go func() {
			defer wg.Done()
			loaded, loadErr := mgr.LoadSession(s.ID)
			if loadErr != nil {
				errs <- loadErr
				return
			}
			if loaded.ID == "" {
				errs <- errors.New("loaded session has empty ID")
			}
		}()
	}

	wg.Wait()
	close(errs)

	for err := range errs {
		t.Errorf("Concurrent operation error: %v", err)
	}
}

// TestPlanning_AtomicWrites verifies that concurrent planning file writes
// don't corrupt data.
func TestPlanning_AtomicWrites(t *testing.T) {
	mgr, dir := newTestManager(t)
	defer os.RemoveAll(dir)

	s, err := mgr.NewSession("gpt-4o", "openrouter")
	if err != nil {
		t.Fatalf("NewSession failed: %v", err)
	}

	proj := &types.ProjectState{
		Goal:        "test goal",
		ProjectType: "go",
		Framework:   "fiber",
		Answers:     map[string]string{"lang": "go"},
	}

	var wg sync.WaitGroup
	errs := make(chan error, 10)

	// Concurrent project writes
	for i := 0; i < 5; i++ {
		wg.Add(1)
		go func() {
			defer wg.Done()
			if err := mgr.SaveProject(s.ID, proj); err != nil {
				errs <- err
			}
		}()
	}

	wg.Wait()
	close(errs)

	for err := range errs {
		t.Errorf("Concurrent SaveProject error: %v", err)
	}

	// Verify file is valid
	loaded, err := mgr.LoadProject(s.ID)
	if err != nil {
		t.Fatalf("LoadProject failed: %v", err)
	}
	if loaded.Goal != "test goal" {
		t.Errorf("Expected goal 'test goal', got %q", loaded.Goal)
	}
}

// ---------------------------------------------------------------------------
// M-29: Bisect reset error sentinel (definition check)
// ---------------------------------------------------------------------------

// TestBisect_ResetFailure_Typed verifies that ErrBisectResetFailed is defined.
func TestBisect_ResetFailure_Typed(t *testing.T) {
	err := m31errors.ErrBisectResetFailed
	if err == nil {
		t.Fatal("ErrBisectResetFailed should not be nil")
	}
	if err.Error() != "bisect reset failed" {
		t.Errorf("Expected error message 'bisect reset failed', got %q", err.Error())
	}
}

// ---------------------------------------------------------------------------
// L-16: Checkpoint retention
// ---------------------------------------------------------------------------

// TestCheckpoint_Retention_LastTwoOnly verifies that saving more than 2
// checkpoints prunes the oldest ones.
func TestCheckpoint_Retention_LastTwoOnly(t *testing.T) {
	mgr, dir := newTestManager(t)
	defer os.RemoveAll(dir)

	s, err := mgr.NewSession("gpt-4o", "openrouter")
	if err != nil {
		t.Fatalf("NewSession failed: %v", err)
	}

	// Save 3 checkpoints with distinct timestamps
	for i := 0; i < 3; i++ {
		cp := Checkpoint{
			Phase:        types.PhasePlan,
			Timestamp:    time.Now().Add(time.Duration(i) * time.Second),
			MessageCount: i,
			TaskCount:    i,
		}
		if err := mgr.SaveCheckpoint(s.ID, cp); err != nil {
			t.Fatalf("SaveCheckpoint %d failed: %v", i, err)
		}
		time.Sleep(10 * time.Millisecond) // ensure distinct timestamps
	}

	// Load checkpoints — should have at most 2
	checkpoints, err := mgr.LoadCheckpoints(s.ID)
	if err != nil {
		t.Fatalf("LoadCheckpoints failed: %v", err)
	}
	if len(checkpoints) > 2 {
		t.Errorf("Expected at most 2 checkpoints, got %d", len(checkpoints))
	}

	// Verify the 2 kept are the newest
	if len(checkpoints) == 2 {
		if checkpoints[0].Timestamp.Before(checkpoints[1].Timestamp) {
			t.Error("Expected checkpoints to be sorted newest-first")
		}
	}
}

// TestCheckpoint_Retention_FilePruned verifies that the checkpoint.json file
// is rewritten with only 2 entries after LoadCheckpoints prunes.
func TestCheckpoint_Retention_FilePruned(t *testing.T) {
	mgr, dir := newTestManager(t)
	defer os.RemoveAll(dir)

	s, err := mgr.NewSession("gpt-4o", "openrouter")
	if err != nil {
		t.Fatalf("NewSession failed: %v", err)
	}

	// Save 3 checkpoints
	for i := 0; i < 3; i++ {
		cp := Checkpoint{
			Phase:        types.PhaseExecute,
			Timestamp:    time.Now().Add(time.Duration(i) * time.Second),
			MessageCount: i,
			TaskCount:    i,
		}
		if err := mgr.SaveCheckpoint(s.ID, cp); err != nil {
			t.Fatalf("SaveCheckpoint %d failed: %v", i, err)
		}
	}

	// Load (triggers pruning)
	checkpoints, err := mgr.LoadCheckpoints(s.ID)
	if err != nil {
		t.Fatalf("LoadCheckpoints failed: %v", err)
	}

	// Re-read the file directly to verify it was pruned on disk
	path := filepath.Join(mgr.basePathFor(s.ID), "checkpoint.json")
	data, err := os.ReadFile(path)
	if err != nil {
		t.Fatalf("ReadFile failed: %v", err)
	}

	var fileCheckpoints []Checkpoint
	if err := json.Unmarshal(data, &fileCheckpoints); err != nil {
		t.Fatalf("Unmarshal failed: %v", err)
	}

	if len(fileCheckpoints) > 2 {
		t.Errorf("Expected file to contain at most 2 checkpoints after pruning, got %d", len(fileCheckpoints))
	}
	if len(fileCheckpoints) != len(checkpoints) {
		t.Errorf("File count %d != loaded count %d", len(fileCheckpoints), len(checkpoints))
	}
}
