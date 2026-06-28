package tui

import (
	"testing"
	"time"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/eshanized/M31A/internal/tools"
	"github.com/eshanized/M31A/internal/tui/theme"
	"github.com/eshanized/M31A/internal/types"
)

func testAppState() *AppState {
	rm := NewReplModel(testTheme(), "v1")
	tm := theme.NewManager(theme.ModeDark)
	return &AppState{
		screen:       ScreenREPL,
		replModel:    &rm,
		themeManager: tm,
		toasts:       []Toast{},
		toastTimers:  make(map[int]*time.Timer),
		width:        80,
		height:       24,
	}
}

// ═══ countDone ═══

func TestCountDone(t *testing.T) {
	tests := []struct {
		name  string
		tasks []types.Task
		want  int
	}{
		{"nil", nil, 0},
		{"empty", []types.Task{}, 0},
		{"none done", []types.Task{
			{ID: 1, Status: types.StatusPending},
			{ID: 2, Status: types.StatusRunning},
		}, 0},
		{"all done", []types.Task{
			{ID: 1, Status: types.StatusDone},
			{ID: 2, Status: types.StatusDone},
		}, 2},
		{"mixed", []types.Task{
			{ID: 1, Status: types.StatusDone},
			{ID: 2, Status: types.StatusFailed},
			{ID: 3, Status: types.StatusDone},
		}, 2},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			if got := countDone(tt.tasks); got != tt.want {
				t.Errorf("countDone() = %d, want %d", got, tt.want)
			}
		})
	}
}

// ═══ boolStr ═══

func TestBoolStrFunc(t *testing.T) {
	if boolStr(true) != "yes" {
		t.Error("boolStr(true) != yes")
	}
	if boolStr(false) != "no" {
		t.Error("boolStr(false) != no")
	}
}

// ═══ addToast / removeToastByID ═══

func TestAddToast(t *testing.T) {
	m := testAppState()
	id1 := m.addToast("hello", "success")
	if id1 != 1 {
		t.Errorf("first toast id=%d, want 1", id1)
	}
	if len(m.toasts) != 1 {
		t.Errorf("toasts=%d, want 1", len(m.toasts))
	}
	id2 := m.addToast("world", "error")
	if id2 != 2 {
		t.Errorf("second toast id=%d, want 2", id2)
	}
	if m.toasts[0].Text != "hello" || m.toasts[1].Text != "world" {
		t.Error("toast text mismatch")
	}
}

func TestAddToastOverflow(t *testing.T) {
	m := testAppState()
	for i := 0; i < maxVisibleToasts+5; i++ {
		m.addToast("toast", "info")
	}
	if len(m.toasts) > maxVisibleToasts+2 {
		t.Errorf("toasts=%d, want <= %d", len(m.toasts), maxVisibleToasts+2)
	}
}

func TestRemoveToastByID(t *testing.T) {
	m := testAppState()
	m.addToast("a", "info")
	m.addToast("b", "error")
	m.addToast("c", "success")
	m.removeToastByID(2)
	if len(m.toasts) != 2 {
		t.Errorf("toasts=%d, want 2", len(m.toasts))
	}
	for _, toast := range m.toasts {
		if toast.ID == 2 {
			t.Error("toast 2 should have been removed")
		}
	}
}

func TestRemoveToastByIDNotFound(t *testing.T) {
	m := testAppState()
	m.addToast("a", "info")
	m.removeToastByID(999)
	if len(m.toasts) != 1 {
		t.Error("should not remove anything")
	}
}

func TestRemoveToastByIDEmpty(t *testing.T) {
	m := testAppState()
	m.removeToastByID(1)
}

// ═══ popScreen ═══

func TestPopScreen(t *testing.T) {
	m := testAppState()
	m.screen = ScreenSettings
	m.screenStack = []Screen{ScreenREPL, ScreenPlan}
	cmd := m.popScreen()
	_ = cmd
	if m.screen != ScreenPlan {
		t.Errorf("screen=%d, want ScreenPlan", m.screen)
	}
	if len(m.screenStack) != 1 {
		t.Errorf("stack=%d, want 1", len(m.screenStack))
	}
}

func TestPopScreenEmpty(t *testing.T) {
	m := testAppState()
	m.screen = ScreenSettings
	m.screenStack = []Screen{}
	m.popScreen()
	if m.screen != ScreenREPL {
		t.Errorf("screen=%d, want ScreenREPL", m.screen)
	}
}

func TestPopScreenNil(t *testing.T) {
	m := testAppState()
	m.screen = ScreenSettings
	m.screenStack = nil
	m.popScreen()
	if m.screen != ScreenREPL {
		t.Errorf("screen=%d, want ScreenREPL", m.screen)
	}
}

// ═══ contentDimensions ═══

func TestContentDimensionsNoSidebar(t *testing.T) {
	m := &AppState{width: 100, height: 40}
	w, h := m.contentDimensions()
	if w != 100 {
		t.Errorf("w=%d, want 100", w)
	}
	if h <= 0 {
		t.Errorf("h=%d, want > 0", h)
	}
}

func TestContentDimensionsSmallWidth(t *testing.T) {
	m := &AppState{width: 0, height: 40}
	w, _ := m.contentDimensions()
	if w < 1 {
		t.Errorf("w=%d, want >= 1", w)
	}
}

func TestContentDimensionsSmallHeight(t *testing.T) {
	m := &AppState{width: 80, height: 0}
	_, h := m.contentDimensions()
	if h < 1 {
		t.Errorf("h=%d, want >= 1", h)
	}
}

// ═══ readAgentCh ═══

func TestReadAgentChNil(t *testing.T) {
	m := &AppState{agentCh: nil}
	cmd := m.readAgentCh()
	if cmd != nil {
		t.Error("nil channel should return nil")
	}
}

func TestReadAgentCh(t *testing.T) {
	ch := make(chan tea.Msg, 1)
	ch <- tea.WindowSizeMsg{Width: 80, Height: 24}
	m := &AppState{agentCh: ch}
	cmd := m.readAgentCh()
	if cmd == nil {
		t.Error("should return non-nil cmd")
	}
}

// ═══ handlePermissionTick ═══

func TestHandlePermissionTickZero(t *testing.T) {
	m := &AppState{permCountdown: 0}
	_ = m.handlePermissionTick()
}

func TestHandlePermissionTickDecrement(t *testing.T) {
	m := testAppState()
	m.permCountdown = 5
	m.permRequest = &tools.PermissionRequest{ID: 1}
	_ = m.handlePermissionTick()
	if m.permCountdown != 4 {
		t.Errorf("countdown=%d, want 4", m.permCountdown)
	}
}

// ═══ handlePlanReady ═══

func TestHandlePlanReadyNilPlanModel(t *testing.T) {
	m := testAppState()
	m.planModel = nil
	_ = m.handlePlanReady(PlanReadyMsg{
		Tasks: []types.Task{{ID: 1, Description: "task"}},
	})
}

func TestHandlePlanReadyWithModel(t *testing.T) {
	m := testAppState()
	m.planModel = NewPlanModel(nil, testTheme(), "", "", "", 0, "", 80, 24)
	_ = m.handlePlanReady(PlanReadyMsg{
		Tasks: []types.Task{
			{ID: 1, Description: "task 1", Status: types.StatusPending},
		},
		TimeEstimate: "5 min",
		CostEstimate: "$0.05",
	})
	if len(m.planModel.tasks) != 1 {
		t.Errorf("tasks=%d, want 1", len(m.planModel.tasks))
	}
	if m.planModel.timeEstimate != "5 min" {
		t.Errorf("timeEstimate=%s, want '5 min'", m.planModel.timeEstimate)
	}
}

func TestHandlePlanReadyEmptyCost(t *testing.T) {
	m := testAppState()
	m.planModel = NewPlanModel(nil, testTheme(), "", "", "", 0, "", 80, 24)
	m.handlePlanReady(PlanReadyMsg{
		Tasks:        []types.Task{},
		TimeEstimate: "2 min",
		CostEstimate: "",
	})
}

// ═══ handleDiscussAnswer ═══

func TestHandleDiscussAnswerNilEngine(t *testing.T) {
	m := testAppState()
	m.workflowEngine = nil
	cmd := m.handleDiscussAnswer(DiscussAnswerMsg{Index: 0, Answer: "yes"})
	if cmd != nil {
		t.Error("nil engine should return nil cmd")
	}
}

// ═══ handleDiscussComplete ═══

func TestHandleDiscussCompleteNilEngine(t *testing.T) {
	m := testAppState()
	m.workflowEngine = nil
	cmd := m.handleDiscussComplete()
	if cmd != nil {
		t.Error("nil engine should return nil cmd")
	}
}

// ═══ processCommandResult ═══

func TestProcessCommandResultEmpty(t *testing.T) {
	m := testAppState()
	cmd := m.processCommandResult(CommandResult{})
	if cmd != nil {
		t.Error("empty result should return nil cmd")
	}
}

func TestProcessCommandResultMessage(t *testing.T) {
	m := testAppState()
	_ = m.processCommandResult(CommandResult{Message: "hello world"})
	if len(m.replModel.messages) == 0 {
		t.Error("should have added a message")
	}
}

func TestProcessCommandResultScreen(t *testing.T) {
	m := testAppState()
	s := ScreenSettings
	cmd := m.processCommandResult(CommandResult{Screen: &s})
	// navigateToScreen starts a transition; screen changes after transition completes.
	// Just verify cmd is non-nil (transition was started).
	if cmd == nil {
		t.Error("should return transition cmd")
	}
}

func TestProcessCommandResultConfirm(t *testing.T) {
	m := testAppState()
	m.processCommandResult(CommandResult{
		ConfirmRequired: true,
		ConfirmPrompt:   "Are you sure?",
	})
	if m.pendingConfirm == nil {
		t.Error("pendingConfirm should be set")
	}
}

// ═══ routeAppMsgAction ═══

func TestRouteAppMsgActionNewSession(t *testing.T) {
	m := testAppState()
	_ = m.routeAppMsgAction(AppMsg{Action: "new_session"})
}

func TestRouteAppMsgActionToggleSidebar(t *testing.T) {
	m := testAppState()
	_ = m.routeAppMsgAction(AppMsg{Action: "toggle_sidebar"})
}

func TestRouteAppMsgActionCancelStream(t *testing.T) {
	m := testAppState()
	m.streamCancelFn = func() {}
	_ = m.routeAppMsgAction(AppMsg{Action: "cancel_stream"})
}

func TestRouteAppMsgActionSessionList(t *testing.T) {
	m := testAppState()
	_ = m.routeAppMsgAction(AppMsg{Action: "session_list"})
}

func TestRouteAppMsgActionOpenSettings(t *testing.T) {
	m := testAppState()
	_ = m.routeAppMsgAction(AppMsg{Action: "open_settings"})
}

func TestRouteAppMsgActionOpenPalette(t *testing.T) {
	m := testAppState()
	_ = m.routeAppMsgAction(AppMsg{Action: "open_palette"})
}

func TestRouteAppMsgActionUnknown(t *testing.T) {
	m := testAppState()
	_ = m.routeAppMsgAction(AppMsg{Action: "unknown_action"})
}

// ═══ handleAppMsg ═══

func TestHandleAppMsgModelSelected(t *testing.T) {
	m := testAppState()
	_ = m.handleAppMsg(AppMsg{
		ModelSelected: &ModelSelectedMsg{
			Model:    types.ModelInfo{ID: "gpt-4"},
			Provider: "openai",
		},
	})
	if m.screen != ScreenREPL {
		t.Errorf("screen=%d, want ScreenREPL", m.screen)
	}
}

func TestHandleAppMsgSessionID(t *testing.T) {
	m := testAppState()
	_ = m.handleAppMsg(AppMsg{SessionID: "test-session"})
}

func TestHandleAppMsgScreen(t *testing.T) {
	m := testAppState()
	_ = m.handleAppMsg(AppMsg{Screen: ScreenSettings})
}

func TestHandleAppMsgAction(t *testing.T) {
	m := testAppState()
	_ = m.handleAppMsg(AppMsg{Action: "toggle_sidebar"})
}

func TestHandleAppMsgEmpty(t *testing.T) {
	m := testAppState()
	cmd := m.handleAppMsg(AppMsg{})
	if cmd != nil {
		t.Error("empty msg should return nil")
	}
}

// ═══ navigateToScreen ═══

func TestNavigateToScreen(t *testing.T) {
	m := testAppState()
	cmd := m.navigateToScreen(ScreenSettings)
	_ = cmd
	// Verify prevScreen was set
	if m.prevScreen != ScreenREPL {
		t.Errorf("prevScreen=%d, want ScreenREPL", m.prevScreen)
	}
	// REPL is never pushed — it's the fallback when stack is empty
	if len(m.screenStack) != 0 {
		t.Errorf("stack=%d, want 0 (REPL is never pushed)", len(m.screenStack))
	}
}

func TestNavigateToScreenFromNonREPL(t *testing.T) {
	m := testAppState()
	m.screen = ScreenSettings
	cmd := m.navigateToScreen(ScreenHelp)
	_ = cmd
	// Non-REPL screen should be pushed onto the stack
	if len(m.screenStack) != 1 {
		t.Errorf("stack=%d, want 1", len(m.screenStack))
	}
	if len(m.screenStack) > 0 && m.screenStack[0] != ScreenSettings {
		t.Errorf("stack[0]=%d, want ScreenSettings", m.screenStack[0])
	}
}

func TestNavigateToScreenSameScreen(t *testing.T) {
	m := testAppState()
	cmd := m.navigateToScreen(ScreenREPL)
	_ = cmd
	// Same screen → no transition, screen stays REPL
	if m.screen != ScreenREPL {
		t.Errorf("screen=%d, want ScreenREPL", m.screen)
	}
}

// ═══ handlePlanRefine ═══

func TestHandlePlanRefineNilEngine(t *testing.T) {
	m := testAppState()
	m.workflowEngine = nil
	_ = m.handlePlanRefine(PlanRefineMsg{Feedback: "make it simpler"})
}

// ═══ setWorkflowPhase ═══

func TestSetWorkflowPhase(t *testing.T) {
	m := testAppState()
	m.setWorkflowPhase(types.PhasePlan)
	if m.workflowPhase != types.PhasePlan {
		t.Errorf("workflowPhase = %q, want %q", m.workflowPhase, types.PhasePlan)
	}
}

func TestSetWorkflowPhase_UpdatesReplModel(t *testing.T) {
	m := testAppState()
	m.setWorkflowPhase(types.PhaseExecute)
	if m.replModel.lastStatus != "Phase: execute" {
		t.Errorf("replModel.lastStatus = %q, want %q", m.replModel.lastStatus, "Phase: execute")
	}
}

func TestSetWorkflowPhase_NilReplModel(t *testing.T) {
	m := testAppState()
	m.replModel = nil
	// Should not panic
	m.setWorkflowPhase(types.PhaseVerify)
	if m.workflowPhase != types.PhaseVerify {
		t.Errorf("workflowPhase = %q, want %q", m.workflowPhase, types.PhaseVerify)
	}
}

// ═══ SetCwd / SetResumeSessionID ═══

func TestSetCwd(t *testing.T) {
	m := testAppState()
	m.SetCwd("/home/user/project")
	if m.cwd != "/home/user/project" {
		t.Errorf("cwd = %q, want /home/user/project", m.cwd)
	}
}

func TestSetResumeSessionID(t *testing.T) {
	m := testAppState()
	m.SetResumeSessionID("session-abc")
	if m.resumeSessionID != "session-abc" {
		t.Errorf("resumeSessionID = %q, want session-abc", m.resumeSessionID)
	}
}

// ═══ addToastCmd ═══

func TestAddToastCmd(t *testing.T) {
	m := testAppState()
	cmd := m.addToastCmd("hello", "info", 0)
	if cmd == nil {
		t.Error("addToastCmd should return non-nil Cmd")
	}
	if len(m.toasts) != 1 {
		t.Errorf("toasts len = %d, want 1", len(m.toasts))
	}
}

func TestAddToastCmd_WithDuration(t *testing.T) {
	m := testAppState()
	cmd := m.addToastCmd("warning", "warn", 5*time.Second)
	if cmd == nil {
		t.Error("addToastCmd should return non-nil Cmd")
	}
	if m.toasts[0].Text != "warning" {
		t.Errorf("toast text = %q, want warning", m.toasts[0].Text)
	}
}

// ═══ Shutdown ═══

func TestShutdown(t *testing.T) {
	m := testAppState()
	// Should not panic even with nil fields
	m.Shutdown()
}

func TestShutdown_WithCancel(t *testing.T) {
	m := testAppState()
	m.workflowCancel = func() {}
	m.shutdownCancel = func() {}
	m.Shutdown()
}

// ═══ handlePermissionTick ═══

func TestHandlePermissionTick_ZeroCountdown(t *testing.T) {
	m := testAppState()
	m.permRequest = &tools.PermissionRequest{ID: 1, TimeoutSecs: 10}
	m.permCountdown = 0
	cmd := m.handlePermissionTick()
	if cmd == nil {
		t.Error("handlePermissionTick should return a tick cmd when permRequest is active")
	}
}

func TestHandlePermissionTick_PositiveCountdown(t *testing.T) {
	m := testAppState()
	m.permRequest = &tools.PermissionRequest{ID: 1, TimeoutSecs: 10}
	m.permCountdown = 5
	cmd := m.handlePermissionTick()
	if m.permCountdown != 4 {
		t.Errorf("countdown = %d, want 4", m.permCountdown)
	}
	_ = cmd
}

func TestHandlePermissionTick_Timeout(t *testing.T) {
	m := testAppState()
	m.permCountdown = 1
	// permRequest is nil → should return nil immediately
	cmd := m.handlePermissionTick()
	if cmd != nil {
		t.Error("nil permRequest should return nil cmd")
	}
}

// ═══ popScreen ═══

func TestPopScreen_Empty(t *testing.T) {
	m := testAppState()
	m.screenStack = []Screen{}
	cmd := m.popScreen()
	if cmd != nil {
		t.Error("popScreen with empty stack should return nil")
	}
}

// ═══ openSettingsScreen ═══

func TestOpenSettingsScreen(t *testing.T) {
	m := testAppState()
	cmd := m.openSettingsScreen()
	_ = cmd
	// Should navigate to settings
}

// ═══ runWorkflowFromGoal ═══

func TestRunWorkflowFromGoal_NilEngine(t *testing.T) {
	m := testAppState()
	m.workflowEngine = nil
	cmd := m.runWorkflowFromGoal("build a website")
	if cmd == nil {
		t.Error("runWorkflowFromGoal should return non-nil Cmd")
	}
	if m.workflowGoal != "build a website" {
		t.Errorf("workflowGoal = %q, want build a website", m.workflowGoal)
	}
}

func TestRunWorkflowFromGoal_PhasePreserved(t *testing.T) {
	m := testAppState()
	m.workflowEngine = nil
	m.workflowPhase = types.PhaseExecute
	cmd := m.runWorkflowFromGoal("do something")
	_ = cmd
	if m.workflowPhase != types.PhaseExecute {
		t.Errorf("workflowPhase = %q, want %q (preserved)", m.workflowPhase, types.PhaseExecute)
	}
}

// ═══ openResumeScreen ═══

func TestOpenResumeScreen_NilSession(t *testing.T) {
	m := testAppState()
	m.sessionManager = nil
	cmd := m.openResumeScreen()
	if cmd == nil {
		t.Error("openResumeScreen should always return a Cmd")
	}
}

// ═══ handleAppMsg ═══

func TestHandleAppMsg_EmptyMsg(t *testing.T) {
	m := testAppState()
	msg := AppMsg{}
	cmd := m.handleAppMsg(msg)
	_ = cmd
	// Empty AppMsg with no screen/action should be handled gracefully
}

// ═══ handlePermissionResponse ═══

func TestHandlePermissionResponse_NilDispatcher(t *testing.T) {
	m := testAppState()
	m.dispatcher = nil
	cmd := m.handlePermissionResponse(PermissionResponseMsg{
		Response: tools.PermissionResponse{Allowed: true},
	})
	if cmd != nil {
		t.Error("nil dispatcher should return nil cmd")
	}
}

func TestHandlePermissionResponse_NilPermRequest(t *testing.T) {
	m := testAppState()
	cmd := m.handlePermissionResponse(PermissionResponseMsg{
		Response: tools.PermissionResponse{Allowed: true},
	})
	if cmd != nil {
		t.Error("nil permRequest should return nil cmd")
	}
}
