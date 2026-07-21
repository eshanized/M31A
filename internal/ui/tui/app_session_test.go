package tui

import (
	"testing"

	"github.com/eshanized/M31A/internal/tools"
	"github.com/eshanized/M31A/internal/ui/tui/theme"
	"github.com/eshanized/M31A/internal/types"
)

func TestResolveWorkflowMode_ConfigOverride(t *testing.T) {
	m := &AppState{
		config: &testConfig{},
	}
	m.config.Features.WorkflowMode = string(types.ModeFast)
	mode := m.resolveWorkflowMode("build a website")
	if mode != types.ModeFast {
		t.Errorf("mode=%q, want ModeFast", mode)
	}
}

func TestResolveWorkflowMode_ConfigFull(t *testing.T) {
	m := &AppState{
		config: &testConfig{},
	}
	m.config.Features.WorkflowMode = string(types.ModeFull)
	mode := m.resolveWorkflowMode("simple task")
	if mode != types.ModeFull {
		t.Errorf("mode=%q, want ModeFull", mode)
	}
}

func TestResolveWorkflowMode_ConfigDirect(t *testing.T) {
	m := &AppState{
		config: &testConfig{},
	}
	m.config.Features.WorkflowMode = string(types.ModeDirect)
	mode := m.resolveWorkflowMode("fix a bug")
	if mode != types.ModeDirect {
		t.Errorf("mode=%q, want ModeDirect", mode)
	}
}

func TestResolveWorkflowMode_NilConfig(t *testing.T) {
	m := &AppState{config: nil}
	mode := m.resolveWorkflowMode("build a website")
	// Should not panic and should return some mode
	_ = mode
}

func TestResolveWorkflowMode_EmptyGoal(t *testing.T) {
	m := &AppState{config: nil}
	mode := m.resolveWorkflowMode("")
	_ = mode
}

func TestHandlePermissionTick_NilPermRequest(t *testing.T) {
	m := &AppState{permRequest: nil}
	cmd := m.handlePermissionTick()
	if cmd != nil {
		t.Error("nil permRequest should return nil cmd")
	}
}

func TestSessionHandlePermissionTick_ZeroCountdown(t *testing.T) {
	m := testAppState()
	m.permRequest = &tools.PermissionRequest{ID: 1, TimeoutSecs: 10}
	m.permCountdown = 0
	cmd := m.handlePermissionTick()
	if cmd == nil {
		t.Error("handlePermissionTick should return a tick cmd when permRequest is active")
	}
}

func TestSessionHandlePermissionTick_PositiveCountdown(t *testing.T) {
	m := testAppState()
	m.permRequest = &tools.PermissionRequest{ID: 1, TimeoutSecs: 10}
	m.permCountdown = 5
	cmd := m.handlePermissionTick()
	if m.permCountdown != 4 {
		t.Errorf("countdown = %d, want 4", m.permCountdown)
	}
	_ = cmd
}

func TestSessionHandlePermissionTick_Timeout(t *testing.T) {
	m := testAppState()
	m.permCountdown = 1
	// permRequest is nil -> should return nil immediately
	cmd := m.handlePermissionTick()
	if cmd != nil {
		t.Error("nil permRequest should return nil cmd")
	}
}

func TestSessionHandlePermissionResponse_NilDispatcher(t *testing.T) {
	m := testAppState()
	m.dispatcher = nil
	cmd := m.handlePermissionResponse(PermissionResponseMsg{
		Response: tools.PermissionResponse{Allowed: true},
	})
	if cmd != nil {
		t.Error("nil dispatcher should return nil cmd")
	}
}

func TestSessionHandlePermissionResponse_NilPermRequest(t *testing.T) {
	m := testAppState()
	cmd := m.handlePermissionResponse(PermissionResponseMsg{
		Response: tools.PermissionResponse{Allowed: true},
	})
	if cmd != nil {
		t.Error("nil permRequest should return nil cmd")
	}
}

func TestHandlePermissionKey_NilPermRequest(t *testing.T) {
	rm := NewReplModel(testTheme(), "v1")
	tm := theme.NewManager(theme.ModeDark)
	m := &AppState{
		replModel:    &rm,
		themeManager: tm,
		permRequest:  nil,
		screen:       ScreenREPL,
		width:        80,
		height:       24,
	}
	msg := testKeyMsg("y")
	cmd := m.handlePermissionKey(msg)
	_ = cmd
	// Should return to REPL when permRequest is nil
}

func TestHandlePermissionKey_WithQuestionRequest(t *testing.T) {
	rm := NewReplModel(testTheme(), "v1")
	tm := theme.NewManager(theme.ModeDark)
	m := &AppState{
		replModel:       &rm,
		themeManager:    tm,
		questionRequest: &QuestionRequestMsg{ID: 1, Question: "test?"},
		screen:          ScreenPermission,
		width:           80,
		height:          24,
	}
	msg := testKeyMsg("y")
	cmd := m.handlePermissionKey(msg)
	_ = cmd
	// Should delegate to handleQuestionKey
}

func TestHandleQuestionKey_NilModel(t *testing.T) {
	m := &AppState{
		questionModel: nil,
		screen:        ScreenREPL,
	}
	msg := testKeyMsg("y")
	cmd := m.handleQuestionKey(msg)
	if cmd != nil {
		t.Error("nil questionModel should return nil cmd")
	}
}

func TestHandleQuestionResponse_NilDispatcher(t *testing.T) {
	m := testAppState()
	m.dispatcher = nil
	cmd := m.handleQuestionResponse(QuestionResponseMsg{
		Answer: "yes",
	})
	if cmd != nil {
		t.Error("nil dispatcher should return nil cmd")
	}
}

func TestHandleQuestionResponse_NilQuestionRequest(t *testing.T) {
	m := testAppState()
	cmd := m.handleQuestionResponse(QuestionResponseMsg{
		Answer: "yes",
	})
	if cmd != nil {
		t.Error("nil questionRequest should return nil cmd")
	}
}

func TestHandleDiscussAnswer_NilEngine(t *testing.T) {
	m := testAppState()
	m.workflowEngine = nil
	cmd := m.handleDiscussAnswer(DiscussAnswerMsg{Index: 0, Answer: "yes"})
	if cmd != nil {
		t.Error("nil engine should return nil cmd")
	}
}

func TestHandleDiscussComplete_NilEngine(t *testing.T) {
	m := testAppState()
	m.workflowEngine = nil
	cmd := m.handleDiscussComplete()
	if cmd != nil {
		t.Error("nil engine should return nil cmd")
	}
}

func TestStartNewSession_NilManager(t *testing.T) {
	m := testAppState()
	m.sessionManager = nil
	cmd := m.startNewSession()
	if cmd != nil {
		t.Error("nil sessionManager should return nil cmd")
	}
}

func TestSessionRunWorkflowFromGoal_NilEngine(t *testing.T) {
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

func TestSessionRunWorkflowFromGoal_PhasePreserved(t *testing.T) {
	m := testAppState()
	m.workflowEngine = nil
	m.workflowPhase = types.PhaseExecute
	cmd := m.runWorkflowFromGoal("do something")
	_ = cmd
	if m.workflowPhase != types.PhaseExecute {
		t.Errorf("workflowPhase = %q, want %q (preserved)", m.workflowPhase, types.PhaseExecute)
	}
}

func TestRunWorkflowFromGoal_SetsGoal(t *testing.T) {
	m := testAppState()
	m.workflowEngine = nil
	m.runWorkflowFromGoal("test goal")
	if m.workflowGoal != "test goal" {
		t.Errorf("workflowGoal = %q, want test goal", m.workflowGoal)
	}
}

func TestRunWorkflowFromGoal_SetsPhase(t *testing.T) {
	m := testAppState()
	m.workflowEngine = nil
	m.workflowPhase = types.PhaseIdle
	m.runWorkflowFromGoal("test goal")
	if m.workflowPhase != types.PhaseInitialize {
		t.Errorf("workflowPhase = %q, want %q", m.workflowPhase, types.PhaseInitialize)
	}
}

func TestRunWorkflowFromGoal_WithSidebar(t *testing.T) {
	rm := NewReplModel(testTheme(), "v1")
	tm := theme.NewManager(theme.ModeDark)
	sm := NewSidebarModel(nil, tm.Current())
	m := &AppState{
		replModel:    &rm,
		themeManager: tm,
		sidebarModel: sm,
		width:        80,
		height:       24,
	}
	m.workflowEngine = nil
	cmd := m.runWorkflowFromGoal("test")
	_ = cmd
	// Sidebar should be in todo mode
}

// Verify signatures
func TestStartNewSession_Signature(t *testing.T) {
	var fn = (&AppState{}).startNewSession
	_ = fn
}

func TestRunWorkflowFromGoal_Signature(t *testing.T) {
	var fn = (&AppState{}).runWorkflowFromGoal
	_ = fn
}

func TestResolveWorkflowMode_Signature(t *testing.T) {
	var fn = (&AppState{}).resolveWorkflowMode
	_ = fn
}

func TestHandlePermissionResponse_Signature(t *testing.T) {
	var fn = (&AppState{}).handlePermissionResponse
	_ = fn
}

func TestHandlePermissionTick_Signature(t *testing.T) {
	var fn = (&AppState{}).handlePermissionTick
	_ = fn
}

func TestHandlePermissionKey_Signature(t *testing.T) {
	var fn = (&AppState{}).handlePermissionKey
	_ = fn
}

func TestHandleQuestionKey_Signature(t *testing.T) {
	var fn = (&AppState{}).handleQuestionKey
	_ = fn
}

func TestHandleQuestionResponse_Signature(t *testing.T) {
	var fn = (&AppState{}).handleQuestionResponse
	_ = fn
}

func TestHandleDiscussAnswer_Signature(t *testing.T) {
	var fn = (&AppState{}).handleDiscussAnswer
	_ = fn
}

func TestHandleDiscussComplete_Signature(t *testing.T) {
	var fn = (&AppState{}).handleDiscussComplete
	_ = fn
}
