package tui

import (
	"strings"
	"testing"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/eshanized/M31A/internal/git"
	"github.com/eshanized/M31A/internal/tui/theme"
)

func TestShipModel_New(t *testing.T) {
	summary := ShipSummary{
		TaskDone:    5,
		TaskTotal:   6,
		TaskFailed:  1,
		TaskSkipped: 0,
		Commits: []git.CommitInfo{
			{ShortHash: "abc1234", Message: "Implement feature A"},
			{ShortHash: "def5678", Message: "Fix bug B"},
		},
		Duration:  "15m30s",
		SessionID: "sess-001",
	}
	th := theme.Dark()

	m := NewShipModel(summary, th, 0, 0)

	if m == nil {
		t.Fatal("expected non-nil ShipModel")
	}
	if m.summary.TaskDone != 5 {
		t.Errorf("expected TaskDone 5, got %d", m.summary.TaskDone)
	}
	if m.summary.SessionID != "sess-001" {
		t.Errorf("expected SessionID sess-001, got %s", m.summary.SessionID)
	}
	if len(m.summary.Commits) != 2 {
		t.Errorf("expected 2 commits, got %d", len(m.summary.Commits))
	}
}

func TestShipModel_UpdateWindowSize(t *testing.T) {
	summary := ShipSummary{
		TaskDone:  3,
		TaskTotal: 3,
		SessionID: "sess-001",
		Duration:  "5m",
	}
	m := NewShipModel(summary, theme.Dark(), 0, 0)

	_, _ = m.Update(tea.WindowSizeMsg{Width: 100, Height: 40})

	if m.width != 100 {
		t.Errorf("expected width 100, got %d", m.width)
	}
	if m.height != 40 {
		t.Errorf("expected height 40, got %d", m.height)
	}
}

func TestShipModel_UpdateNewSession(t *testing.T) {
	summary := ShipSummary{
		TaskDone:  1,
		TaskTotal: 1,
		SessionID: "sess-001",
		Duration:  "1m",
	}
	m := NewShipModel(summary, theme.Dark(), 0, 0)
	m.width = 80
	m.height = 24

	// First 'n' sets confirmation gate
	_, appMsg := m.Update(tea.KeyMsg{Type: tea.KeyRunes, Runes: []rune{'n'}})
	if appMsg != nil {
		t.Error("first 'n' should set confirmation gate, not trigger action")
	}
	// Second 'n' confirms and triggers new session
	_, appMsg = m.Update(tea.KeyMsg{Type: tea.KeyRunes, Runes: []rune{'n'}})
	if appMsg == nil || appMsg.Screen != ScreenREPL || appMsg.Action != "new_session" {
		t.Error("expected ScreenREPL with new_session action on 'n' key")
	}

	// Test uppercase N
	m2 := NewShipModel(summary, theme.Dark(), 0, 0)
	m2.width = 80
	m2.height = 24
	// First 'N' sets confirmation gate
	_, appMsg = m2.Update(tea.KeyMsg{Type: tea.KeyRunes, Runes: []rune{'N'}})
	if appMsg != nil {
		t.Error("first 'N' should set confirmation gate, not trigger action")
	}
	// Second 'N' confirms and triggers new session
	_, appMsg = m2.Update(tea.KeyMsg{Type: tea.KeyRunes, Runes: []rune{'N'}})
	if appMsg == nil || appMsg.Screen != ScreenREPL || appMsg.Action != "new_session" {
		t.Error("expected ScreenREPL with new_session action on 'N' key")
	}
}

func TestShipModel_UpdateREPL(t *testing.T) {
	summary := ShipSummary{
		TaskDone:  1,
		TaskTotal: 1,
		SessionID: "sess-001",
		Duration:  "1m",
	}
	m := NewShipModel(summary, theme.Dark(), 0, 0)
	m.width = 80
	m.height = 24

	_, appMsg := m.Update(tea.KeyMsg{Type: tea.KeyRunes, Runes: []rune{'r'}})
	if appMsg == nil || appMsg.Screen != ScreenREPL {
		t.Error("expected ScreenREPL on 'r' key")
	}

	// Test uppercase R
	m2 := NewShipModel(summary, theme.Dark(), 0, 0)
	m2.width = 80
	m2.height = 24
	_, appMsg = m2.Update(tea.KeyMsg{Type: tea.KeyRunes, Runes: []rune{'R'}})
	if appMsg == nil || appMsg.Screen != ScreenREPL {
		t.Error("expected ScreenREPL on 'R' key")
	}
}

func TestShipModel_UpdateOpenBrowser(t *testing.T) {
	summary := ShipSummary{
		TaskDone:  1,
		TaskTotal: 1,
		SessionID: "sess-001",
		Duration:  "1m",
	}
	m := NewShipModel(summary, theme.Dark(), 0, 0)
	m.width = 80
	m.height = 24

	// 'o' key should not panic and should not emit screen change (V1: placeholder)
	_, appMsg := m.Update(tea.KeyMsg{Type: tea.KeyRunes, Runes: []rune{'o'}})
	if appMsg != nil {
		t.Error("expected no AppMsg on 'o' key (V1 placeholder)")
	}
}

func TestShipModel_UpdateCtrlC(t *testing.T) {
	summary := ShipSummary{
		TaskDone:  1,
		TaskTotal: 1,
		SessionID: "sess-001",
		Duration:  "1m",
	}
	m := NewShipModel(summary, theme.Dark(), 0, 0)

	_, appMsg := m.Update(tea.KeyMsg{Type: tea.KeyCtrlC})
	if appMsg != nil {
		t.Error("expected no AppMsg on ctrl+c")
	}
}

func TestShipModel_ViewNonEmpty(t *testing.T) {
	summary := ShipSummary{
		TaskDone:    5,
		TaskTotal:   6,
		TaskFailed:  1,
		TaskSkipped: 0,
		Commits: []git.CommitInfo{
			{ShortHash: "abc1234", Message: "Implement feature A"},
			{ShortHash: "def5678", Message: "Fix bug B"},
		},
		Duration:  "15m30s",
		SessionID: "sess-001",
	}
	m := NewShipModel(summary, theme.Dark(), 0, 0)
	m.width = 80
	m.height = 24

	view := m.View()
	if view == "" {
		t.Fatal("View() should not be empty")
	}
	if !strings.Contains(view, "Session Complete") {
		t.Error("expected 'Session Complete' in view")
	}
	if !strings.Contains(view, "sess-001") {
		t.Error("expected session ID in view")
	}
	if !strings.Contains(view, "15m30s") {
		t.Error("expected duration in view")
	}
	if !strings.Contains(view, "5/6") {
		t.Error("expected task summary in view")
	}
}

func TestShipModel_ViewLoading(t *testing.T) {
	summary := ShipSummary{
		SessionID: "sess-001",
		Duration:  "1m",
	}
	m := NewShipModel(summary, theme.Dark(), 0, 0)

	view := m.View()
	if !strings.Contains(view, "Loading") {
		t.Errorf("expected loading message, got: %s", view)
	}
}

func TestShipModel_ViewCommits(t *testing.T) {
	summary := ShipSummary{
		TaskDone:  1,
		TaskTotal: 1,
		Commits: []git.CommitInfo{
			{ShortHash: "a1b2c3d", Message: "Initial commit"},
			{ShortHash: "e4f5g6h", Message: "Add tests"},
		},
		Duration:  "2m",
		SessionID: "sess-001",
	}
	m := NewShipModel(summary, theme.Dark(), 0, 0)
	m.width = 80
	m.height = 24

	view := m.View()
	if !strings.Contains(view, "Commits") {
		t.Error("expected 'Commits' header in view")
	}
	if !strings.Contains(view, "a1b2c3d") {
		t.Error("expected first commit hash in view")
	}
	if !strings.Contains(view, "Initial commit") {
		t.Error("expected first commit message in view")
	}
}

func TestShipModel_ViewNoCommits(t *testing.T) {
	summary := ShipSummary{
		TaskDone:  1,
		TaskTotal: 1,
		Commits:   []git.CommitInfo{},
		Duration:  "1m",
		SessionID: "sess-001",
	}
	m := NewShipModel(summary, theme.Dark(), 0, 0)
	m.width = 80
	m.height = 24

	view := m.View()
	if !strings.Contains(view, "Session Complete") {
		t.Error("expected 'Session Complete' in view even with no commits")
	}
}

func TestShipModel_UpdateEscape(t *testing.T) {
	summary := ShipSummary{
		TaskDone:  1,
		TaskTotal: 1,
		SessionID: "sess-001",
		Duration:  "1m",
	}
	m := NewShipModel(summary, theme.Dark(), 0, 0)
	m.width = 80
	m.height = 24

	// Escape should return to REPL
	_, appMsg := m.Update(tea.KeyMsg{Type: tea.KeyEscape})
	if appMsg == nil || appMsg.Screen != ScreenREPL {
		t.Error("expected AppMsg with ScreenREPL on escape")
	}
}

func TestShipModel_UpdateUnknownKey(t *testing.T) {
	summary := ShipSummary{
		TaskDone:  1,
		TaskTotal: 1,
		SessionID: "sess-001",
		Duration:  "1m",
	}
	m := NewShipModel(summary, theme.Dark(), 0, 0)
	m.width = 80
	m.height = 24

	// Unknown key should not panic
	_, appMsg := m.Update(tea.KeyMsg{Type: tea.KeyRunes, Runes: []rune{'x'}})
	if appMsg != nil {
		t.Error("expected no AppMsg on unknown key")
	}
}
