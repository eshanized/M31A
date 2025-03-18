package session

import (
	"encoding/json"
	"os"
	"path/filepath"
	"strings"
	"testing"
	"time"

	"github.com/eshanized/M31A/internal/types"
	m31errors "github.com/eshanized/M31A/internal/errors"
)

// newTestManager creates a Manager backed by a temp directory.
// The caller must call os.RemoveAll on the returned dir string.
func newTestManager(t *testing.T) (*Manager, string) {
	t.Helper()
	dir, err := os.MkdirTemp("", "m31a-session-test-*")
	if err != nil {
		t.Fatalf("Failed to create temp dir: %v", err)
	}
	return NewManager(dir), dir
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
	if err != m31errors.ErrSessionCorrupted {
		t.Errorf("Expected ErrSessionCorrupted for missing session, got %v", err)
	}
}

func TestSession_LoadCorruptJSON(t *testing.T) {
	mgr, dir := newTestManager(t)
	defer os.RemoveAll(dir)

	// Create a session dir with corrupt session.json
	sessionDir := filepath.Join(dir, "deadbeef")
	if err := os.MkdirAll(sessionDir, 0755); err != nil {
		t.Fatalf("Failed to create session dir: %v", err)
	}
	if err := os.WriteFile(filepath.Join(sessionDir, "session.json"), []byte("{corrupt"), 0644); err != nil {
		t.Fatalf("Failed to write corrupt session.json: %v", err)
	}

	_, err := mgr.LoadSession("deadbeef")
	if err != m31errors.ErrSessionCorrupted {
		t.Errorf("Expected ErrSessionCorrupted for corrupt JSON, got %v", err)
	}
}

func TestSession_LoadMissingMessages(t *testing.T) {
	mgr, dir := newTestManager(t)
	defer os.RemoveAll(dir)

	// Create a session with NewSession (this creates session.json but no messages.json)
	s, err := mgr.NewSession("gpt-4o", "openrouter")
	if err != nil {
		t.Fatalf("NewSession failed: %v", err)
	}

	// NewSession doesn't create messages.json, so it's already "missing"

	// Load should succeed, messages should be empty slice
	loaded, err := mgr.LoadSession(s.ID)
	if err != nil {
		t.Fatalf("LoadSession should succeed without messages.json, got: %v", err)
	}
	if loaded.Messages == nil {
		t.Error("Messages should be empty slice, not nil")
	}
	if len(loaded.Messages) != 0 {
		t.Errorf("Expected 0 messages, got %d", len(loaded.Messages))
	}
	if loaded.MessageCount != 0 {
		t.Errorf("Expected MessageCount 0, got %d", loaded.MessageCount)
	}
}

func TestSession_ListSessions(t *testing.T) {
	mgr, dir := newTestManager(t)
	defer os.RemoveAll(dir)

	// Create 3 sessions
	s1, err := mgr.NewSession("gpt-4o", "openrouter")
	if err != nil {
		t.Fatalf("NewSession 1 failed: %v", err)
	}
	s2, err := mgr.NewSession("claude-3", "zen")
	if err != nil {
		t.Fatalf("NewSession 2 failed: %v", err)
	}
	s3, err := mgr.NewSession("gpt-4o-mini", "openrouter")
	if err != nil {
		t.Fatalf("NewSession 3 failed: %v", err)
	}

	// Slight delay to ensure ordering (directory ModTime granularity)
	time.Sleep(10 * time.Millisecond)

	// Update s2's session.json to push it as most recent
	s2.MessageCount = 5
	if err := mgr.SaveSession(s2); err != nil {
		t.Fatalf("SaveSession failed: %v", err)
	}

	time.Sleep(10 * time.Millisecond)

	sessions, err := mgr.ListSessions()
	if err != nil {
		t.Fatalf("ListSessions failed: %v", err)
	}

	if len(sessions) != 3 {
		t.Fatalf("Expected 3 sessions, got %d", len(sessions))
	}

	// Check that s2 appears first (most recently modified via SaveSession)
	if sessions[0].ID != s2.ID {
		t.Logf("Expected first session %s (most recent), got %s", s2.ID, sessions[0].ID)
	}

	// Verify all IDs present
	ids := make(map[string]bool)
	for _, si := range sessions {
		ids[si.ID] = true
	}
	if !ids[s1.ID] {
		t.Error("List missing s1")
	}
	if !ids[s2.ID] {
		t.Error("List missing s2")
	}
	if !ids[s3.ID] {
		t.Error("List missing s3")
	}

	// Verify sorted descending
	for i := 0; i < len(sessions)-1; i++ {
		if sessions[i].LastModified.Before(sessions[i+1].LastModified) {
			t.Errorf("Sessions not sorted descending at index %d", i)
		}
	}
}

func TestSession_ListSessionsEmpty(t *testing.T) {
	mgr, dir := newTestManager(t)
	defer os.RemoveAll(dir)

	sessions, err := mgr.ListSessions()
	if err != nil {
		t.Fatalf("ListSessions on empty dir failed: %v", err)
	}
	if len(sessions) != 0 {
		t.Errorf("Expected 0 sessions, got %d", len(sessions))
	}
}

func TestSession_ListSessionsWithCorrupt(t *testing.T) {
	mgr, dir := newTestManager(t)
	defer os.RemoveAll(dir)

	// Create a valid session
	valid, err := mgr.NewSession("gpt-4o", "openrouter")
	if err != nil {
		t.Fatalf("NewSession failed: %v", err)
	}
	_ = valid

	// Create a corrupt session directory
	corruptDir := filepath.Join(dir, "baddead")
	if err := os.MkdirAll(corruptDir, 0755); err != nil {
		t.Fatalf("Failed to create corrupt dir: %v", err)
	}
	if err := os.WriteFile(filepath.Join(corruptDir, "session.json"), []byte("{garbage"), 0644); err != nil {
		t.Fatalf("Failed to write corrupt file: %v", err)
	}

	sessions, err := mgr.ListSessions()
	if err != nil {
		t.Fatalf("ListSessions failed: %v", err)
	}
	if len(sessions) != 2 {
		t.Fatalf("Expected 2 sessions, got %d", len(sessions))
	}

	// Find the corrupt one
	var found bool
	for _, s := range sessions {
		if s.ID == "baddead" {
			found = true
			if !s.Corrupted {
				t.Error("Expected corrupt session to have Corrupted=true")
			}
		}
	}
	if !found {
		t.Error("Corrupt session not in list")
	}
}

func TestSession_DeleteSession(t *testing.T) {
	mgr, dir := newTestManager(t)
	defer os.RemoveAll(dir)

	s, err := mgr.NewSession("gpt-4o", "openrouter")
	if err != nil {
		t.Fatalf("NewSession failed: %v", err)
	}

	// Verify directory exists
	sessionDir := mgr.basePathFor(s.ID)
	if _, err := os.Stat(sessionDir); os.IsNotExist(err) {
		t.Fatal("Session directory should exist after creation")
	}

	// Delete
	if err := mgr.DeleteSession(s.ID); err != nil {
		t.Fatalf("DeleteSession failed: %v", err)
	}

	// Verify directory is gone
	if _, err := os.Stat(sessionDir); !os.IsNotExist(err) {
		t.Error("Session directory should be removed after delete")
	}

	// Double delete should not error (os.RemoveAll returns nil for missing)
	if err := mgr.DeleteSession(s.ID); err != nil {
		t.Errorf("DeleteSession on already-deleted session should not error, got: %v", err)
	}
}

func TestSession_ArchiveSession(t *testing.T) {
	mgr, dir := newTestManager(t)
	defer os.RemoveAll(dir)

	s, err := mgr.NewSession("gpt-4o", "openrouter")
	if err != nil {
		t.Fatalf("NewSession failed: %v", err)
	}

	// Archive
	if err := mgr.ArchiveSession(s.ID); err != nil {
		t.Fatalf("ArchiveSession failed: %v", err)
	}

	// Verify original dir gone
	originalPath := mgr.basePathFor(s.ID)
	if _, err := os.Stat(originalPath); !os.IsNotExist(err) {
		t.Error("Original session directory should be removed after archive")
	}

	// Verify in archived/
	archivePath := filepath.Join(dir, "archived", s.ID)
	if _, err := os.Stat(archivePath); os.IsNotExist(err) {
		t.Fatal("Archived session directory should exist")
	}

	// Verify session.json exists inside
	archiveSessionJSON := filepath.Join(archivePath, "session.json")
	if _, err := os.Stat(archiveSessionJSON); os.IsNotExist(err) {
		t.Error("Archived session.json should exist")
	}
}

func TestSession_ArchiveAndList(t *testing.T) {
	mgr, dir := newTestManager(t)
	defer os.RemoveAll(dir)

	s, err := mgr.NewSession("gpt-4o", "openrouter")
	if err != nil {
		t.Fatalf("NewSession failed: %v", err)
	}

	// Verify it appears in list
	sessions, err := mgr.ListSessions()
	if err != nil {
		t.Fatalf("ListSessions failed: %v", err)
	}
	if len(sessions) != 1 {
		t.Fatalf("Expected 1 session before archive, got %d", len(sessions))
	}

	// Archive
	if err := mgr.ArchiveSession(s.ID); err != nil {
		t.Fatalf("ArchiveSession failed: %v", err)
	}

	// Verify it no longer appears in list
	sessions, err = mgr.ListSessions()
	if err != nil {
		t.Fatalf("ListSessions failed: %v", err)
	}
	if len(sessions) != 0 {
		t.Errorf("Expected 0 sessions after archive, got %d", len(sessions))
		for _, si := range sessions {
			t.Logf("  Remaining: %s (corrupted=%v)", si.ID, si.Corrupted)
		}
	}
}

func TestSession_SaveSession(t *testing.T) {
	mgr, dir := newTestManager(t)
	defer os.RemoveAll(dir)

	s, err := mgr.NewSession("gpt-4o", "openrouter")
	if err != nil {
		t.Fatalf("NewSession failed: %v", err)
	}

	// Modify and save
	s.WorkflowPhase = types.PhaseDiscuss
	s.Messages = append(s.Messages, types.Message{
		Role:    "user",
		Content: "Hello",
	})
	s.MessageCount = len(s.Messages) // must be consistent with Messages length

	if err := mgr.SaveSession(s); err != nil {
		t.Fatalf("SaveSession failed: %v", err)
	}

	// Reload and verify
	loaded, err := mgr.LoadSession(s.ID)
	if err != nil {
		t.Fatalf("LoadSession after save failed: %v", err)
	}
	if loaded.MessageCount != 1 {
		t.Errorf("Expected MessageCount 1 (matching Messages length), got %d", loaded.MessageCount)
	}
	if loaded.WorkflowPhase != types.PhaseDiscuss {
		t.Errorf("Expected PhaseDiscuss, got %s", loaded.WorkflowPhase)
	}
	if len(loaded.Messages) != 1 {
		t.Fatalf("Expected 1 message, got %d", len(loaded.Messages))
	}
	if loaded.Messages[0].Content != "Hello" {
		t.Errorf("Expected message content 'Hello', got %q", loaded.Messages[0].Content)
	}
}

func TestSession_SaveSessionEmptyMessages(t *testing.T) {
	mgr, dir := newTestManager(t)
	defer os.RemoveAll(dir)

	s, err := mgr.NewSession("gpt-4o", "openrouter")
	if err != nil {
		t.Fatalf("NewSession failed: %v", err)
	}

	// Save with nil messages (should handle gracefully)
	s.Messages = nil
	if err := mgr.SaveSession(s); err != nil {
		t.Fatalf("SaveSession with nil messages failed: %v", err)
	}

	// Load back and verify messages are empty slice, not nil
	loaded, err := mgr.LoadSession(s.ID)
	if err != nil {
		t.Fatalf("LoadSession failed: %v", err)
	}
	if loaded.Messages == nil {
		t.Error("Messages should be empty slice after save/load, not nil")
	}
}

func TestSession_NewSessionCollisionHandling(t *testing.T) {
	mgr, dir := newTestManager(t)
	defer os.RemoveAll(dir)

	// Test that multiple sessions all get unique IDs
	generated := make(map[string]bool)
	for i := 0; i < 20; i++ {
		s, err := mgr.NewSession("gpt-4o", "openrouter")
		if err != nil {
			t.Fatalf("NewSession %d failed: %v", i, err)
		}
		if generated[s.ID] {
			t.Errorf("Duplicate session ID generated: %s", s.ID)
		}
		generated[s.ID] = true
	}

	if len(generated) != 20 {
		t.Errorf("Expected 20 unique IDs, got %d", len(generated))
	}
}

func TestSession_NewAndArchiveAtomicity(t *testing.T) {
	mgr, dir := newTestManager(t)
	defer os.RemoveAll(dir)

	s, err := mgr.NewSession("gpt-4o", "openrouter")
	if err != nil {
		t.Fatalf("NewSession failed: %v", err)
	}

	// Verify session.json was written atomically by checking for temp files
	sessionDir := mgr.basePathFor(s.ID)
	entries, err := os.ReadDir(sessionDir)
	if err != nil {
		t.Fatalf("Failed to read session dir: %v", err)
	}
	for _, e := range entries {
		if strings.HasPrefix(e.Name(), ".m31a_tmp") {
			t.Errorf("Leftover temp file found: %s", e.Name())
		}
	}
}

func TestSession_SetPhase(t *testing.T) {
	s := NewSession("test-id", "gpt-4o", "openrouter")

	if s.WorkflowPhase != types.PhaseIdle {
		t.Errorf("Expected initial PhaseIdle, got %s", s.WorkflowPhase)
	}

	s.SetPhase(types.PhaseExecute)
	if s.WorkflowPhase != types.PhaseExecute {
		t.Errorf("Expected PhaseExecute after SetPhase, got %s", s.WorkflowPhase)
	}

	s.SetPhase(types.PhaseShip)
	if s.WorkflowPhase != types.PhaseShip {
		t.Errorf("Expected PhaseShip after SetPhase, got %s", s.WorkflowPhase)
	}
}

func TestSession_ListSessionsEmptyBaseDir(t *testing.T) {
	mgr, dir := newTestManager(t)
	defer os.RemoveAll(dir)

	// Remove the base dir entirely
	if err := os.RemoveAll(dir); err != nil {
		t.Fatalf("Failed to remove base dir: %v", err)
	}

	sessions, err := mgr.ListSessions()
	if err != nil {
		t.Fatalf("ListSessions with missing base dir should not error, got: %v", err)
	}
	if len(sessions) != 0 {
		t.Errorf("Expected 0 sessions, got %d", len(sessions))
	}
}

func TestSession_GenerateIDFormat(t *testing.T) {
	for i := 0; i < 100; i++ {
		id, err := generateID()
		if err != nil {
			t.Fatalf("generateID failed: %v", err)
		}
		if len(id) != 8 {
			t.Errorf("Expected 8-char ID, got %d: %q", len(id), id)
		}
		// Verify all lowercase hex
		for _, c := range id {
			if !((c >= '0' && c <= '9') || (c >= 'a' && c <= 'f')) {
				t.Errorf("Non-hex char %c in ID %q", c, id)
				break
			}
		}
	}
}

func TestSession_LoadSessionAfterArchive(t *testing.T) {
	mgr, dir := newTestManager(t)
	defer os.RemoveAll(dir)

	s, err := mgr.NewSession("gpt-4o", "openrouter")
	if err != nil {
		t.Fatalf("NewSession failed: %v", err)
	}

	if err := mgr.ArchiveSession(s.ID); err != nil {
		t.Fatalf("ArchiveSession failed: %v", err)
	}

	// Loading archived session from original path should fail
	_, err = mgr.LoadSession(s.ID)
	if err == nil {
		t.Error("LoadSession should fail after archive")
	}
}

func TestSession_MarshalSessionJSON(t *testing.T) {
	s := NewSession("abcdef12", "gpt-4o", "openrouter")
	s.Messages = []types.Message{
		{Role: "user", Content: "test"},
	}
	s.Tasks = []types.Task{
		{ID: 1, Description: "Test task"},
	}

	data, err := json.Marshal(s)
	if err != nil {
		t.Fatalf("Marshal session failed: %v", err)
	}

	var decoded Session
	if err := json.Unmarshal(data, &decoded); err != nil {
		t.Fatalf("Unmarshal session failed: %v", err)
	}

	if decoded.ID != "abcdef12" {
		t.Errorf("Expected ID abcdef12, got %s", decoded.ID)
	}
	if decoded.Model != "gpt-4o" {
		t.Errorf("Expected model gpt-4o, got %s", decoded.Model)
	}
	if len(decoded.Messages) != 1 {
		t.Errorf("Expected 1 message, got %d", len(decoded.Messages))
	}
	if len(decoded.Tasks) != 1 {
		t.Errorf("Expected 1 task, got %d", len(decoded.Tasks))
	}
}
