package session

import (
	"testing"
	"time"
)

func TestSessionInfo_Fields(t *testing.T) {
	t.Parallel()
	si := SessionInfo{
		ID:        "abc123",
		Model:     "claude-3",
		Provider:  "anthropic",
		StartedAt: time.Now(),
	}
	if si.ID != "abc123" {
		t.Errorf("expected 'abc123', got %q", si.ID)
	}
	if si.Model != "claude-3" {
		t.Errorf("expected 'claude-3', got %q", si.Model)
	}
	if si.Provider != "anthropic" {
		t.Errorf("expected 'anthropic', got %q", si.Provider)
	}
}

func TestSessionInfo_MessageCount(t *testing.T) {
	t.Parallel()
	si := SessionInfo{
		ID:           "test",
		MessageCount: 5,
	}
	if si.MessageCount != 5 {
		t.Errorf("expected 5, got %d", si.MessageCount)
	}
}

func TestSessionInfo_Corrupted(t *testing.T) {
	t.Parallel()
	si := SessionInfo{
		ID:          "test",
		Corrupted:   true,
		Label:       "test session",
		ParentID:    "parent1",
		ChildrenIDs: []string{"child1", "child2"},
	}
	if !si.Corrupted {
		t.Error("expected corrupted to be true")
	}
	if si.Label != "test session" {
		t.Errorf("expected 'test session', got %q", si.Label)
	}
	if si.ParentID != "parent1" {
		t.Errorf("expected 'parent1', got %q", si.ParentID)
	}
	if len(si.ChildrenIDs) != 2 {
		t.Errorf("expected 2 children, got %d", len(si.ChildrenIDs))
	}
}
