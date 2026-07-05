package tui

import (
	"context"
	"strings"
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
	// D-32: sidebar starts visible by default on terminals >= 80 columns
	if !s.visible {
		t.Error("should start visible (D-32 default)")
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

// ─── M1 Sidebar Framework Tests ───────────────────────────────────────────────

func TestSidebarM1_DefaultHidden(t *testing.T) {
	s := NewSidebarModel(nil, testTheme())
	// D-32: sidebar starts visible by default on terminals >= 80 columns
	if !s.visible {
		t.Error("sidebar should start visible (D-32 default)")
	}
	if !s.IsVisible() {
		t.Error("IsVisible should return true (D-32 default)")
	}
}

func TestSidebarM1_DefaultMode(t *testing.T) {
	s := NewSidebarModel(nil, testTheme())
	// M1: default mode should be SidebarModeFiles
	if s.GetMode() != SidebarModeFiles {
		t.Errorf("default mode = %v, want SidebarModeFiles", s.GetMode())
	}
}

func TestSidebarM1_CycleMode(t *testing.T) {
	s := NewSidebarModel(nil, testTheme())

	// Start at Files (default) → cycle to Todo
	s.CycleMode()
	if s.GetMode() != SidebarModeTodo {
		t.Errorf("after 1st cycle: mode = %v, want SidebarModeTodo", s.GetMode())
	}

	// Todo → Idle
	s.CycleMode()
	if s.GetMode() != SidebarModeIdle {
		t.Errorf("after 2nd cycle: mode = %v, want SidebarModeIdle", s.GetMode())
	}

	// Idle → Active
	s.CycleMode()
	if s.GetMode() != SidebarModeActive {
		t.Errorf("after 3rd cycle: mode = %v, want SidebarModeActive", s.GetMode())
	}

	// Active → Narrative
	s.CycleMode()
	if s.GetMode() != SidebarModeNarrative {
		t.Errorf("after 4th cycle: mode = %v, want SidebarModeNarrative", s.GetMode())
	}

	// Narrative → Files (back to start)
	s.CycleMode()
	if s.GetMode() != SidebarModeFiles {
		t.Errorf("after 5th cycle: mode = %v, want SidebarModeFiles", s.GetMode())
	}
}

func TestSidebarM1_IdleModeRender(t *testing.T) {
	s := NewSidebarModel(nil, testTheme())
	s.visible = true
	s.SetMode(SidebarModeIdle)
	s.branch = "main"
	s.cost = 0.05
	s.showCost = true
	s.currentPhase = "execute"

	view := s.View()
	if view == "" {
		t.Error("idle mode should produce non-empty output")
	}
	// Should contain branch
	if !strings.Contains(view, "main") {
		t.Error("idle mode should show branch")
	}
	// Should contain cost
	if !strings.Contains(view, "$0.05") {
		t.Error("idle mode should show cost")
	}
	// Should contain phase
	if !strings.Contains(view, "execute") {
		t.Error("idle mode should show phase")
	}
}

func TestSidebarM1_ActiveModeRender(t *testing.T) {
	s := NewSidebarModel(nil, testTheme())
	s.visible = true
	s.SetMode(SidebarModeActive)
	s.branch = "feature"
	s.currentPhase = "plan"
	s.todoItems = []SidebarTodoItem{
		{Content: "Task 1", Status: "in_progress"},
		{Content: "Task 2", Status: "completed"},
	}

	view := s.View()
	if view == "" {
		t.Error("active mode should produce non-empty output")
	}
	// Should contain branch
	if !strings.Contains(view, "feature") {
		t.Error("active mode should show branch")
	}
	// Should contain phase
	if !strings.Contains(view, "plan") {
		t.Error("active mode should show phase")
	}
}

func TestSidebarM1_IdleModeMinimalLines(t *testing.T) {
	s := NewSidebarModel(nil, testTheme())
	s.visible = true
	s.SetMode(SidebarModeIdle)

	// With minimal data, idle mode should produce 0-3 lines
	s.branch = "main"
	lines := s.renderIdle(28)
	if len(lines) > 3 {
		t.Errorf("idle mode produced %d lines, want <= 3", len(lines))
	}
}

func TestSidebarM1_ActiveModeModerateLines(t *testing.T) {
	s := NewSidebarModel(nil, testTheme())
	s.visible = true
	s.SetMode(SidebarModeActive)
	s.branch = "main"
	s.currentPhase = "execute"
	s.todoItems = []SidebarTodoItem{
		{Content: "Task 1", Status: "in_progress"},
		{Content: "Task 2", Status: "in_progress"},
		{Content: "Task 3", Status: "completed"},
	}

	lines := s.renderActive(28)
	// Active mode: branch + phase + 2 in-progress = 4 lines min
	// With more data: up to 7 lines
	if len(lines) > 7 {
		t.Errorf("active mode produced %d lines, want <= 7", len(lines))
	}
	if len(lines) < 2 {
		t.Errorf("active mode produced %d lines, want >= 2", len(lines))
	}
}

func TestSidebarM1_ToggleVisibility(t *testing.T) {
	s := NewSidebarModel(nil, testTheme())

	// D-32: Start visible by default
	if !s.IsVisible() {
		t.Error("should start visible (D-32 default)")
	}

	// Toggle to hidden
	s.Toggle()
	if s.IsVisible() {
		t.Error("should be hidden after toggle")
	}

	// Toggle back to visible
	s.Toggle()
	if !s.IsVisible() {
		t.Error("should be visible after second toggle")
	}
}

func TestSidebarM1_WidthDefault(t *testing.T) {
	s := NewSidebarModel(nil, testTheme())
	// D-32: Default width should be 30 (sidebarDefaultWidth), visible by default
	if s.GetWidth() != sidebarDefaultWidth {
		t.Errorf("visible sidebar width = %d, want %d", s.GetWidth(), sidebarDefaultWidth)
	}
	s.visible = false
	if s.GetWidth() != 0 {
		t.Errorf("hidden sidebar width = %d, want 0", s.GetWidth())
	}
}

func TestSidebarM1_ModeConstants(t *testing.T) {
	// Verify mode constants exist and have expected values
	if SidebarModeFiles != 0 {
		t.Errorf("SidebarModeFiles = %d, want 0", SidebarModeFiles)
	}
	if SidebarModeTodo != 1 {
		t.Errorf("SidebarModeTodo = %d, want 1", SidebarModeTodo)
	}
	if SidebarModeIdle != 2 {
		t.Errorf("SidebarModeIdle = %d, want 2", SidebarModeIdle)
	}
	if SidebarModeActive != 3 {
		t.Errorf("SidebarModeActive = %d, want 3", SidebarModeActive)
	}
}
