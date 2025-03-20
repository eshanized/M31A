package tui

import (
	"os"
	"strings"
	"testing"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/eshanized/M31A/internal/tui/theme"
	"github.com/eshanized/M31A/pkg/session"
)

func newTestResumeModel(t *testing.T) (*ResumeModel, *session.Manager, string) {
	t.Helper()
	tmpDir, err := os.MkdirTemp("", "m31a-test-*")
	if err != nil {
		t.Fatalf("Failed to create temp dir: %v", err)
	}
	mgr := session.NewManager(tmpDir)
	m := NewResumeModel(theme.Dark(), mgr)
	m.width = 80
	m.height = 24
	return m, mgr, tmpDir
}

func TestResume_InitialRender(t *testing.T) {
	m, _, tmpDir := newTestResumeModel(t)
	defer os.RemoveAll(tmpDir)

	v := m.View()
	if v == "" {
		t.Error("View() should not be empty")
	}
	if !strings.Contains(v, "Enter: resume") {
		t.Errorf("View() should contain footer hint, got: %s", v)
	}
}

func TestResume_ListSessions(t *testing.T) {
	m, mgr, tmpDir := newTestResumeModel(t)
	defer os.RemoveAll(tmpDir)

	// Create 2 sessions
	s1, err := mgr.NewSession("gpt-4o", "openrouter")
	if err != nil {
		t.Fatalf("Failed to create session: %v", err)
	}
	mgr.SaveSession(s1)

	s2, err := mgr.NewSession("claude-3", "zen")
	if err != nil {
		t.Fatalf("Failed to create session: %v", err)
	}
	mgr.SaveSession(s2)

	// Refresh the model
	m.Refresh()

	if len(m.list.Items()) != 2 {
		t.Errorf("Expected 2 items in list, got %d", len(m.list.Items()))
	}
}

func TestResume_CorruptedBadge(t *testing.T) {
	m, mgr, tmpDir := newTestResumeModel(t)
	defer os.RemoveAll(tmpDir)

	// Create a valid session first
	s, err := mgr.NewSession("gpt-4o", "openrouter")
	if err != nil {
		t.Fatalf("Failed to create session: %v", err)
	}
	mgr.SaveSession(s)

	// Create a corrupt session directory (no session.json)
	corruptDir := tmpDir + "/corrupted1234"
	os.MkdirAll(corruptDir, 0755)

	// Refresh and check
	m.Refresh()

	found := false
	for _, item := range m.list.Items() {
		si := item.(sessionItem)
		if si.corrupted {
			found = true
			if !strings.Contains(si.title, "[!]") {
				t.Errorf("Corrupted session should have '[!]' in title, got: %s", si.title)
			}
		}
	}
	if !found {
		t.Error("Expected at least one corrupted session item")
	}

	os.RemoveAll(corruptDir)
}

func TestResume_EnterSelectsSession(t *testing.T) {
	m, mgr, tmpDir := newTestResumeModel(t)
	defer os.RemoveAll(tmpDir)

	// Create a session
	s, err := mgr.NewSession("gpt-4o", "openrouter")
	if err != nil {
		t.Fatalf("Failed to create session: %v", err)
	}
	mgr.SaveSession(s)
	m.Refresh()

	// Send Enter to select the session
	_, appMsg := m.Update(tea.KeyMsg{Type: tea.KeyEnter})
	if appMsg == nil {
		t.Fatal("Enter should return an AppMsg")
	}
	if appMsg.Screen != ScreenREPL {
		t.Errorf("Expected ScreenREPL after selection, got %d", appMsg.Screen)
	}
	if appMsg.SessionID == "" {
		t.Error("SessionID should not be empty after selection")
	}
}

func TestResume_NewSession(t *testing.T) {
	m, _, tmpDir := newTestResumeModel(t)
	defer os.RemoveAll(tmpDir)

	// Send N key
	_, appMsg := m.Update(tea.KeyMsg{Type: tea.KeyRunes, Runes: []rune{'N'}})
	if appMsg == nil {
		t.Fatal("N key should return an AppMsg")
	}
	if appMsg.Screen != ScreenFirstRun {
		t.Errorf("Expected ScreenFirstRun for new session, got %d", appMsg.Screen)
	}

	// Also test 'n' lowercase
	_, appMsg = m.Update(tea.KeyMsg{Type: tea.KeyRunes, Runes: []rune{'n'}})
	if appMsg == nil {
		t.Fatal("n key should return an AppMsg")
	}
	if appMsg.Screen != ScreenFirstRun {
		t.Errorf("Expected ScreenFirstRun for new session (lowercase), got %d", appMsg.Screen)
	}
}

func TestResume_DeleteConfirmation(t *testing.T) {
	m, mgr, tmpDir := newTestResumeModel(t)
	defer os.RemoveAll(tmpDir)

	// Create a session
	s, err := mgr.NewSession("gpt-4o", "openrouter")
	if err != nil {
		t.Fatalf("Failed to create session: %v", err)
	}
	mgr.SaveSession(s)
	m.Refresh()

	// Send D key to enter delete mode
	if m.confirmDel {
		t.Error("Should not be in confirm mode initially")
	}

	_, appMsg := m.Update(tea.KeyMsg{Type: tea.KeyRunes, Runes: []rune{'D'}})
	if !m.confirmDel {
		t.Error("D key should set confirmDel=true")
	}
	if appMsg != nil {
		t.Error("D key should not return an AppMsg")
	}

	// Verify the delete confirmation view
	v := m.View()
	if !strings.Contains(v, "Delete session") {
		t.Errorf("Delete confirmation view should contain 'Delete session', got: %s", v)
	}
}

func TestResume_DeleteConfirmYes(t *testing.T) {
	m, mgr, tmpDir := newTestResumeModel(t)
	defer os.RemoveAll(tmpDir)

	// Create a session
	s, err := mgr.NewSession("gpt-4o", "openrouter")
	if err != nil {
		t.Fatalf("Failed to create session: %v", err)
	}
	mgr.SaveSession(s)

	m.Refresh()

	// Enter delete mode
	m.confirmDel = true
	m.delTarget = s.ID

	// Confirm with Y
	_, appMsg := m.Update(tea.KeyMsg{Type: tea.KeyRunes, Runes: []rune{'Y'}})
	if m.confirmDel {
		t.Error("After Y, confirmDel should be false")
	}
	if appMsg != nil {
		t.Error("Y should not return an AppMsg")
	}

	// Verify session is deleted
	_, err = mgr.LoadSession(s.ID)
	if err == nil {
		t.Error("Session should be deleted")
	}
}

func TestResume_DeleteConfirmNo(t *testing.T) {
	m, mgr, tmpDir := newTestResumeModel(t)
	defer os.RemoveAll(tmpDir)

	// Create a session
	s, err := mgr.NewSession("gpt-4o", "openrouter")
	if err != nil {
		t.Fatalf("Failed to create session: %v", err)
	}
	mgr.SaveSession(s)

	m.Refresh()

	// Enter delete mode
	m.confirmDel = true
	m.delTarget = s.ID

	// Cancel with N
	_, appMsg := m.Update(tea.KeyMsg{Type: tea.KeyRunes, Runes: []rune{'N'}})
	if m.confirmDel {
		t.Error("After N, confirmDel should be false")
	}
	if appMsg != nil {
		t.Error("N should not return an AppMsg")
	}

	// Verify session still exists
	loaded, err := mgr.LoadSession(s.ID)
	if err != nil {
		t.Errorf("Session should still exist after cancel: %v", err)
	}
	if loaded.ID != s.ID {
		t.Error("Loaded session ID should match")
	}
}

func TestResume_EscReturnsToREPL(t *testing.T) {
	m, _, tmpDir := newTestResumeModel(t)
	defer os.RemoveAll(tmpDir)

	// Send Esc key
	_, appMsg := m.Update(tea.KeyMsg{Type: tea.KeyEsc})
	if appMsg == nil {
		t.Fatal("Esc should return an AppMsg")
	}
	if appMsg.Screen != ScreenREPL {
		t.Errorf("Expected ScreenREPL after Esc, got %d", appMsg.Screen)
	}
}

func TestResume_WindowSize(t *testing.T) {
	// Create model with zero initial size by not using the helper
	tmpDir, err := os.MkdirTemp("", "m31a-test-*")
	if err != nil {
		t.Fatalf("Failed to create temp dir: %v", err)
	}
	defer os.RemoveAll(tmpDir)

	mgr := session.NewManager(tmpDir)
	m := NewResumeModel(theme.Dark(), mgr)

	// Initial size should be 0
	if m.width != 0 || m.height != 0 {
		t.Error("Initial width/height should be 0")
	}

	m.Update(tea.WindowSizeMsg{Width: 120, Height: 40})
	if m.width != 120 {
		t.Errorf("Expected width=120, got %d", m.width)
	}
	if m.height != 40 {
		t.Errorf("Expected height=40, got %d", m.height)
	}
}
