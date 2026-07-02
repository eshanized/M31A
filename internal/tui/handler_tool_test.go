package tui

import (
	"testing"

	tea "github.com/charmbracelet/bubbletea"
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
	msg := tools.QuestionResponse{Answer: "yes"}
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
	var f1 func(*AppState, PermissionRequestMsg) (tea.Model, tea.Cmd) = handlePermissionRequestMsg
	var f2 func(*AppState, PermissionResponseMsg) (tea.Model, tea.Cmd) = handlePermissionResponseMsg
	var f3 func(*AppState, PermissionTickMsg) (tea.Model, tea.Cmd) = handlePermissionTickMsg
	var f4 func(*AppState, QuestionRequestMsg) (tea.Model, tea.Cmd) = handleQuestionRequestMsg
	var f5 func(*AppState, QuestionResponseMsg) (tea.Model, tea.Cmd) = handleQuestionResponseMsg
	var f6 func(*AppState, tools.QuestionResponse) (tea.Model, tea.Cmd) = handleToolsQuestionResponse
	_ = f1
	_ = f2
	_ = f3
	_ = f4
	_ = f5
	_ = f6
}
