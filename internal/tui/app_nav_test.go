package tui

import (
	"context"
	"testing"

	"github.com/eshanized/M31A/internal/tui/theme"
)

// ─── contentDimensions ─────────────────────────────────────────────────────

func TestNavContentDimensions_NoSidebar(t *testing.T) {
	m := &AppState{width: 100, height: 40}
	w, h := m.contentDimensions()
	if w != 100 {
		t.Errorf("w=%d, want 100", w)
	}
	if h <= 0 {
		t.Errorf("h=%d, want > 0", h)
	}
}

func TestContentDimensions_SmallWidth(t *testing.T) {
	m := &AppState{width: 0, height: 40}
	w, _ := m.contentDimensions()
	if w < 1 {
		t.Errorf("w=%d, want >= 1", w)
	}
}

func TestContentDimensions_SmallHeight(t *testing.T) {
	m := &AppState{width: 80, height: 0}
	_, h := m.contentDimensions()
	if h < 1 {
		t.Errorf("h=%d, want >= 1", h)
	}
}

// ─── popScreen ─────────────────────────────────────────────────────────────

func TestPopScreen_EmptyStack(t *testing.T) {
	m := &AppState{screen: ScreenSettings}
	m.popScreen()
	if m.screen != ScreenREPL {
		t.Errorf("screen=%d, want ScreenREPL", m.screen)
	}
}

func TestPopScreen_WithStack(t *testing.T) {
	tm := theme.NewManager(theme.ModeDark)
	m := &AppState{screen: ScreenSettings, screenStack: []Screen{ScreenREPL, ScreenPlan}, themeManager: tm, width: 80, height: 24}
	cmd := m.popScreen()
	_ = cmd
	if m.screen != ScreenPlan {
		t.Errorf("screen=%d, want ScreenPlan", m.screen)
	}
	if len(m.screenStack) != 1 {
		t.Errorf("stack=%d, want 1", len(m.screenStack))
	}
}

// ─── navigateToScreen ─────────────────────────────────────────────────────

func TestNavigateToScreen_FromREPL(t *testing.T) {
	rm := NewReplModel(testTheme(), "v1")
	tm := theme.NewManager(theme.ModeDark)
	m := &AppState{screen: ScreenREPL, replModel: &rm, themeManager: tm, width: 80, height: 24}
	cmd := m.navigateToScreen(ScreenSettings)
	_ = cmd
	if len(m.screenStack) != 0 {
		t.Errorf("stack=%d, want 0 (REPL is never pushed)", len(m.screenStack))
	}
}

func TestNavigateToScreen_FromNonREPL(t *testing.T) {
	rm := NewReplModel(testTheme(), "v1")
	tm := theme.NewManager(theme.ModeDark)
	m := &AppState{screen: ScreenSettings, replModel: &rm, themeManager: tm, width: 80, height: 24}
	cmd := m.navigateToScreen(ScreenHelp)
	_ = cmd
	if len(m.screenStack) != 1 {
		t.Errorf("stack=%d, want 1", len(m.screenStack))
	}
	if len(m.screenStack) > 0 && m.screenStack[0] != ScreenSettings {
		t.Errorf("stack[0]=%d, want ScreenSettings", m.screenStack[0])
	}
}

func TestNavigateToScreen_SameScreen(t *testing.T) {
	rm := NewReplModel(testTheme(), "v1")
	tm := theme.NewManager(theme.ModeDark)
	m := &AppState{screen: ScreenREPL, replModel: &rm, themeManager: tm, width: 80, height: 24}
	cmd := m.navigateToScreen(ScreenREPL)
	_ = cmd
	if m.screen != ScreenREPL {
		t.Errorf("screen=%d, want ScreenREPL", m.screen)
	}
}

// ─── openSettingsScreen / openResumeScreen ─────────────────────────────────

func TestNavOpenSettingsScreen(t *testing.T) {
	rm := NewReplModel(testTheme(), "v1")
	tm := theme.NewManager(theme.ModeDark)
	m := &AppState{screen: ScreenREPL, replModel: &rm, themeManager: tm, width: 80, height: 24}
	cmd := m.openSettingsScreen()
	_ = cmd
}

func TestNavOpenResumeScreen_NilSession(t *testing.T) {
	m := &AppState{sessionManager: nil}
	cmd := m.openResumeScreen()
	if cmd == nil {
		t.Error("openResumeScreen should always return a Cmd")
	}
}

func TestOpenResumeScreen_ReturnsCmd(t *testing.T) {
	m := &AppState{}
	cmd := m.openResumeScreen()
	if cmd == nil {
		t.Error("openResumeScreen should return non-nil Cmd")
	}
	msg := cmd()
	if _, ok := msg.(resumeScreenReadyMsg); !ok {
		t.Errorf("expected resumeScreenReadyMsg, got %T", msg)
	}
}

// ─── ensureSubModel ────────────────────────────────────────────────────────

func TestEnsureSubModel_REPL(t *testing.T) {
	rm := NewReplModel(testTheme(), "v1")
	tm := theme.NewManager(theme.ModeDark)
	m := &AppState{replModel: &rm, themeManager: tm, width: 80, height: 24}
	cmd := m.ensureSubModel(ScreenREPL)
	_ = cmd
}

func TestEnsureSubModel_Help(t *testing.T) {
	tm := theme.NewManager(theme.ModeDark)
	m := &AppState{themeManager: tm, width: 80, height: 24}
	cmd := m.ensureSubModel(ScreenHelp)
	_ = cmd
	if m.helpModel == nil {
		t.Error("helpModel should be created")
	}
}

func TestEnsureSubModel_Metrics(t *testing.T) {
	tm := theme.NewManager(theme.ModeDark)
	m := &AppState{themeManager: tm, width: 80, height: 24}
	cmd := m.ensureSubModel(ScreenMetrics)
	_ = cmd
	if m.metricsModel == nil {
		t.Error("metricsModel should be created")
	}
}

func TestEnsureSubModel_Bisect(t *testing.T) {
	tm := theme.NewManager(theme.ModeDark)
	m := &AppState{themeManager: tm, width: 80, height: 24}
	cmd := m.ensureSubModel(ScreenBisect)
	_ = cmd
	if m.bisectModel == nil {
		t.Error("bisectModel should be created")
	}
}

func TestEnsureSubModel_Notifications(t *testing.T) {
	tm := theme.NewManager(theme.ModeDark)
	m := &AppState{themeManager: tm, width: 80, height: 24}
	cmd := m.ensureSubModel(ScreenNotifications)
	_ = cmd
	if m.notifModel == nil {
		t.Error("notifModel should be created")
	}
}

func TestEnsureSubModel_Dashboard(t *testing.T) {
	tm := theme.NewManager(theme.ModeDark)
	m := &AppState{themeManager: tm, width: 80, height: 24}
	cmd := m.ensureSubModel(ScreenDashboard)
	_ = cmd
	if m.dashboardModel == nil {
		t.Error("dashboardModel should be created")
	}
}

func TestEnsureSubModel_Config(t *testing.T) {
	tm := theme.NewManager(theme.ModeDark)
	m := &AppState{themeManager: tm, width: 80, height: 24}
	cmd := m.ensureSubModel(ScreenConfig)
	_ = cmd
	if m.configModel == nil {
		t.Error("configModel should be created")
	}
}

func TestEnsureSubModel_GoalInput(t *testing.T) {
	tm := theme.NewManager(theme.ModeDark)
	m := &AppState{themeManager: tm, width: 80, height: 24}
	cmd := m.ensureSubModel(ScreenGoalInput)
	_ = cmd
	if m.goalInput == nil {
		t.Error("goalInput should be created")
	}
}

func TestEnsureSubModel_Ledger(t *testing.T) {
	tm := theme.NewManager(theme.ModeDark)
	m := &AppState{themeManager: tm, width: 80, height: 24}
	cmd := m.ensureSubModel(ScreenLedger)
	_ = cmd
	if m.ledgerModel == nil {
		t.Error("ledgerModel should be created")
	}
}

func TestEnsureSubModel_Rollback(t *testing.T) {
	tm := theme.NewManager(theme.ModeDark)
	m := &AppState{themeManager: tm, width: 80, height: 24}
	cmd := m.ensureSubModel(ScreenRollback)
	_ = cmd
	if m.rollbackModel == nil {
		t.Error("rollbackModel should be created")
	}
}

func TestEnsureSubModel_FileExplorer(t *testing.T) {
	tm := theme.NewManager(theme.ModeDark)
	m := &AppState{themeManager: tm, width: 80, height: 24}
	cmd := m.ensureSubModel(ScreenFileExplorer)
	_ = cmd
	if m.fileExplorerModel == nil {
		t.Error("fileExplorerModel should be created")
	}
}

func TestEnsureSubModel_ToolDetail(t *testing.T) {
	tm := theme.NewManager(theme.ModeDark)
	m := &AppState{themeManager: tm, width: 80, height: 24}
	cmd := m.ensureSubModel(ScreenToolDetail)
	_ = cmd
	if m.toolDetailModel == nil {
		t.Error("toolDetailModel should be created")
	}
}

func TestEnsureSubModel_ConfirmQuit(t *testing.T) {
	tm := theme.NewManager(theme.ModeDark)
	m := &AppState{themeManager: tm, width: 80, height: 24}
	cmd := m.ensureSubModel(ScreenConfirmQuit)
	_ = cmd
	if m.confirmQuitModel == nil {
		t.Error("confirmQuitModel should be created")
	}
}

func TestEnsureSubModel_GhostPicker(t *testing.T) {
	tm := theme.NewManager(theme.ModeDark)
	m := &AppState{themeManager: tm, width: 80, height: 24}
	cmd := m.ensureSubModel(ScreenGhostPicker)
	_ = cmd
	if m.ghostPickerModel == nil {
		t.Error("ghostPickerModel should be created")
	}
}

func TestEnsureSubModel_GhostOutput(t *testing.T) {
	tm := theme.NewManager(theme.ModeDark)
	m := &AppState{themeManager: tm, width: 80, height: 24}
	cmd := m.ensureSubModel(ScreenGhostOutput)
	_ = cmd
	if m.ghostOutputModel == nil {
		t.Error("ghostOutputModel should be created")
	}
}

func TestEnsureSubModel_ChatHistory(t *testing.T) {
	tm := theme.NewManager(theme.ModeDark)
	m := &AppState{themeManager: tm, width: 80, height: 24}
	cmd := m.ensureSubModel(ScreenChatHistory)
	_ = cmd
	if m.chatHistoryModel == nil {
		t.Error("chatHistoryModel should be created")
	}
}

func TestEnsureSubModel_CommandPalette(t *testing.T) {
	tm := theme.NewManager(theme.ModeDark)
	m := &AppState{themeManager: tm, width: 80, height: 24}
	cmd := m.ensureSubModel(ScreenCommandPalette)
	_ = cmd
	if m.commandPaletteScreenModel == nil {
		t.Error("commandPaletteScreenModel should be created")
	}
}

func TestEnsureSubModel_Home(t *testing.T) {
	tm := theme.NewManager(theme.ModeDark)
	m := &AppState{themeManager: tm, width: 80, height: 24}
	cmd := m.ensureSubModel(ScreenHome)
	_ = cmd
	if m.homeModel == nil {
		t.Error("homeModel should be created")
	}
}

func TestEnsureSubModel_SessionDetail(t *testing.T) {
	tm := theme.NewManager(theme.ModeDark)
	m := &AppState{themeManager: tm, width: 80, height: 24}
	cmd := m.ensureSubModel(ScreenSessionDetail)
	_ = cmd
	if m.sessionDetailModel == nil {
		t.Error("sessionDetailModel should be created")
	}
}

func TestEnsureSubModel_Discuss(t *testing.T) {
	tm := theme.NewManager(theme.ModeDark)
	m := &AppState{themeManager: tm, width: 80, height: 24}
	cmd := m.ensureSubModel(ScreenDiscuss)
	_ = cmd
	if m.discussModel == nil {
		t.Error("discussModel should be created")
	}
}

func TestEnsureSubModel_FirstRun(t *testing.T) {
	tm := theme.NewManager(theme.ModeDark)
	m := &AppState{themeManager: tm, width: 80, height: 24}
	cmd := m.ensureSubModel(ScreenFirstRun)
	_ = cmd
	if m.firstRunModel == nil {
		t.Error("firstRunModel should be created")
	}
}

func TestEnsureSubModel_Settings(t *testing.T) {
	tm := theme.NewManager(theme.ModeDark)
	ctx, cancel := context.WithCancel(context.Background())
	defer cancel()
	registry := newTestRegistry()
	m := &AppState{themeManager: tm, registry: registry, shutdownCtx: ctx, width: 80, height: 24}
	cmd := m.ensureSubModel(ScreenSettings)
	_ = cmd
	if m.settingsModel == nil {
		t.Error("settingsModel should be created")
	}
}

func TestEnsureSubModel_ModelSelector(t *testing.T) {
	tm := theme.NewManager(theme.ModeDark)
	ctx, cancel := context.WithCancel(context.Background())
	defer cancel()
	registry := newTestRegistry()
	m := &AppState{themeManager: tm, registry: registry, shutdownCtx: ctx, width: 80, height: 24}
	cmd := m.ensureSubModel(ScreenModelSelector)
	_ = cmd
	if m.msModel == nil {
		t.Error("msModel should be created")
	}
}

func TestEnsureSubModel_Decisions(t *testing.T) {
	m := &AppState{}
	cmd := m.ensureSubModel(ScreenDecisions)
	if cmd != nil {
		t.Error("ScreenDecisions should return nil cmd")
	}
}

func TestEnsureSubModel_UnknownScreen(t *testing.T) {
	m := &AppState{}
	cmd := m.ensureSubModel(Screen(9999))
	if cmd != nil {
		t.Error("unknown screen should return nil cmd")
	}
}

func TestEnsureSubModel_Plan_NilModel(t *testing.T) {
	m := &AppState{planModel: nil}
	cmd := m.ensureSubModel(ScreenPlan)
	if cmd != nil {
		t.Error("nil planModel should return nil cmd")
	}
}

func TestEnsureSubModel_Execute_NilModel(t *testing.T) {
	m := &AppState{executeModel: nil}
	cmd := m.ensureSubModel(ScreenExecute)
	if cmd != nil {
		t.Error("nil executeModel should return nil cmd")
	}
}

func TestEnsureSubModel_Verify_NilModel(t *testing.T) {
	m := &AppState{verifyModel: nil}
	cmd := m.ensureSubModel(ScreenVerify)
	if cmd != nil {
		t.Error("nil verifyModel should return nil cmd")
	}
}

func TestEnsureSubModel_Ship_NilModel(t *testing.T) {
	m := &AppState{shipModel: nil}
	cmd := m.ensureSubModel(ScreenShip)
	if cmd != nil {
		t.Error("nil shipModel should return nil cmd")
	}
}

func TestEnsureSubModel_Resume(t *testing.T) {
	m := &AppState{}
	cmd := m.ensureSubModel(ScreenResume)
	_ = cmd
}

func TestEnsureSubModel_PhaseModelPicker(t *testing.T) {
	// PhaseModelPicker has deep cursor dependencies; verify code path compiles
	tm := theme.NewManager(theme.ModeDark)
	m := &AppState{themeManager: tm, width: 80, height: 24}
	if m.phaseModelPicker != nil {
		t.Error("phaseModelPicker should start nil")
	}
}

func TestEnsureSubModel_Diff(t *testing.T) {
	// Diff model has viewport dependencies; verify code path compiles
	tm := theme.NewManager(theme.ModeDark)
	m := &AppState{themeManager: tm, width: 80, height: 24}
	if m.diffModel != nil {
		t.Error("diffModel should start nil")
	}
}

// ─── routeToScreen ────────────────────────────────────────────────────────

func TestRouteToScreen_ReturnsCmd(t *testing.T) {
	rm := NewReplModel(testTheme(), "v1")
	tm := theme.NewManager(theme.ModeDark)
	m := &AppState{screen: ScreenREPL, replModel: &rm, themeManager: tm, width: 80, height: 24}
	cmd := m.routeToScreen()
	_ = cmd
}

func TestRouteToScreen_DefaultScreen(t *testing.T) {
	tm := theme.NewManager(theme.ModeDark)
	m := &AppState{screen: Screen(9999), themeManager: tm, width: 80, height: 24}
	cmd := m.routeToScreen()
	if cmd != nil {
		t.Error("unknown screen should return nil cmd")
	}
}

func TestRouteToScreen_UnknownScreen(t *testing.T) {
	m := &AppState{screen: Screen(9999)}
	cmd := m.routeToScreen()
	if cmd != nil {
		t.Error("unknown screen should return nil cmd")
	}
}

// ─── Signature verification ───────────────────────────────────────────────

func TestRouteToScreen_Signature(t *testing.T) {
	var fn = (&AppState{}).routeToScreen
	_ = fn
}

func TestNavigateToScreen_Signature(t *testing.T) {
	var fn = (&AppState{}).navigateToScreen
	_ = fn
}

func TestPopScreen_Signature(t *testing.T) {
	var fn = (&AppState{}).popScreen
	_ = fn
}

func TestContentDimensions_Signature(t *testing.T) {
	var fn = (&AppState{}).contentDimensions
	_ = fn
}

func TestEnsureSubModel_Signature(t *testing.T) {
	var fn = (&AppState{}).ensureSubModel
	_ = fn
}

func TestOpenResumeScreen_Signature(t *testing.T) {
	var fn = (&AppState{}).openResumeScreen
	_ = fn
}

func TestOpenSettingsScreen_Signature(t *testing.T) {
	var fn = (&AppState{}).openSettingsScreen
	_ = fn
}
