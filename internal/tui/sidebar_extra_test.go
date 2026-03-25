package tui

import (
	"context"
	"testing"

	"github.com/eshanized/M31A/internal/git"
)

func TestSidebarSettersGetters(t *testing.T) {
	s := NewSidebarModel(nil, testTheme())

	s.SetVersion("1.0.0")
	if s.version != "1.0.0" {
		t.Error("version not set")
	}

	s.SetSessionID("sess-42")
	if s.sessionID != "sess-42" {
		t.Error("sessionID not set")
	}

	ctx := context.Background()
	s.SetShutdownContext(ctx)
	if s.shutdownCtx != ctx {
		t.Error("shutdownCtx not set")
	}

	s.SetTheme(testTheme())
}

func TestSidebarToggleExtra(t *testing.T) {
	s := NewSidebarModel(nil, testTheme())
	if !s.visible {
		t.Error("should start visible")
	}
	s.Toggle()
	if s.visible {
		t.Error("should be hidden after toggle")
	}
	s.Toggle()
	if !s.visible {
		t.Error("should be visible after second toggle")
	}
}

func TestSidebarFocus(t *testing.T) {
	s := NewSidebarModel(nil, testTheme())
	if s.focused {
		t.Error("should start unfocused")
	}
	if s.IsFocused() {
		t.Error("IsFocused should be false")
	}
	s.Focus()
	if !s.IsFocused() {
		t.Error("should be focused")
	}
	s.Blur()
	if s.IsFocused() {
		t.Error("should be blurred")
	}
}

func TestSidebarToggleFocusExtra(t *testing.T) {
	s := NewSidebarModel(nil, testTheme())
	s.ToggleFocus()
	if !s.IsFocused() {
		t.Error("should be focused")
	}
	if s.tree.Cursor != 0 {
		t.Error("cursor should be 0 after first focus")
	}
	s.ToggleFocus()
	if s.IsFocused() {
		t.Error("should be blurred")
	}
}

func TestSidebarGetWidth(t *testing.T) {
	s := NewSidebarModel(nil, testTheme())
	s.visible = true
	s.width = 30
	if got := s.GetWidth(); got != 30 {
		t.Errorf("want 30, got %d", got)
	}
	s.visible = false
	if got := s.GetWidth(); got != 0 {
		t.Errorf("want 0 when hidden, got %d", got)
	}
}

func TestSidebarWidthChange(t *testing.T) {
	s := NewSidebarModel(nil, testTheme())
	s.width = sidebarDefaultWidth

	s.IncreaseWidth()
	if s.width != sidebarDefaultWidth+2 {
		t.Errorf("want %d, got %d", sidebarDefaultWidth+2, s.width)
	}

	s.DecreaseWidth()
	if s.width != sidebarDefaultWidth {
		t.Errorf("want %d, got %d", sidebarDefaultWidth, s.width)
	}

	s.width = sidebarMaxWidth
	s.IncreaseWidth()
	if s.width != sidebarMaxWidth {
		t.Error("should not increase past max")
	}

	s.width = sidebarMinWidth
	s.DecreaseWidth()
	if s.width != sidebarMinWidth {
		t.Error("should not decrease past min")
	}
}

func TestSidebarSetHeight(t *testing.T) {
	s := NewSidebarModel(nil, testTheme())
	s.SetHeight(30)
	if s.height != 30 {
		t.Error("height not set")
	}
	if s.tree.Height != 30-sidebarFixedOverhead {
		t.Error("tree height not propagated correctly")
	}

	s.SetHeight(1)
	if s.tree.Height != 3 {
		t.Error("tree height should clamp to 3 minimum")
	}
}

func TestSidebarSelectedFileNilTreeExtra(t *testing.T) {
	s := &SidebarModel{}
	if s.SelectedFile() != nil {
		t.Error("nil tree should return nil")
	}
}

func TestCountFileStatusesExtra(t *testing.T) {
	files := []git.FileStatus{
		{Status: "M", Path: "a.go"},
		{Status: "A", Path: "b.go"},
		{Status: "D", Path: "c.go"},
		{Status: "??", Path: "d.go"},
		{Status: " M", Path: "e.go"},
		{Status: "MM", Path: "f.go"},
	}
	mod, add, del, untracked := countFileStatuses(files)
	if mod != 3 {
		t.Errorf("want 3 mod, got %d", mod)
	}
	if add != 1 {
		t.Errorf("want 1 add, got %d", add)
	}
	if del != 1 {
		t.Errorf("want 1 del, got %d", del)
	}
	if untracked != 1 {
		t.Errorf("want 1 untracked, got %d", untracked)
	}
}

func TestCountFileStatusesEmptyExtra(t *testing.T) {
	mod, add, del, untracked := countFileStatuses(nil)
	if mod != 0 || add != 0 || del != 0 || untracked != 0 {
		t.Error("all should be 0")
	}
}

func TestBuildSidebarTreeExtra(t *testing.T) {
	files := []git.FileStatus{
		{Path: "src/main.go", Status: "M"},
		{Path: "src/utils.go", Status: "A"},
		{Path: "README.md", Status: "??"},
	}
	root := buildSidebarTree(files)
	if root == nil {
		t.Fatal("root should not be nil")
	}
	if root.Name != "." {
		t.Errorf("want root name '.', got %q", root.Name)
	}

	if len(root.Children) != 2 {
		t.Fatalf("want 2 root children, got %d", len(root.Children))
	}

	foundSrc := false
	foundReadme := false
	for _, c := range root.Children {
		if c.Name == "src" {
			foundSrc = true
			if !c.IsDir {
				t.Error("src should be a dir")
			}
			if len(c.Children) != 2 {
				t.Errorf("src should have 2 children, got %d", len(c.Children))
			}
		}
		if c.Name == "README.md" {
			foundReadme = true
			if c.IsDir {
				t.Error("README.md should not be a dir")
			}
			if c.Status != "??" {
				t.Errorf("want status '??', got %q", c.Status)
			}
		}
	}
	if !foundSrc || !foundReadme {
		t.Error("missing expected children")
	}
}

func TestBuildSidebarTreeEmptyExtra(t *testing.T) {
	root := buildSidebarTree(nil)
	if root == nil {
		t.Fatal("root should not be nil")
	}
	if len(root.Children) != 0 {
		t.Error("should have no children")
	}
}
