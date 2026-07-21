package tui

import (
	"testing"
	"time"

	"github.com/eshanized/M31A/internal/integrations/git"
	"github.com/eshanized/M31A/internal/ui/tui/theme"
)

func TestSidebarModel_MarkFileChanged_TTLEviction(t *testing.T) {
	sm := NewSidebarModel(nil, theme.NewManager(theme.ModeDark).Current())

	sm.MarkFileChanged("file1.go")
	sm.MarkFileChanged("file2.go")

	if !sm.IsFileRecentlyChanged("file1.go") {
		t.Error("expected file1.go to be recently changed")
	}
	if !sm.IsFileRecentlyChanged("file2.go") {
		t.Error("expected file2.go to be recently changed")
	}

	// Manually age an entry to simulate TTL expiry
	sm.recentlyChanged["file1.go"] = time.Now().Add(-recentlyChangedTTL - time.Second)

	// IsFileRecentlyChanged should return false for the aged entry
	if sm.IsFileRecentlyChanged("file1.go") {
		t.Error("expected file1.go to be evicted by TTL")
	}

	// file2.go should still be recent
	if !sm.IsFileRecentlyChanged("file2.go") {
		t.Error("expected file2.go to still be recent")
	}
}

func TestSidebarModel_MarkFileChanged_EvictsStaleOnInsert(t *testing.T) {
	sm := NewSidebarModel(nil, theme.NewManager(theme.ModeDark).Current())

	// Add a stale entry directly
	sm.recentlyChanged = map[string]time.Time{
		"stale.go": time.Now().Add(-recentlyChangedTTL - time.Minute),
	}

	// MarkFileChanged should evict stale entries
	sm.MarkFileChanged("new.go")

	if _, exists := sm.recentlyChanged["stale.go"]; exists {
		t.Error("expected stale.go to be evicted after MarkFileChanged")
	}
	if !sm.IsFileRecentlyChanged("new.go") {
		t.Error("expected new.go to be recently changed")
	}
}

func TestSidebarModel_ClearRecentlyChanged(t *testing.T) {
	sm := NewSidebarModel(nil, theme.NewManager(theme.ModeDark).Current())

	sm.MarkFileChanged("file1.go")
	sm.MarkFileChanged("file2.go")
	sm.ClearRecentlyChanged()

	if sm.IsFileRecentlyChanged("file1.go") {
		t.Error("expected file1.go to be cleared")
	}
	if sm.IsFileRecentlyChanged("file2.go") {
		t.Error("expected file2.go to be cleared")
	}
}

func TestSidebarModel_IsFileRecentlyChanged_NeverAdded(t *testing.T) {
	sm := NewSidebarModel(git.New(t.TempDir()), theme.NewManager(theme.ModeDark).Current())

	if sm.IsFileRecentlyChanged("nonexistent.go") {
		t.Error("expected false for file that was never marked")
	}
}
