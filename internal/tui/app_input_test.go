package tui

import (
	"testing"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/eshanized/M31A/internal/tui/theme"
	"github.com/eshanized/M31A/pkg/types"
)

func TestApplyTheme_Dark(t *testing.T) {
	rm := NewReplModel(testTheme(), "v1")
	tm := theme.NewManager(theme.ModeDark)
	m := &AppState{
		replModel:    &rm,
		themeManager: tm,
		width:        80,
		height:       24,
	}
	m.applyTheme("dark")
	// Should not panic and theme should be applied
}

func TestApplyTheme_Light(t *testing.T) {
	rm := NewReplModel(testTheme(), "v1")
	tm := theme.NewManager(theme.ModeDark)
	m := &AppState{
		replModel:    &rm,
		themeManager: tm,
		width:        80,
		height:       24,
	}
	m.applyTheme("light")
	// Should not panic
}

func TestApplyTheme_Auto(t *testing.T) {
	rm := NewReplModel(testTheme(), "v1")
	tm := theme.NewManager(theme.ModeDark)
	m := &AppState{
		replModel:    &rm,
		themeManager: tm,
		width:        80,
		height:       24,
	}
	m.applyTheme("auto")
	// Should not panic
}

func TestApplyTheme_WithConfig(t *testing.T) {
	rm := NewReplModel(testTheme(), "v1")
	tm := theme.NewManager(theme.ModeDark)
	m := &AppState{
		replModel:    &rm,
		themeManager: tm,
		config:       &testConfig{},
		width:        80,
		height:       24,
	}
	m.applyTheme("dark")
	// Config should have theme set
	if m.config.UI.Theme != "dark" {
		t.Errorf("config.UI.Theme=%q, want dark", m.config.UI.Theme)
	}
}

func TestApplyTheme_WithNilConfig(t *testing.T) {
	rm := NewReplModel(testTheme(), "v1")
	tm := theme.NewManager(theme.ModeDark)
	m := &AppState{
		replModel:    &rm,
		themeManager: tm,
		config:       nil,
		width:        80,
		height:       24,
	}
	m.applyTheme("dark")
	// Should not panic with nil config
}

func TestHandleWindowResize_Basic(t *testing.T) {
	rm := NewReplModel(testTheme(), "v1")
	tm := theme.NewManager(theme.ModeDark)
	m := &AppState{
		replModel:    &rm,
		themeManager: tm,
		width:        80,
		height:       24,
	}
	msg := tea.WindowSizeMsg{Width: 100, Height: 30}
	cmd := m.handleWindowResize(msg)
	_ = cmd
	// Width and height should be updated via sub-models
}

func TestHandleWindowResize_SmallTerminal(t *testing.T) {
	rm := NewReplModel(testTheme(), "v1")
	tm := theme.NewManager(theme.ModeDark)
	m := &AppState{
		replModel:    &rm,
		themeManager: tm,
		width:        80,
		height:       24,
	}
	msg := tea.WindowSizeMsg{Width: 1, Height: 1}
	cmd := m.handleWindowResize(msg)
	_ = cmd
	// Should not panic with very small terminal
}

func TestHandleWindowResize_NilReplModel(t *testing.T) {
	tm := theme.NewManager(theme.ModeDark)
	m := &AppState{
		themeManager: tm,
		width:        80,
		height:       24,
	}
	msg := tea.WindowSizeMsg{Width: 100, Height: 30}
	cmd := m.handleWindowResize(msg)
	if cmd != nil {
		t.Error("nil replModel should return nil cmd")
	}
}

func TestHandleWindowResize_WithSidebar(t *testing.T) {
	rm := NewReplModel(testTheme(), "v1")
	tm := theme.NewManager(theme.ModeDark)
	sm := NewSidebarModel(nil, tm.Current())
	sm.visible = true
	m := &AppState{
		replModel:    &rm,
		sidebarModel: sm,
		themeManager: tm,
		width:        80,
		height:       24,
	}
	msg := tea.WindowSizeMsg{Width: 100, Height: 30}
	cmd := m.handleWindowResize(msg)
	_ = cmd
}

func TestRouteKeyMsg_NilPalette(t *testing.T) {
	rm := NewReplModel(testTheme(), "v1")
	tm := theme.NewManager(theme.ModeDark)
	m := &AppState{
		replModel:    &rm,
		themeManager: tm,
		width:        80,
		height:       24,
	}
	msg := tea.KeyMsg{Type: tea.KeyRunes, Runes: []rune{'a'}}
	cmd := m.routeKeyMsg(msg)
	_ = cmd
}

func TestRouteKeyMsg_WithPendingConfirm(t *testing.T) {
	rm := NewReplModel(testTheme(), "v1")
	tm := theme.NewManager(theme.ModeDark)
	m := &AppState{
		replModel:      &rm,
		themeManager:   tm,
		pendingConfirm: &CommandResult{ConfirmPrompt: "Are you sure?"},
		width:          80,
		height:         24,
	}
	// Press 'n' to cancel
	msg := tea.KeyMsg{Type: tea.KeyRunes, Runes: []rune{'n'}}
	cmd := m.routeKeyMsg(msg)
	_ = cmd
	if m.pendingConfirm != nil {
		t.Error("pendingConfirm should be nil after pressing n")
	}
}

func TestRouteKeyMsg_WithPendingIntent_Y(t *testing.T) {
	rm := NewReplModel(testTheme(), "v1")
	tm := theme.NewManager(theme.ModeDark)
	intent := types.IntentResult{Intent: types.IntentFeature}
	m := &AppState{
		replModel:          &rm,
		themeManager:       tm,
		pendingIntent:      &intent,
		pendingIntentInput: "test goal",
		width:              80,
		height:             24,
	}
	msg := tea.KeyMsg{Type: tea.KeyRunes, Runes: []rune{'y'}}
	cmd := m.routeKeyMsg(msg)
	_ = cmd
	if m.pendingIntent != nil {
		t.Error("pendingIntent should be nil after pressing y")
	}
}

func TestRouteKeyMsg_WithPendingIntent_N(t *testing.T) {
	rm := NewReplModel(testTheme(), "v1")
	tm := theme.NewManager(theme.ModeDark)
	intent := types.IntentResult{Intent: types.IntentFeature}
	// Need a minimal registry to avoid nil dereference when intent is dismissed
	registry := newTestRegistry()
	m := &AppState{
		replModel:          &rm,
		themeManager:       tm,
		registry:           registry,
		pendingIntent:      &intent,
		pendingIntentInput: "test goal",
		width:              80,
		height:             24,
	}
	msg := tea.KeyMsg{Type: tea.KeyRunes, Runes: []rune{'n'}}
	cmd := m.routeKeyMsg(msg)
	_ = cmd
	if m.pendingIntent != nil {
		t.Error("pendingIntent should be nil after pressing n")
	}
}

func TestHandleKeyAction_OpenHome(t *testing.T) {
	rm := NewReplModel(testTheme(), "v1")
	tm := theme.NewManager(theme.ModeDark)
	m := &AppState{
		replModel:    &rm,
		themeManager: tm,
		width:        80,
		height:       24,
	}
	cmd := m.handleKeyAction("open_home")
	_ = cmd
}

func TestHandleKeyAction_NewSession(t *testing.T) {
	rm := NewReplModel(testTheme(), "v1")
	tm := theme.NewManager(theme.ModeDark)
	m := &AppState{
		replModel:    &rm,
		themeManager: tm,
		width:        80,
		height:       24,
	}
	cmd := m.handleKeyAction("new_session")
	_ = cmd
}

func TestHandleKeyAction_ToggleSidebar(t *testing.T) {
	rm := NewReplModel(testTheme(), "v1")
	tm := theme.NewManager(theme.ModeDark)
	sm := NewSidebarModel(nil, tm.Current())
	m := &AppState{
		replModel:    &rm,
		sidebarModel: sm,
		themeManager: tm,
		width:        80,
		height:       24,
	}
	cmd := m.handleKeyAction("toggle_sidebar")
	_ = cmd
}

func TestHandleKeyAction_CancelStream(t *testing.T) {
	rm := NewReplModel(testTheme(), "v1")
	tm := theme.NewManager(theme.ModeDark)
	cancelled := false
	m := &AppState{
		replModel:      &rm,
		themeManager:   tm,
		streamCancelFn: func() { cancelled = true },
		width:          80,
		height:         24,
	}
	cmd := m.handleKeyAction("cancel_stream")
	_ = cmd
	if !cancelled {
		t.Error("streamCancelFn should have been called")
	}
	if m.streamCancelFn != nil {
		t.Error("streamCancelFn should be nil after cancel")
	}
}

func TestHandleKeyAction_Unknown(t *testing.T) {
	m := &AppState{}
	cmd := m.handleKeyAction("unknown_action")
	if cmd != nil {
		t.Error("unknown action should return nil cmd")
	}
}

// Verify signatures
func TestHandleWindowResize_Signature(t *testing.T) {
	var fn = (&AppState{}).handleWindowResize
	_ = fn
}

func TestRouteKeyMsg_Signature(t *testing.T) {
	var fn = (&AppState{}).routeKeyMsg
	_ = fn
}

func TestHandleKeyAction_Signature(t *testing.T) {
	var fn = (&AppState{}).handleKeyAction
	_ = fn
}

func TestApplyTheme_Signature(t *testing.T) {
	var fn = (&AppState{}).applyTheme
	_ = fn
}
