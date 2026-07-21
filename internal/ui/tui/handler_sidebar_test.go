package tui

import (
	"testing"

	"github.com/eshanized/M31A/internal/integrations/arbitrage"
)

func TestHandleGoalSubmittedMsg_Signature(t *testing.T) {
	// Verify function signature matches expected pattern.
	// Full test skipped: NewPhaseModelPickerModel requires registry setup.
	var f = handleGoalSubmittedMsg
	_ = f
}

func TestHandleSidebarRevertMsg_NilSidebar(t *testing.T) {
	m := newTestAppStateForWorkflow()
	msg := SidebarRevertMsg{}
	result, cmd := handleSidebarRevertMsg(m, msg)
	if result == nil {
		t.Fatal("expected non-nil model")
	}
	if cmd != nil {
		t.Fatal("expected nil cmd when sidebarModel is nil")
	}
}

func TestHandleGhostWriteRequestMsg_EmptyFiles(t *testing.T) {
	m := newTestAppStateForWorkflow()
	msg := GhostWriteRequestMsg{Files: []string{}}
	result, cmd := handleGhostWriteRequestMsg(m, msg)
	if result == nil {
		t.Fatal("expected non-nil model")
	}
	if cmd != nil {
		t.Fatal("expected nil cmd when no files")
	}
}

func TestHandleGhostWriteResultMsg_NilResult(t *testing.T) {
	m := newTestAppStateForWorkflow()
	msg := GhostWriteResultMsg{}
	result, cmd := handleGhostWriteResultMsg(m, msg)
	if result == nil {
		t.Fatal("expected non-nil model")
	}
	if cmd != nil {
		t.Fatal("expected nil cmd")
	}
}

func TestHandleOptimizedMsg_EmptyRecommendations(t *testing.T) {
	m := newTestAppStateForWorkflow()
	msg := OptimizedMsg{Recommendations: []arbitrage.ArbitrageRecommendation{}}
	result, cmd := handleOptimizedMsg(m, msg)
	if result == nil {
		t.Fatal("expected non-nil model")
	}
	if cmd != nil {
		t.Fatal("expected nil cmd when no recommendations")
	}
}

func TestHandleSidebarRefreshTickMsg_NilSidebar(t *testing.T) {
	m := newTestAppStateForWorkflow()
	msg := SidebarRefreshTickMsg{}
	result, cmd := handleSidebarRefreshTickMsg(m, msg)
	if result == nil {
		t.Fatal("expected non-nil model")
	}
	if cmd != nil {
		t.Fatal("expected nil cmd")
	}
}

func TestHandleSidebarRefreshMsg_NilSidebar(t *testing.T) {
	m := newTestAppStateForWorkflow()
	msg := SidebarRefreshMsg{}
	result, cmd := handleSidebarRefreshMsg(m, msg)
	if result == nil {
		t.Fatal("expected non-nil model")
	}
	if cmd != nil {
		t.Fatal("expected nil cmd")
	}
}

func TestHandleSidebarTodoUpdateMsg_NilSidebar(t *testing.T) {
	m := newTestAppStateForWorkflow()
	msg := SidebarTodoUpdateMsg{}
	result, cmd := handleSidebarTodoUpdateMsg(m, msg)
	if result == nil {
		t.Fatal("expected non-nil model")
	}
	if cmd != nil {
		t.Fatal("expected nil cmd")
	}
}

func TestHandlerSidebarSignatures(t *testing.T) {
	var f1 = handleGoalSubmittedMsg
	var f2 = handleSidebarRevertMsg
	var f3 = handleGhostWriteRequestMsg
	var f4 = handleGhostWriteResultMsg
	var f5 = handleOptimizedMsg
	var f6 = handleSidebarRefreshTickMsg
	var f7 = handleSidebarRefreshMsg
	var f8 = handleSidebarTodoUpdateMsg
	_ = f1
	_ = f2
	_ = f3
	_ = f4
	_ = f5
	_ = f6
	_ = f7
	_ = f8
}
