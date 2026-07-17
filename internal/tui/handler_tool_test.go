package tui

import (
	"testing"

	"github.com/eshanized/M31A/internal/tools"
	"github.com/eshanized/M31A/internal/tui/theme"
)

func newTestAppState() *AppState {
	return &AppState{
		themeManager: theme.NewManager(theme.ModeDark),
	}
}

func TestHandlePermissionRequestMsg_NilSidebar(t *testing.T) {
	m := newTestAppState()
	msg := PermissionRequestMsg{}
	result, cmd := handlePermissionRequestMsg(m, msg)
	if result == nil {
		t.Fatal("expected non-nil model")
	}
	if cmd != nil {
		t.Fatal("expected nil cmd")
	}
}

func TestHandlePermissionResponseMsg_NilDispatcher(t *testing.T) {
	m := newTestAppState()
	msg := PermissionResponseMsg{}
	result, cmd := handlePermissionResponseMsg(m, msg)
	if result == nil {
		t.Fatal("expected non-nil model")
	}
	if cmd != nil {
		t.Fatal("expected nil cmd when dispatcher is nil")
	}
}

func TestHandlePermissionTickMsg_NilPermRequest(t *testing.T) {
	m := newTestAppState()
	msg := PermissionTickMsg{}
	result, cmd := handlePermissionTickMsg(m, msg)
	if result == nil {
		t.Fatal("expected non-nil model")
	}
	if cmd != nil {
		t.Fatal("expected nil cmd when permRequest is nil")
	}
}

func TestHandleQuestionRequestMsg_NilModel(t *testing.T) {
	m := newTestAppState()
	msg := QuestionRequestMsg{
		Question: "test",
		Header:   "header",
	}
	result, cmd := handleQuestionRequestMsg(m, msg)
	if result == nil {
		t.Fatal("expected non-nil model")
	}
	if cmd != nil {
		t.Fatal("expected nil cmd")
	}
}

func TestHandleQuestionResponseMsg_NilDispatcher(t *testing.T) {
	m := newTestAppState()
	msg := QuestionResponseMsg{Answer: "yes"}
	result, cmd := handleQuestionResponseMsg(m, msg)
	if result == nil {
		t.Fatal("expected non-nil model")
	}
	if cmd != nil {
		t.Fatal("expected nil cmd when dispatcher is nil")
	}
}

func TestHandleToolsQuestionResponse_NilDispatcher(t *testing.T) {
	m := newTestAppState()
	msg := types.QuestionResponse{Answer: "yes"}
	result, cmd := handleToolsQuestionResponse(m, msg)
	if result == nil {
		t.Fatal("expected non-nil model")
	}
	if cmd != nil {
		t.Fatal("expected nil cmd when dispatcher is nil")
	}
}

func TestHandlerToolSignatures(t *testing.T) {
	// Verify function signatures match expected patterns.
	var f1 = handlePermissionRequestMsg
	var f2 = handlePermissionResponseMsg
	var f3 = handlePermissionTickMsg
	var f4 = handleQuestionRequestMsg
	var f5 = handleQuestionResponseMsg
	var f6 = handleToolsQuestionResponse
	_ = f1
	_ = f2
	_ = f3
	_ = f4
	_ = f5
	_ = f6
}
