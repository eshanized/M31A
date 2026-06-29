package tui

import (
	"testing"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/eshanized/M31A/internal/tui/tuitypes"
)

// TestScreenName verifies that Screen.Name() returns the same lowercase slugs
// that the sidebar uses for keyboard hint lookups.
func TestScreenName(t *testing.T) {
	tests := []struct {
		screen tuitypes.Screen
		want   string
	}{
		{ScreenREPL, "repl"},
		{ScreenExecute, "execute"},
		{ScreenPlan, "plan"},
		{ScreenVerify, "verify"},
		{ScreenRuntimeCheck, "runtime"},
		{ScreenShip, "ship"},
		{ScreenDiscuss, "discuss"},
		{ScreenSettings, "settings"},
		{ScreenHelp, "help"},
		{ScreenChatHistory, "chathistory"},
		{ScreenConfig, "config"},
		{ScreenResume, "resume"},
		{ScreenRollback, "rollback"},
		{ScreenDiff, "diff"},
		{ScreenModelSelector, "modelselector"},
		{ScreenCommandPalette, "cmdpalette"},
		{ScreenPhaseModelPicker, "phasempicker"},
		{ScreenSessionDetail, "session"},
		{ScreenFileExplorer, "fileexplorer"},
		{ScreenConfirmQuit, "confirmquit"},
		{ScreenDashboard, "dashboard"},
		{ScreenMetrics, "metrics"},
		{ScreenLedger, "ledger"},
		{ScreenHome, "home"},
		{ScreenFirstRun, "firstrun"},
		{ScreenGoalInput, "goalinput"},
		{ScreenGhostPicker, "ghostpicker"},
		{ScreenGhostOutput, "ghostoutput"},
		{ScreenToolDetail, "tooldetail"},
		{ScreenNotifications, "notifications"},
		{ScreenBisect, "bisect"},
		{ScreenPermission, "permission"},
	}

	for _, tt := range tests {
		t.Run(tt.want, func(t *testing.T) {
			got := tt.screen.Name()
			if got != tt.want {
				t.Errorf("Screen(%d).Name() = %q, want %q", tt.screen, got, tt.want)
			}
		})
	}
}

// TestScreenNameUniqueness verifies all screen names are unique.
func TestScreenNameUniqueness(t *testing.T) {
	seen := make(map[string]tuitypes.Screen)
	for i := tuitypes.Screen(0); i <= 32; i++ {
		name := i.Name()
		if name == "" {
			continue
		}
		if prev, ok := seen[name]; ok {
			t.Errorf("duplicate Name() %q: Screen(%d) and Screen(%d)", name, prev, i)
		}
		seen[name] = i
	}
}

// TestScreenLabel verifies Label() returns non-empty strings for all screens.
func TestScreenLabel(t *testing.T) {
	for i := tuitypes.Screen(0); i <= 32; i++ {
		label := i.Label()
		if label == "" || label == "Unknown" {
			t.Errorf("Screen(%d).Label() = %q, want a meaningful label", i, label)
		}
	}
}

// TestInitScreenUpdaters verifies that the routing map is initialized with
// entries for all standard screens.
func TestInitScreenUpdaters(t *testing.T) {
	a := NewApp(nil, "", nil, nil, nil, nil, nil, nil, nil, "test", 0)

	if a.screenUpdaters == nil {
		t.Fatal("screenUpdaters not initialized")
	}

	screens := []tuitypes.Screen{
		ScreenREPL, ScreenPlan, ScreenExecute, ScreenVerify,
		ScreenRuntimeCheck, ScreenShip, ScreenLedger, ScreenRollback,
		ScreenConfig, ScreenDiff, ScreenHelp, ScreenToolDetail,
		ScreenCommandPalette, ScreenFileExplorer, ScreenBisect,
		ScreenDashboard, ScreenNotifications, ScreenMetrics,
		ScreenResume, ScreenSettings,
		ScreenModelSelector, ScreenDiscuss, ScreenGoalInput,
		ScreenFirstRun, ScreenGhostPicker, ScreenGhostOutput,
		ScreenConfirmQuit, ScreenPhaseModelPicker, ScreenSessionDetail,
		ScreenChatHistory, ScreenHome,
	}

	for _, screen := range screens {
		if _, ok := a.screenUpdaters[screen]; !ok {
			t.Errorf("screenUpdaters missing entry for %s (Screen(%d))", screen.Name(), screen)
		}
	}
}

// TestForwardMsgToScreen_NilModels verifies that forwardMsgToScreen returns nil
// when no models are initialized (all nil), matching original behavior.
func TestForwardMsgToScreen_NilModels(t *testing.T) {
	a := NewApp(nil, "", nil, nil, nil, nil, nil, nil, nil, "test", 0)
	a.screen = ScreenREPL

	cmd := a.forwardMsgToScreen(tea.KeyMsg{Type: tea.KeyRunes, Runes: []rune{'a'}})
	if cmd != nil {
		t.Error("forwardMsgToScreen should return nil when replModel is nil")
	}
}

// TestForwardMouseToScreen_NilModels verifies mouse forwarding with nil models.
func TestForwardMouseToScreen_NilModels(t *testing.T) {
	a := NewApp(nil, "", nil, nil, nil, nil, nil, nil, nil, "test", 0)
	a.screen = ScreenREPL

	cmd := a.forwardMouseToScreen(tea.MouseMsg{Action: tea.MouseActionPress, Button: tea.MouseButtonLeft, X: 10, Y: 5})
	if cmd != nil {
		t.Error("forwardMouseToScreen should return nil when models are nil")
	}
}

// TestRouteKeyToScreen_Permission verifies that permission screen gets special handling.
func TestRouteKeyToScreen_Permission(t *testing.T) {
	a := NewApp(nil, "", nil, nil, nil, nil, nil, nil, nil, "test", 0)
	a.screen = ScreenPermission

	// Without a permission request, handlePermissionKey should not panic
	cmd := a.routeKeyToScreen(tea.KeyMsg{Type: tea.KeyRunes, Runes: []rune{'y'}})
	_ = cmd // just verify no panic
}

// TestRouteKeyToScreen_UnknownScreen verifies that unknown screens return nil.
func TestRouteKeyToScreen_UnknownScreen(t *testing.T) {
	a := NewApp(nil, "", nil, nil, nil, nil, nil, nil, nil, "test", 0)
	a.screen = tuitypes.Screen(99) // invalid screen

	cmd := a.routeKeyToScreen(tea.KeyMsg{Type: tea.KeyRunes, Runes: []rune{'a'}})
	if cmd != nil {
		t.Error("routeKeyToScreen should return nil for unknown screen")
	}
}
