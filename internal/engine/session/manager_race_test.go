package session

import (
	"os"
	"path/filepath"
	"sync"
	"testing"

	"github.com/eshanized/M31A/internal/core/types"
)

func TestLoadWorkflowState_Concurrent(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	mgr := NewManager(dir, dir, ManagerOpts{})

	// Create a session
	s, err := mgr.NewSession("test-model", "test-provider")
	if err != nil {
		t.Fatalf("NewSession failed: %v", err)
	}

	var wg sync.WaitGroup
	const goroutines = 50

	// Concurrent LoadWorkflowState and UpdateWorkflowState
	for i := 0; i < goroutines; i++ {
		wg.Add(1)
		go func(id int) {
			defer wg.Done()
			if id%2 == 0 {
				// Read
				_, _, _, _ = mgr.LoadWorkflowState(s.ID)
			} else {
				// Write
				_ = mgr.UpdateWorkflowState(s.ID, "test-goal", types.PhaseDiscuss, nil)
			}
		}(i)
	}

	wg.Wait()
}

func TestSaveSessionAtomic_PreservesMessages(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	mgr := NewManager(dir, dir, ManagerOpts{})

	// Create a session
	s, err := mgr.NewSession("test-model", "test-provider")
	if err != nil {
		t.Fatalf("NewSession failed: %v", err)
	}

	// Add messages to the session
	s.Messages = []types.Message{
		{Role: "user", Content: "hello"},
		{Role: "assistant", Content: "hi there"},
	}

	// Save the session
	if err := mgr.saveSessionAtomic(s); err != nil {
		t.Fatalf("saveSessionAtomic failed: %v", err)
	}

	// Read the session.json file
	data, err := os.ReadFile(filepath.Join(dir, ".m31a", "session.json"))
	if err != nil {
		t.Fatalf("ReadFile failed: %v", err)
	}

	// Verify that "messages" field is NOT in the JSON
	// (sessionMetadata excludes Messages)
	jsonStr := string(data)
	if contains(jsonStr, `"messages"`) {
		t.Error("session.json should not contain 'messages' field")
	}

	// Verify that essential metadata is present
	if !contains(jsonStr, `"id"`) {
		t.Error("session.json should contain 'id' field")
	}
	if !contains(jsonStr, `"model"`) {
		t.Error("session.json should contain 'model' field")
	}
}

func TestSession_ConcurrentAccess(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	mgr := NewManager(dir, dir, ManagerOpts{})

	// Create a session
	s, err := mgr.NewSession("test-model", "test-provider")
	if err != nil {
		t.Fatalf("NewSession failed: %v", err)
	}

	var wg sync.WaitGroup
	const goroutines = 20

	// Concurrent load/update/save cycles
	for i := 0; i < goroutines; i++ {
		wg.Add(1)
		go func(id int) {
			defer wg.Done()
			// Load
			_, _, _, _ = mgr.LoadWorkflowState(s.ID)
			// Update
			_ = mgr.UpdateWorkflowState(s.ID, "goal", types.PhasePlan, nil)
			// Save
			_ = mgr.saveSessionAtomic(s)
		}(i)
	}

	wg.Wait()
}

func contains(s, substr string) bool {
	for i := 0; i <= len(s)-len(substr); i++ {
		if s[i:i+len(substr)] == substr {
			return true
		}
	}
	return false
}
