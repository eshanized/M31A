package tui

import (
	"testing"

	"github.com/eshanized/M31A/internal/tui/theme"
	"github.com/eshanized/M31A/internal/engine/workflow"
)

func newTestAppStateForWorkflow() *AppState {
	return &AppState{
		themeManager: theme.NewManager(theme.ModeDark),
		router:       NewRouter(),
		width:        80,
		height:       24,
	}
}

func TestHandlePhaseResultMsg_NilEngine(t *testing.T) {
	m := newTestAppStateForWorkflow()
	msg := PhaseResultMsg{}
	result, cmd := handlePhaseResultMsg(m, msg)
	if result == nil {
		t.Fatal("expected non-nil model")
	}
	if cmd != nil {
		t.Fatal("expected nil cmd when workflow engine is nil")
	}
}

func TestHandlePlanReadyMsg_NilPlanModel(t *testing.T) {
	m := newTestAppStateForWorkflow()
	msg := PlanReadyMsg{}
	result, cmd := handlePlanReadyMsg(m, msg)
	if result == nil {
		t.Fatal("expected non-nil model")
	}
	if cmd != nil {
		t.Fatal("expected nil cmd")
	}
}

func TestHandlePlanApproveMsg_NilEngine(t *testing.T) {
	m := newTestAppStateForWorkflow()
	msg := PlanApproveMsg{}
	// handlePlanApprove may return non-nil cmd due to RunPhaseCmd
	// When engine is nil, RunPhaseCmd returns an error cmd
	result, _ := handlePlanApproveMsg(m, msg)
	if result == nil {
		t.Fatal("expected non-nil model")
	}
}

func TestHandlePlanRefineMsg_NilEngine(t *testing.T) {
	m := newTestAppStateForWorkflow()
	msg := PlanRefineMsg{}
	// handlePlanRefine may return non-nil cmd due to RunPhaseCmd
	result, _ := handlePlanRefineMsg(m, msg)
	if result == nil {
		t.Fatal("expected non-nil model")
	}
}

func TestHandleDemonstrationReadyMsg_NilShipModel(t *testing.T) {
	m := newTestAppStateForWorkflow()
	msg := workflow.DemonstrationReadyMsg{Content: "demo"}
	result, cmd := handleDemonstrationReadyMsg(m, msg)
	if result == nil {
		t.Fatal("expected non-nil model")
	}
	if cmd != nil {
		t.Fatal("expected nil cmd")
	}
}

func TestHandleExecutePauseMsg_NilExecuteModel(t *testing.T) {
	m := newTestAppStateForWorkflow()
	msg := ExecutePauseMsg{Paused: true}
	result, cmd := handleExecutePauseMsg(m, msg)
	if result == nil {
		t.Fatal("expected non-nil model")
	}
	if cmd != nil {
		t.Fatal("expected nil cmd")
	}
}

func TestHandleHealResultMsg_NilVerifyModel(t *testing.T) {
	m := newTestAppStateForWorkflow()
	msg := HealResultMsg{}
	result, cmd := handleHealResultMsg(m, msg)
	if result == nil {
		t.Fatal("expected non-nil model")
	}
	if cmd != nil {
		t.Fatal("expected nil cmd")
	}
}

func TestHandlerWorkflowSignatures(t *testing.T) {
	var f1 = handlePhaseResultMsg
	var f2 = handlePlanReadyMsg
	var f3 = handlePlanApproveMsg
	var f4 = handlePlanRefineMsg
	var f5 = handleDemonstrationReadyMsg
	var f6 = handleExecutePauseMsg
	var f7 = handleHealResultMsg
	var f8 = handleTaskStartWorkflowMsg
	var f9 = handleTaskUpdateWorkflowMsg
	var f10 = handleToolStartWorkflowMsg
	var f11 = handleToolCompleteWorkflowMsg
	var f12 = handleSelfHealStartWorkflowMsg
	var f13 = handleSelfHealCompleteWorkflowMsg
	_ = f1
	_ = f2
	_ = f3
	_ = f4
	_ = f5
	_ = f6
	_ = f7
	_ = f8
	_ = f9
	_ = f10
	_ = f11
	_ = f12
	_ = f13
}
