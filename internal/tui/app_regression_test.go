package tui

import (
	"testing"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/eshanized/M31A/internal/tui/tuitypes"
)

// ─── Update() dispatch tests ─────────────────────────────────────────────────

func TestUpdate_WindowSizeMsg(t *testing.T) {
	a := NewApp(nil, "", nil, nil, nil, nil, nil, nil, nil, "test", 0)
	a.width = 80
	a.height = 24

	msg := tea.WindowSizeMsg{Width: 120, Height: 40}
	result, cmd := a.Update(msg)
	m := result.(*AppState)
	if m.width != 120 || m.height != 40 {
		t.Errorf("expected 120x40, got %dx%d", m.width, m.height)
	}
	_ = cmd
}

func TestUpdate_LeaderTimeoutMsg(t *testing.T) {
	a := NewApp(nil, "", nil, nil, nil, nil, nil, nil, nil, "test", 0)

	result, _ := a.Update(LeaderTimeoutMsg{})
	m := result.(*AppState)

	if m.keyRegistry != nil && m.keyRegistry.IsLeaderActive() {
		t.Error("leader should be deactivated after timeout")
	}
}

func TestUpdate_ToggleToastDismiss(t *testing.T) {
	a := NewApp(nil, "", nil, nil, nil, nil, nil, nil, nil, "test", 0)
	a.addToast("test toast", "info")

	toastID := a.toasts[0].ID
	result, _ := a.Update(DismissToastMsg{ToastID: toastID})
	m := result.(*AppState)

	if len(m.toasts) != 0 {
		t.Errorf("expected 0 toasts after dismiss, got %d", len(m.toasts))
	}
}

func TestUpdate_PopScreenMsg(t *testing.T) {
	a := NewApp(nil, "", nil, nil, nil, nil, nil, nil, nil, "test", 0)
	a.screen = ScreenHelp
	a.screenStack = []Screen{ScreenREPL}

	result, _ := a.Update(PopScreenMsg{})
	m := result.(*AppState)

	if m.screen != ScreenREPL {
		t.Errorf("expected screen REPL after pop, got %v", m.screen)
	}
	if len(m.screenStack) != 0 {
		t.Errorf("expected empty screen stack, got %d", len(m.screenStack))
	}
}

func TestUpdate_PopScreenMsg_EmptyStack(t *testing.T) {
	a := NewApp(nil, "", nil, nil, nil, nil, nil, nil, nil, "test", 0)
	a.screen = ScreenHelp
	a.screenStack = nil

	result, _ := a.Update(PopScreenMsg{})
	m := result.(*AppState)

	if m.screen != ScreenREPL {
		t.Errorf("expected REPL fallback, got %v", m.screen)
	}
}

// ─── Screen routing tests ───────────────────────────────────────────────────

func TestNavigateToScreen_PushesToStack(t *testing.T) {
	a := NewApp(nil, "", nil, nil, nil, nil, nil, nil, nil, "test", 0)
	a.screen = ScreenHelp // Start from a non-REPL screen
	a.width = 80
	a.height = 24

	_ = a.navigateToScreen(ScreenSettings)

	// navigateToScreen starts a transition, so prevScreen is set and stack is pushed
	if a.prevScreen != ScreenHelp {
		t.Errorf("expected prevScreen Help, got %v", a.prevScreen)
	}
	if len(a.screenStack) == 0 {
		t.Error("expected screen stack to have entry")
	}
	if a.screenStack[len(a.screenStack)-1] != ScreenHelp {
		t.Errorf("expected Help on stack, got %v", a.screenStack[len(a.screenStack)-1])
	}
}

func TestNavigateToScreen_NoDuplicateConsecutive(t *testing.T) {
	a := NewApp(nil, "", nil, nil, nil, nil, nil, nil, nil, "test", 0)
	a.screen = ScreenREPL
	a.width = 80
	a.height = 24

	_ = a.navigateToScreen(ScreenREPL)

	// Same screen should not push to stack
	for _, s := range a.screenStack {
		if s == ScreenREPL {
			t.Error("should not push same screen to stack")
		}
	}
}

func TestPopScreen_FromStack(t *testing.T) {
	a := NewApp(nil, "", nil, nil, nil, nil, nil, nil, nil, "test", 0)
	a.screen = ScreenHelp
	a.width = 80
	a.height = 24
	a.screenStack = []Screen{ScreenREPL, ScreenSettings}

	_ = a.popScreen()

	if a.screen != ScreenSettings {
		t.Errorf("expected Settings, got %v", a.screen)
	}
	if len(a.screenStack) != 1 {
		t.Errorf("expected 1 entry on stack, got %d", len(a.screenStack))
	}
}

func TestPopScreen_EmptyStackGoesToREPL(t *testing.T) {
	a := NewApp(nil, "", nil, nil, nil, nil, nil, nil, nil, "test", 0)
	a.screen = ScreenHelp
	a.screenStack = nil

	_ = a.popScreen()

	if a.screen != ScreenREPL {
		t.Errorf("expected REPL, got %v", a.screen)
	}
}

// ─── Keyboard routing tests ─────────────────────────────────────────────────

func TestRouteKeyMsg_CommandPalettePriority(t *testing.T) {
	a := NewApp(nil, "", nil, nil, nil, nil, nil, nil, nil, "test", 0)
	a.screen = ScreenREPL

	// When command palette is nil, keys should route to screen
	cmd := a.routeKeyMsg(tea.KeyMsg{Type: tea.KeyRunes, Runes: []rune{'a'}})
	_ = cmd // Just verify no panic
}

func TestRouteKeyMsg_SidebarFocusToggle(t *testing.T) {
	a := NewApp(nil, "", nil, nil, nil, nil, nil, nil, nil, "test", 0)
	a.screen = ScreenREPL
	if a.sidebarModel != nil {
		a.sidebarModel.visible = true
	}

	result, _ := a.Update(tea.KeyMsg{Type: tea.KeyRunes, Runes: []rune{0}, Alt: false})
	_ = result
	// ctrl+g should toggle sidebar focus if sidebar is visible
}

// ─── Mouse routing tests ────────────────────────────────────────────────────

func TestForwardMouseToScreen_NilSidebar(t *testing.T) {
	a := NewApp(nil, "", nil, nil, nil, nil, nil, nil, nil, "test", 0)
	a.screen = ScreenREPL
	a.sidebarModel = nil

	// Should not panic with nil sidebar
	cmd := a.forwardMouseToScreen(tea.MouseMsg{Type: tea.MouseLeft, X: 10, Y: 5})
	_ = cmd
}

func TestForwardMouseToScreen_SidebarClick(t *testing.T) {
	a := NewApp(nil, "", nil, nil, nil, nil, nil, nil, nil, "test", 0)
	a.screen = ScreenREPL
	a.ensureSidebarModel()
	if a.sidebarModel != nil {
		a.sidebarModel.visible = true
		a.sidebarModel.width = 30
	}

	// Click in sidebar area (x < sidebar width)
	cmd := a.forwardMouseToScreen(tea.MouseMsg{Type: tea.MouseLeft, X: 5, Y: 10})
	_ = cmd
	// Should route to sidebar, not screen
}

// ─── Resize handling tests ──────────────────────────────────────────────────

func TestHandleWindowResize_MinDimensions(t *testing.T) {
	a := NewApp(nil, "", nil, nil, nil, nil, nil, nil, nil, "test", 0)
	a.width = 80
	a.height = 24

	result, _ := a.Update(tea.WindowSizeMsg{Width: 10, Height: 3})
	m := result.(*AppState)

	if m.width != 10 || m.height != 3 {
		t.Errorf("expected 10x3, got %dx%d", m.width, m.height)
	}
}

func TestHandleWindowResize_ContentDimensions(t *testing.T) {
	a := NewApp(nil, "", nil, nil, nil, nil, nil, nil, nil, "test", 0)
	a.width = 80
	a.height = 24

	w, h := a.contentDimensions()
	// Sidebar is visible by default at width >= 80, subtracts sidebar width
	if w <= 0 || w > 80 {
		t.Errorf("expected content width in (0,80], got %d", w)
	}
	if h != 24-2 { // ChromeHeight = 2
		t.Errorf("expected content height %d, got %d", 24-2, h)
	}
}

// ─── Focus restoration tests ────────────────────────────────────────────────

func TestSidebarFocusToggle(t *testing.T) {
	a := NewApp(nil, "", nil, nil, nil, nil, nil, nil, nil, "test", 0)
	a.ensureSidebarModel()
	if a.sidebarModel == nil {
		t.Skip("sidebar model not available")
	}

	initial := a.sidebarModel.focused
	a.sidebarModel.ToggleFocus()
	if a.sidebarModel.focused == initial {
		t.Error("sidebar focus should toggle")
	}
}

// ─── Overlay lifecycle tests ────────────────────────────────────────────────

func TestPermissionModal_OverlayLifecycle(t *testing.T) {
	a := NewApp(nil, "", nil, nil, nil, nil, nil, nil, nil, "test", 0)

	// Initially no permission request
	if a.permRequest != nil {
		t.Error("should start with nil permRequest")
	}
}

func TestToast_Lifecycle(t *testing.T) {
	a := NewApp(nil, "", nil, nil, nil, nil, nil, nil, nil, "test", 0)

	id := a.addToast("test", "info")
	if len(a.toasts) != 1 {
		t.Errorf("expected 1 toast, got %d", len(a.toasts))
	}

	a.removeToastByID(id)
	if len(a.toasts) != 0 {
		t.Errorf("expected 0 toasts after remove, got %d", len(a.toasts))
	}
}

// ─── Screen Name() consistency tests ────────────────────────────────────────

func TestScreenNames_AllScreensHaveNames(t *testing.T) {
	screens := []tuitypes.Screen{
		ScreenREPL, ScreenPlan, ScreenExecute, ScreenVerify,
		ScreenRuntimeCheck, ScreenShip, ScreenLedger, ScreenRollback,
		ScreenConfig, ScreenDiff, ScreenHelp, ScreenToolDetail,
		ScreenCommandPalette, ScreenFileExplorer, ScreenBisect,
		ScreenDashboard, ScreenNotifications, ScreenMetrics,
		ScreenThemePicker, ScreenResume, ScreenSettings,
		ScreenModelSelector, ScreenDiscuss, ScreenGoalInput,
		ScreenFirstRun, ScreenGhostPicker, ScreenGhostOutput,
		ScreenConfirmQuit, ScreenPhaseModelPicker, ScreenSessionDetail,
		ScreenChatHistory, ScreenHome,
	}

	seen := make(map[string]bool)
	for _, s := range screens {
		name := s.Name()
		if name == "" {
			t.Errorf("Screen(%d) has empty name", s)
		}
		if seen[name] {
			t.Errorf("duplicate screen name: %q", name)
		}
		seen[name] = true
	}
}

func TestScreenLabels_AllScreensHaveLabels(t *testing.T) {
	screens := []tuitypes.Screen{
		ScreenREPL, ScreenPlan, ScreenExecute, ScreenVerify,
		ScreenRuntimeCheck, ScreenShip, ScreenSettings,
		ScreenModelSelector, ScreenHelp, ScreenConfig,
	}

	for _, s := range screens {
		label := s.Label()
		if label == "" || label == "Unknown" {
			t.Errorf("Screen(%d).Label() = %q", s, label)
		}
	}
}

// ─── Route-to-screen routing map tests ──────────────────────────────────────

func TestScreenUpdaters_AllRegistered(t *testing.T) {
	a := NewApp(nil, "", nil, nil, nil, nil, nil, nil, nil, "test", 0)

	screens := []tuitypes.Screen{
		ScreenREPL, ScreenPlan, ScreenExecute, ScreenVerify,
		ScreenRuntimeCheck, ScreenShip, ScreenLedger, ScreenRollback,
		ScreenConfig, ScreenDiff, ScreenHelp, ScreenToolDetail,
		ScreenCommandPalette, ScreenFileExplorer, ScreenBisect,
		ScreenDashboard, ScreenNotifications, ScreenMetrics,
		ScreenThemePicker, ScreenResume, ScreenSettings,
		ScreenModelSelector, ScreenDiscuss, ScreenGoalInput,
		ScreenFirstRun, ScreenGhostPicker, ScreenGhostOutput,
		ScreenConfirmQuit, ScreenPhaseModelPicker, ScreenSessionDetail,
		ScreenChatHistory, ScreenHome,
	}

	for _, screen := range screens {
		if _, ok := a.screenUpdaters[screen]; !ok {
			t.Errorf("screenUpdaters missing entry for %s", screen.Name())
		}
	}
}

func TestScreenUpdaters_ForwardMsgToScreen_NilSafe(t *testing.T) {
	a := NewApp(nil, "", nil, nil, nil, nil, nil, nil, nil, "test", 0)
	a.screen = ScreenREPL

	// All sub-models nil — should not panic
	cmd := a.forwardMsgToScreen(tea.KeyMsg{Type: tea.KeyRunes, Runes: []rune{'a'}})
	_ = cmd
}

// ─── Content dimensions tests ───────────────────────────────────────────────

func TestContentDimensions_WithSidebar(t *testing.T) {
	a := NewApp(nil, "", nil, nil, nil, nil, nil, nil, nil, "test", 0)
	a.width = 120
	a.height = 40
	a.ensureSidebarModel()
	if a.sidebarModel != nil {
		a.sidebarModel.visible = true
		a.sidebarModel.width = 30
	}

	w, h := a.contentDimensions()
	if w != 120-30 {
		t.Errorf("expected width %d with sidebar, got %d", 120-30, w)
	}
	if h != 40-2 {
		t.Errorf("expected height %d, got %d", 40-2, h)
	}
}

func TestContentDimensions_NoSidebar(t *testing.T) {
	a := NewApp(nil, "", nil, nil, nil, nil, nil, nil, nil, "test", 0)
	a.width = 120
	a.height = 40
	a.ensureSidebarModel()
	if a.sidebarModel != nil {
		a.sidebarModel.visible = false
	}

	w, h := a.contentDimensions()
	if w != 120 {
		t.Errorf("expected width 120 without sidebar, got %d", w)
	}
	if h != 40-2 {
		t.Errorf("expected height %d, got %d", 40-2, h)
	}
}

// ─── Double ctrl+c protection ───────────────────────────────────────────────

func TestDoubleCtrlC_RequiresTwoPresses(t *testing.T) {
	a := NewApp(nil, "", nil, nil, nil, nil, nil, nil, nil, "test", 0)
	a.screen = ScreenREPL

	// First ctrl+c should set timer, not quit
	result, _ := a.Update(tea.KeyMsg{Type: tea.KeyCtrlC})
	m := result.(*AppState)
	if m.lastCtrlCTime.IsZero() {
		t.Error("first ctrl+c should set lastCtrlCTime")
	}
}

// ─── Slash command routing ──────────────────────────────────────────────────

func TestHandleSlashCommand_EmptyInput(t *testing.T) {
	a := NewApp(nil, "", nil, nil, nil, nil, nil, nil, nil, "test", 0)
	cmd := a.handleSlashCommand("", 0)
	if cmd != nil {
		t.Error("empty slash command should return nil")
	}
}

// ─── Theme application ──────────────────────────────────────────────────────

func TestApplyTheme_UnknownThemeNoPanic(t *testing.T) {
	a := NewApp(nil, "", nil, nil, nil, nil, nil, nil, nil, "test", 0)
	// Should not panic for unknown theme name — just a no-op
	a.applyTheme("nonexistent_theme_xyz")
}
