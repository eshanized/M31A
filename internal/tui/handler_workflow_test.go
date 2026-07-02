package tui

import (
	"testing"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/eshanized/M31A/internal/tui/theme"
	"github.com/eshanized/M31A/internal/workflow"
)

func newTestAppStateForWorkflow() *AppState {
	return &AppState{
		themeManager: theme.NewManager(theme.ModeDark),
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
	var f1 func(*AppState, PhaseResultMsg) (tea.Model, tea.Cmd) = handlePhaseResultMsg
	var f2 func(*AppState, PlanReadyMsg) (tea.Model, tea.Cmd) = handlePlanReadyMsg
	var f3 func(*AppState, PlanApproveMsg) (tea.Model, tea.Cmd) = handlePlanApproveMsg
	var f4 func(*AppState, PlanRefineMsg) (tea.Model, tea.Cmd) = handlePlanRefineMsg
	var f5 func(*AppState, workflow.DemonstrationReadyMsg) (tea.Model, tea.Cmd) = handleDemonstrationReadyMsg
	var f6 func(*AppState, ExecutePauseMsg) (tea.Model, tea.Cmd) = handleExecutePauseMsg
	var f7 func(*AppState, HealResultMsg) (tea.Model, tea.Cmd) = handleHealResultMsg
	var f8 func(*AppState, workflow.TaskStartMsg) (tea.Model, tea.Cmd) = handleTaskStartWorkflowMsg
	var f9 func(*AppState, workflow.TaskUpdateMsg) (tea.Model, tea.Cmd) = handleTaskUpdateWorkflowMsg
	var f10 func(*AppState, workflow.ToolStartMsg) (tea.Model, tea.Cmd) = handleToolStartWorkflowMsg
	var f11 func(*AppState, workflow.ToolCompleteMsg) (tea.Model, tea.Cmd) = handleToolCompleteWorkflowMsg
	var f12 func(*AppState, workflow.SelfHealStartMsg) (tea.Model, tea.Cmd) = handleSelfHealStartWorkflowMsg
	var f13 func(*AppState, workflow.SelfHealCompleteMsg) (tea.Model, tea.Cmd) = handleSelfHealCompleteWorkflowMsg
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
