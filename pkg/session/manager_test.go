package session

import (
	"errors"
	"os"
	"testing"
	"time"

	m31errors "github.com/eshanized/M31A/internal/errors"
	"github.com/eshanized/M31A/internal/types"
)

// newTestManager creates a Manager backed by a temp directory.
// The caller must call os.RemoveAll on the returned dir string.
func newTestManager(t *testing.T) (*Manager, string) {
	t.Helper()
	dir, err := os.MkdirTemp("", "m31a-session-test-*")
	if err != nil {
		t.Fatalf("Failed to create temp dir: %v", err)
	}
	return NewManager(dir, dir, ManagerOpts{}), dir
}

func TestSession_NewAndLoad(t *testing.T) {
	mgr, dir := newTestManager(t)
	defer os.RemoveAll(dir)

	s, err := mgr.NewSession("gpt-4o", "openrouter")
	if err != nil {
		t.Fatalf("NewSession failed: %v", err)
	}
	if s.ID == "" {
		t.Fatal("Expected non-empty session ID")
	}
	if s.Model != "gpt-4o" {
		t.Errorf("Expected model gpt-4o, got %s", s.Model)
	}
	if s.Provider != "openrouter" {
		t.Errorf("Expected provider openrouter, got %s", s.Provider)
	}
	if s.WorkflowPhase != types.PhaseIdle {
		t.Errorf("Expected PhaseIdle, got %s", s.WorkflowPhase)
	}
	if s.StartedAt.IsZero() {
		t.Error("Expected StartedAt to be set")
	}

	// Load it back
	loaded, err := mgr.LoadSession(s.ID)
	if err != nil {
		t.Fatalf("LoadSession failed: %v", err)
	}
	if loaded.ID != s.ID {
		t.Errorf("Expected ID %s, got %s", s.ID, loaded.ID)
	}
	if loaded.Model != s.Model {
		t.Errorf("Expected Model %s, got %s", s.Model, loaded.Model)
	}
	if loaded.Provider != s.Provider {
		t.Errorf("Expected Provider %s, got %s", s.Provider, loaded.Provider)
	}
}

func TestSession_IDLength(t *testing.T) {
	mgr, dir := newTestManager(t)
	defer os.RemoveAll(dir)

	s, err := mgr.NewSession("gpt-4o", "openrouter")
	if err != nil {
		t.Fatalf("NewSession failed: %v", err)
	}
	if len(s.ID) != 8 {
		t.Errorf("Expected ID length 8, got %d (%q)", len(s.ID), s.ID)
	}

	// Verify hex characters only
	for _, c := range s.ID {
		if !((c >= '0' && c <= '9') || (c >= 'a' && c <= 'f')) {
			t.Errorf("ID contains non-hex character: %c", c)
		}
	}
}

func TestSession_NewSetsTimestamps(t *testing.T) {
	mgr, dir := newTestManager(t)
	defer os.RemoveAll(dir)

	before := time.Now()
	s, err := mgr.NewSession("claude-3", "zen")
	if err != nil {
		t.Fatalf("NewSession failed: %v", err)
	}
	after := time.Now()

	if s.StartedAt.Before(before) || s.StartedAt.After(after) {
		t.Errorf("StartedAt %v should be between %v and %v", s.StartedAt, before, after)
	}

	if !s.StartedAt.IsZero() {
		t.Logf("StartedAt is set correctly: %v", s.StartedAt)
	}
}

func TestSession_LoadMissing(t *testing.T) {
	mgr, dir := newTestManager(t)
	defer os.RemoveAll(dir)

	_, err := mgr.LoadSession("nonexistent")
	if !errors.Is(err, m31errors.ErrSessionNotFound) {
		t.Errorf("Expected ErrSessionNotFound for missing session, got %v", err)
	}
}

func TestSession_LoadCorruptJSON(t *testing.T) {
	t.Skip("removed: project-local sessions")
}
